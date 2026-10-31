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
        result.update(js_inputs=sum(Path(row.get("path", "")).suffix.lower() in {".js", ".mjs"} for row in captured.get("original_inputs", [])),
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


class _LocaleSource:
    """Literal locale resolution shared by route maps and split webpack chunks."""
    def __init__(self, raw):
        sys.path.insert(0, str(a.ROOT / "tools/analyzers/python-static"))
        from tree_sitter import Language, Parser
        import tree_sitter_javascript
        self.raw = raw
        self.tree = Parser(Language(tree_sitter_javascript.language())).parse(raw)
        if self.tree.root_node.has_error:
            raise ValueError("Locale export source has JavaScript parse errors")
        self.bindings = JavascriptBindings(raw, self.tree)
        self.objects, self.exports = [], []
        pending = [self.tree.root_node]
        while pending:
            node = pending.pop()
            pending.extend(reversed(node.named_children))
            if node.type == "object":
                self.objects.append(node)
            elif node.type == "call_expression":
                function = node.child_by_field_name("function")
                arguments = node.child_by_field_name("arguments")
                if function is None or function.type != "member_expression" or arguments is None:
                    continue
                method = function.child_by_field_name("property")
                parts = arguments.named_children
                if (method is not None and method.text == b"d" and len(parts) == 2
                        and parts[0].type == "identifier" and parts[1].type == "object"):
                    self.exports.append((node, parts[0], parts[1]))

    @staticmethod
    def string(node):
        try:
            value = json.loads(node.text)
        except json.JSONDecodeError:
            import json5
            value = json5.loads(node.text.decode())
        if not isinstance(value, str):
            raise ValueError("Locale source literal is not a string")
        return value

    def region(self, node):
        body = self.raw[node.start_byte:node.end_byte]
        return {"start_byte": node.start_byte, "end_byte": node.end_byte,
                "body": body.decode("utf-8"), "sha256": hashlib.sha256(body).hexdigest()}

    @staticmethod
    def scope(node):
        while node.parent is not None and node.type not in {"statement_block", "program"}:
            node = node.parent
        return node

    def declaration(self, node, owner):
        if node.type != "identifier":
            raise ValueError("Locale reference is not a lexical identifier")
        found = self.bindings.resolve(node.text.decode(), node.start_byte)
        if found["resolution"] != "declaration":
            raise ValueError("Locale binding does not resolve to one declaration")
        declaration = self.tree.root_node.descendant_for_byte_range(found["start_byte"], found["end_byte"] - 1)
        if self.scope(declaration) != owner:
            raise ValueError("Locale binding escapes its lexical owner")
        if declaration.child_by_field_name("value") is None:
            raise ValueError("Locale binding has no value")
        return declaration

    def literal(self, node, owner, seen=()):
        if node.type == "string":
            return self.string(node)
        if node.type != "identifier" or node.text in seen or len(seen) > 32:
            raise ValueError("Locale binding is dynamic or cyclic")
        value = self.declaration(node, owner).child_by_field_name("value")
        return self.literal(value, owner, (*seen, node.text))

    def property_key(self, node, owner):
        if node.type in {"property_identifier", "identifier"}:
            return node.text.decode()
        if node.type == "string":
            return self.string(node)
        if node.type == "computed_property_name" and len(node.named_children) == 1:
            return self.literal(node.named_children[0], owner)
        raise ValueError("Locale map key is dynamic")

    @staticmethod
    def returned(node):
        if node.type not in {"arrow_function", "function_expression"}:
            raise ValueError("Locale export is not a zero-argument function")
        parameters = node.child_by_field_name("parameters")
        body = node.child_by_field_name("body")
        if parameters is None or parameters.named_children or body is None:
            raise ValueError("Locale export getter requires dynamic arguments")
        if body.type == "statement_block":
            statements = [child for child in body.named_children if child.type != "comment"]
            if (len(statements) != 1 or statements[0].type != "return_statement"
                    or len(statements[0].named_children) != 1):
                raise ValueError("Locale callback is not a single return")
            body = statements[0].named_children[0]
        return body

    def values(self, dictionary, owner, *, getters):
        values, resolved, seen = {}, [], set()
        for pair in dictionary.named_children:
            if pair.type != "pair":
                raise ValueError("Locale dictionary contains a spread or shorthand")
            key = self.property_key(pair.child_by_field_name("key"), owner)
            value = pair.child_by_field_name("value")
            if key in seen:
                raise ValueError("Locale export key is duplicate: " + key)
            seen.add(key)
            if not getters and key == "__proto__" and value.type == "null":
                continue
            if getters:
                value = self.returned(value)
            values[key] = self.literal(value, owner)
            resolved.append({"key": key, "export": self.region(pair)})
        if not values:
            raise ValueError("Locale export dictionary is empty")
        return values, resolved

    def global_call(self, node, global_name, method):
        if node.type != "call_expression":
            raise ValueError("Locale expression is not the expected static call")
        function = node.child_by_field_name("function")
        if function.type != "member_expression":
            raise ValueError("Locale expression has an unexpected callee")
        target = function.child_by_field_name("object")
        member = function.child_by_field_name("property")
        if (target.type != "identifier" or target.text.decode() != global_name
                or member.text.decode() != method
                or self.bindings.resolve(global_name, target.start_byte)["resolution"] != "unresolved_identifier"):
            raise ValueError("Locale expression shadows or replaces its standard global")
        return node.child_by_field_name("arguments").named_children

    def lazy_reference(self, getter):
        call = self.returned(getter)
        if call.type != "call_expression":
            raise ValueError("Locale loader is not a static promise callback")
        function = call.child_by_field_name("function")
        if (function.type != "member_expression"
                or function.child_by_field_name("property").text != b"then"):
            raise ValueError("Locale loader has an unexpected promise method")
        if self.global_call(function.child_by_field_name("object"), "Promise", "resolve"):
            raise ValueError("Locale promise resolves a dynamic input")
        callbacks = call.child_by_field_name("arguments").named_children
        if len(callbacks) != 1:
            raise ValueError("Locale loader has additional callbacks")
        reference = self.returned(callbacks[0])
        if reference.type != "identifier":
            raise ValueError("Locale loader does not return its static namespace")
        return reference


def locale_export_dictionaries(raw, locales):
    """Resolve mounted literal/frozen namespaces and webpack exports statically."""
    source = _LocaleSource(raw)
    wanted = set(locales)
    if not wanted or len(wanted) != len(locales) or "en" not in wanted:
        raise ValueError("Locale map must contain unique language identities and English")
    candidates = []
    for node in source.objects:
        if (len(node.named_children) != len(wanted)
                or any(pair.type != "pair" for pair in node.named_children)):
            continue
        owner = source.scope(node)
        try:
            entries = [(source.property_key(pair.child_by_field_name("key"), owner),
                        pair.child_by_field_name("value")) for pair in node.named_children]
        except ValueError:
            continue
        if (set(key for key, _ in entries) == wanted
                and all(value.type in {"identifier", "arrow_function", "function_expression"}
                        for _, value in entries)):
            candidates.append((node, owner, entries))
    if len(candidates) != 1:
        raise ValueError(f"Mounted locale map is not unique: {len(candidates)}")
    route, owner, entries = candidates[0]
    dictionaries, dictionary_owners, namespaces = {}, [], set()
    for locale, loader in entries:
        reference = loader if loader.type == "identifier" else source.lazy_reference(loader)
        declaration = source.declaration(reference, owner)
        if declaration.start_byte in namespaces:
            raise ValueError("Locale languages share the same export namespace")
        namespaces.add(declaration.start_byte)
        initial = declaration.child_by_field_name("value")
        if initial.type == "object" and not initial.named_children:
            matches = []
            for call, target, dictionary in source.exports:
                if target.text != reference.text or source.scope(call) != owner:
                    continue
                if source.declaration(target, owner) == declaration:
                    matches.append(dictionary)
            if len(matches) != 1:
                raise ValueError("Locale export namespace is not unique: " + locale)
            dictionary, getters = matches[0], True
        else:
            arguments = source.global_call(initial, "Object", "freeze")
            if len(arguments) != 1 or arguments[0].type != "object":
                raise ValueError("Locale frozen namespace is not a complete literal object")
            dictionary, getters = arguments[0], False
        values, resolved = source.values(dictionary, owner, getters=getters)
        dictionaries[locale] = values
        dictionary_owners.append({"locale": locale, "namespace": reference.text.decode(),
                                  "loader": source.region(loader),
                                  "export_object": source.region(dictionary), "resolved_exports": resolved})
    factory = owner.parent if owner.parent is not None else owner
    return {"values": dictionaries, "locale_order": [locale for locale, _ in entries],
            "routing_owner": source.region(route), "complete_factory": source.region(factory),
            "dictionaries": dictionary_owners, "javascript_executed": False}


def locale_chunk_exports(raw, keys=None, *, absent_keys=()):
    """Read a complete translation chunk's actual webpack export-parameter owner."""
    source = _LocaleSource(raw)
    wanted = None if keys is None else set(keys)
    if wanted is not None and (not wanted or len(wanted) != len(keys)):
        raise ValueError("Locale keys must be nonempty and unique")
    absent = set(absent_keys)
    if (len(absent) != len(absent_keys) or (absent and wanted is None)
            or not absent <= (wanted or set())):
        raise ValueError("Absent locale keys must be unique selected keys")
    candidates = []
    for call, target, dictionary in source.exports:
        owner = source.scope(call)
        factory = owner.parent
        if factory is None or factory.type not in {"arrow_function", "function_expression"}:
            continue
        parameters = factory.child_by_field_name("parameters")
        parameters = parameters.named_children if parameters is not None else []
        require = call.child_by_field_name("function").child_by_field_name("object")
        if (len(parameters) != 3 or any(parameter.type != "identifier" for parameter in parameters)
                or target.text != parameters[1].text or require.text != parameters[2].text):
            continue
        keys_in_owner = {source.property_key(pair.child_by_field_name("key"), owner)
                         for pair in dictionary.named_children if pair.type == "pair"}
        if wanted is None or wanted - absent <= keys_in_owner:
            candidates.append((dictionary, owner, factory))
    if len(candidates) != 1:
        raise ValueError(f"Locale chunk export owner is not unique: {len(candidates)}")
    dictionary, owner, factory = candidates[0]
    values, resolved = source.values(dictionary, owner, getters=True)
    if absent & values.keys():
        raise ValueError("Declared absent locale key is present in the current source")
    if wanted is not None:
        values = {key: value for key, value in values.items() if key in wanted}
        resolved = [row for row in resolved if row["key"] in wanted]
    return {"values": values, "absent_keys": sorted(absent), "owner": source.region(dictionary),
            "complete_factory": source.region(factory), "resolved_exports": resolved,
            "javascript_executed": False}




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
            elif node.type == "for_in_statement":
                kind = node.child_by_field_name("kind")
                binding = node.child_by_field_name("left")
                if kind is None or kind.type not in {"const", "let", "var"} or binding is None:
                    continue
                # The grammar places these declarations directly in `left`.
                # Iteration values require analysis of the actual collection.
                target = owner(node, function_scoped=True) if kind.type == "var" else node
                descendants = [binding]
                while descendants:
                    candidate = descendants.pop()
                    if candidate.type in {"identifier", "shorthand_property_identifier_pattern"}:
                        definitions.setdefault(key(target), []).append((
                            candidate.text.decode("utf-8"), node, "iteration_requires_value_analysis"))
                    elif candidate.type in {"pair_pattern", "assignment_pattern", "object_assignment_pattern"}:
                        nested = candidate.child_by_field_name(
                            "value" if candidate.type == "pair_pattern" else "left")
                        if nested is not None:
                            descendants.append(nested)
                    elif candidate.type in {"object_pattern", "array_pattern", "rest_pattern"}:
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
        self.named_definitions = {}
        for scope_key, entries in definitions.items():
            for item in entries:
                self.named_definitions.setdefault(scope_key, {}).setdefault(item[0], []).append(item)

    def resolve(self, name, reference_byte):
        raw, tree, definitions = self.raw, self.tree, self.definitions
        key = self.key
        if not 0 <= reference_byte < len(raw):
            raise ValueError("Binding reference has invalid byte boundary")
        current = tree.root_node.descendant_for_byte_range(reference_byte, reference_byte)
        while current is not None:
            candidates = self.named_definitions.get(key(current), {}).get(name, [])
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


def installed_frontend_provenance(input_dir, official_input, audit):
    """Verify cached bodies and compare snapshots without promoting cache scope."""
    import frontend
    input_dir = a.resolve(str(input_dir), ".analysis/inputs/frontend")
    official_input = a.resolve(str(official_input), ".analysis/inputs/frontend")
    acquired_path = input_dir / "acquisition.json"
    acquired = json.loads(audit.artifact(acquired_path))
    if (acquired.get("source") != ".analysis/inputs/installed-current/acquisition.json"
            or acquired.get("source_current_during_acquisition") is not False
            or acquired.get("target_execution") is not False or acquired.get("entry_url") is not None):
        raise ValueError("Installed frontend provenance is not a bounded cache snapshot")
    source_path = a.ROOT / acquired["source"]
    source = json.loads(audit.artifact(source_path))
    if source.get("target_execution") is not False or source.get("source_current_during_acquisition") is not False:
        raise ValueError("Installed source receipt does not establish a static cache snapshot")
    groups = [row for row in source["frontend_groups"] if row["input"] == a.rel(input_dir)]
    if len(groups) != 1 or groups[0]["files"] != len(acquired["files"]) or groups[0]["conflicts"] != 0:
        raise ValueError("Installed frontend group differs from its original acquisition")
    resource_keys = ("container", "body_offset", "body_bytes", "sha256", "canonical_url")
    resources = {}
    for row in source["resources"]:
        resources.setdefault(tuple(row[key] for key in resource_keys), []).append(row)
    containers = {row["path"]: row for row in source["files"]}
    verified, urls = [], set()
    for row in acquired["files"]:
        parsed = urllib.parse.urlsplit(row["canonical_url"])
        if (parsed.scheme != "https" or parsed.netloc not in {"apps.razer.com", "app-assets.razer.com"}
                or parsed.query or parsed.fragment or row["url"] != row["canonical_url"]
                or row["url"] in urls or row.get("cache_crc_verified") is not True
                or row.get("deployment_current_verified") is not False):
            raise ValueError("Installed cached resource has an invalid source identity")
        urls.add(row["url"])
        matches = resources.get(tuple(row[key] for key in resource_keys), [])
        matches = [item for item in matches if item["url"] == row["cached_url"]
                   and item["container_sha256"] == row["container_sha256"]]
        if len(matches) != 1:
            raise ValueError("Cached frontend body has no unique installed-resource origin")
        origin = matches[0]
        body, retained = audit.read(row), audit.read(origin)
        container = containers[row["container"]]
        if container["sha256"] != row["container_sha256"] or origin.get("cache_crc_verified") is not True:
            raise ValueError("Cached frontend container identity or original CRC receipt differs")
        raw = audit.read(container)
        offset, length = row["body_offset"], row["body_bytes"]
        if (type(offset) is not int or type(length) is not int or offset < 0 or length != len(body)
                or offset + length > len(raw) or raw[offset:offset + length] != body or body != retained):
            raise ValueError("Cached frontend response bytes differ from the actual container slice")
        verified.append(row)
    official_path = official_input / "acquisition.json"
    official = json.loads(audit.artifact(official_path))
    selected = frontend.current_source_selection(official["entry_url"], audit, candidates=[official_path])
    official_files = {row["url"]: row for row in official["files"] if row.get("path")}
    same, extra, different, manifests = 0, 0, [], []
    for cached in verified:
        current = official_files.get(cached["url"])
        if current is None:
            extra += 1
            continue
        equal = all(cached[key] == current[key] for key in ("bytes", "sha256"))
        if equal:
            same += 1
        else:
            different.append({"url": cached["url"],
                "cached": {key: cached[key] for key in ("path", "bytes", "sha256")},
                "official": {key: current[key] for key in ("path", "bytes", "sha256")}})
        if Path(urllib.parse.urlsplit(cached["url"]).path).name in {"index.html", "manifest.json", "installer-manifest.json"}:
            manifests.append({"url": cached["url"], "same_bytes": equal,
                              "cached_sha256": cached["sha256"], "official_sha256": current["sha256"]})
    return {"scope": "installed_official_cache_snapshot; not_a_live_deployment",
            "cache_acquisition": audit.artifacts[a.rel(acquired_path)],
            "installed_acquisition": audit.artifacts[a.rel(source_path)],
            "official_acquisition": selected["acquisition"], "official_source_set": selected["name"],
            "cache_bodies_verified": len(verified), "cache_containers_verified": len({row["container"] for row in verified}),
            "same_url_and_bytes": same, "cached_without_official_counterpart": extra,
            "different_bytes": different, "entry_and_manifest_comparisons": manifests,
            "original_crc_receipts_verified": True, "crc_recomputed_by_this_review": False,
            "cache_promoted_to_current_deployment": False, "target_execution": False}


def host_frontend_entry_provenance(audit):
    """Trace the actual dashboard route through byte-verified host originals."""
    acquisition_path = a.ROOT / ".analysis/inputs/host-current/acquisition.json"
    inspection_path = a.ROOT / ".analysis/results/host-current/inspection.json"
    capture_path = a.ROOT / ".analysis/results/host-current/javascript/capture.json"
    acquired = json.loads(audit.artifact(acquisition_path))
    inspection = json.loads(audit.artifact(inspection_path))
    capture = json.loads(audit.artifact(capture_path))
    packages = [row for row in acquired["files"] if row.get("kind") == "official_stable_host_package"
                and row.get("declaration", {}).get("name") == "Razer App Engine"]
    if (len(packages) != 1 or acquired.get("target_execution") is not False
            or capture.get("input_bytes_unchanged") is not True or capture.get("target_execution") is not False):
        raise ValueError("Host route has no unique static official App Engine provenance")
    package = packages[0]
    audit.read(package)
    containers = [row for row in inspection["containers"] if row["input"] == package["path"]
                  and row["sha256"] == package["sha256"]]
    if (len(containers) != 1 or containers[0].get("retained_outputs_verified") is not True
            or containers[0].get("input_bytes_unchanged") is not True):
        raise ValueError("Host route package extraction identity is not verified")
    archives = [row for row in containers[0]["files"] if row["path"].endswith("/resources/app.asar")]
    if len(archives) != 1:
        raise ValueError("Host route package has no unique ASAR original")
    archive = archives[0]
    archive_raw = audit.read(archive)
    sources = {}
    for relative in ("electron/main.js", "electron/constants.js", "electron/lib/getAppHost.js"):
        rows = [row for row in capture["inputs"] if row["path"].endswith("/" + relative)]
        if len(rows) != 1:
            raise ValueError("Host route original is absent or ambiguous: " + relative)
        row = rows[0]
        if (row.get("container_sha256") != archive["sha256"] or row.get("unpacked") is not False
                or row.get("integrity_verified") is not True
                or row["container"] not in archive.get("verified_installed_aliases", [])):
            raise ValueError("Host route ASAR alias does not match the official package")
        raw = audit.read(row)
        offset = row["offset"]
        if type(offset) is not int or offset < 0 or archive_raw[offset:offset + len(raw)] != raw:
            raise ValueError("Host route original differs from the official ASAR byte slice")
        sources[relative] = row
    main, constants, resolver = (sources[name] for name in
        ("electron/main.js", "electron/constants.js", "electron/lib/getAppHost.js"))
    owners = {}
    for role, source, needle, kinds in (
            ("host_default_official_origin", main, 'Re="https://apps.razer.com"', {"variable_declarator"}),
            ("host_origin_initialization", main, '{gAppHost:ze,appHostCmdline:je}=I(Me,Re)', {"variable_declarator"}),
            ("host_route_constants_import", main, '{razerAppPathEnum:R,', {"variable_declarator"}),
            ("host_origin_resolver_import", main, '{getAppHost:I}', {"variable_declarator"}),
            ("host_actual_window_consumer_import", main, 'createWindow:$o', {"variable_declarator"}),
            ("host_dashboard_path", constants, 'SYNAPSE_PATH:"/synapse/dashboard"', {"pair"}),
            ("host_complete_application_host_resolver", resolver, 'module.exports.getAppHost=', {"assignment_expression"}),
            ("host_complete_app_launcher", main, 'n=`${ze}${R.SYNAPSE_PATH}/`', {"function_declaration"}),
            ("host_dashboard_launch_branch", main, 'n=`${ze}${R.SYNAPSE_PATH}/`', {"switch_case"}),
            ("host_actual_window_creation_call", main, '$o(n,e,i)', {"call_expression"})):
        owners[role] = audit.retain(role, source, needle, kinds)
    main_raw = audit.read(main)
    bindings = {name: audit.binding("host_route_lexical_binding_" + name, main, name, main_raw.index(needle))
                for name, needle in (("R", b"R.SYNAPSE_PATH"), ("I", b"I(Me,Re)"), ("$o", b"$o(n,e,i)"))}
    if any(row["resolution"] != "destructuring_requires_value_analysis" for row in bindings.values()):
        raise ValueError("Host route imports changed their actual lexical bindings")
    installer = [row for row in acquired["files"]
                 if row.get("url") == "https://apps.razer.com/synapse/dashboard/installer-manifest.json"]
    if len(installer) != 1:
        raise ValueError("Host dashboard installer manifest is absent or ambiguous")
    installer_document = json.loads(audit.read(installer[0]))
    return {"entry_url": "https://apps.razer.com/synapse/dashboard/",
            "scope": "default_official_origin_with_actual_synapse_launcher; command_line_host_overrides_are_separate",
            "acquisition": audit.artifacts[a.rel(acquisition_path)],
            "inspection": audit.artifacts[a.rel(inspection_path)], "javascript_capture": audit.artifacts[a.rel(capture_path)],
            "package": {key: package[key] for key in ("path", "bytes", "sha256")},
            "package_version": package["declaration"]["version"],
            "official_asar": {key: archive[key] for key in ("path", "bytes", "sha256")},
            "source_originals": [{key: row[key] for key in ("path", "bytes", "sha256")} for row in sources.values()],
            "owner_ids": owners, "owners": [row for row in audit.owners.values()
                if any(row["id"] in ids for ids in owners.values())], "lexical_import_bindings": bindings,
            "host_installer_manifest": {"original": {key: installer[0][key] for key in ("path", "bytes", "sha256")},
                                        "document": installer_document},
            "root_route_is_actual_entry": False, "target_execution": False}


def frontend_deployment_sources(acquired):
    """Separate actual HTML includes from manifest candidates and retained files.

    This resolves deployment references, not React reachability. In particular,
    an old main bundle retained beside the current deployment must not acquire
    current-source status merely because its bytes still match its old receipt.
    """
    if (acquired.get("entry_url") is None
            and acquired.get("source") == ".analysis/inputs/installed-current/acquisition.json"):
        return {"entry_url": None, "entry_response_url": None, "entry_is_html": False,
                "entry_original": None, "manifests": [], "sources": [],
                "failures": [{"kind": "installed_cache_current_deployment_unverified",
                              "source": acquired["source"]}],
                "inline_scripts": [], "inlined_manifest_entrypoints": [], "inline_entrypoint_comparisons": [],
                "retained_without_current_reference": [{"url": row.get("url"), "path": row["path"],
                    "status": "installed_cache_snapshot_requires_official_source_comparison"}
                    for row in acquired.get("files", []) if row.get("path")
                    and Path(urllib.parse.urlsplit(row.get("url", "")).path).suffix.lower() in {".js", ".mjs", ".css"}],
                "unresolved_acquisition_references": acquired.get("unresolved", []),
                "source_semantics_complete": False, "target_execution": False}
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
        if not url or Path(urllib.parse.urlsplit(url).path).suffix.lower() not in {".js", ".mjs", ".css"}:
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
    entry_media = files.get(entry, {}).get("content_type", "").split(";", 1)[0].strip().lower()
    entry_is_html = entry_raw is not None and (entry_media in {"text/html", "application/xhtml+xml"}
        or Path(urllib.parse.urlsplit(entry or "").path).suffix.lower() in {".html", ".htm"}
        or re.match(rb"\s*(?:\xef\xbb\xbf)?\s*<(?:!doctype\s+html|html|head|body|script|link|meta|base|title|div|noscript|style)(?:\s|/?>)", entry_raw, re.I) is not None)
    entry_response = a.artifact_url(entry or "", files.get(entry, {}).get("final_url", entry or ""))
    if (entry_response is None or urllib.parse.urlsplit(entry_response).netloc != urllib.parse.urlsplit(entry or "").netloc):
        failures.append({"kind": "deployment_entry_redirect_outside_origin", "url": entry,
                         "final_url": files.get(entry, {}).get("final_url")})
        entry_response = entry
    parsed = EntryReferences(entry_raw.decode("utf-8") if entry_raw is not None else "")
    if entry_is_html:
        parsed.feed(parsed.text)
        base = urllib.parse.urljoin(entry_response, parsed.base) if parsed.base else entry_response
        for literal, role in parsed.includes:
            select(a.artifact_url(base, literal), role, entry, literal)
    elif entry_raw is not None:
        select(entry, "standalone_resource_entry", entry, entry)
    manifests, inline_entrypoints, inline_comparisons, inline_audit = [], [], [], None

    def inlined_entrypoint(target):
        nonlocal inline_audit
        if Path(urllib.parse.urlsplit(target or "").path).suffix.lower() != ".js":
            return False
        raw = read(target, required=False)
        if raw is None:
            return False
        import frontend
        from host import HostLocalAudit
        for inline in parsed.inline:
            attributes = inline["attributes"]
            if (attributes.get("type", "").strip().lower() not in {"", "text/javascript", "application/javascript"}
                    or "nomodule" in attributes):
                continue
            if inline_audit is None:
                inline_audit = HostLocalAudit()
            compared = frontend.inline_script_equivalence(raw, inline["body"].encode("utf-8"), inline_audit.parser)
            inline_comparisons.append({"url": target, "original": originals[target],
                "entry_original": originals[entry], "inline_start_byte": inline["start_byte"],
                "inline_end_byte": inline["end_byte"], "inline_sha256": inline["sha256"],
                "parser_versions": inline_audit.parser_versions, **compared})
            if not compared["equivalent"]:
                continue
            proof = {"url": target, "original": originals[target], "entry_original": originals[entry],
                "inline_start_byte": inline["start_byte"], "inline_end_byte": inline["end_byte"],
                "inline_sha256": inline["sha256"], "attributes": attributes,
                "parser_versions": inline_audit.parser_versions, **compared}
            inline_entrypoints.append(proof)
            select(target, "manifest_entrypoint_inlined_equivalent", entry, inline["sha256"])
            return True
        return False
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
                if entry_is_html and target in selected and not any(ref["role"] in {"html_script_include", "html_stylesheet_include"}
                                                  for ref in selected[target]["references"]) and not inlined_entrypoint(target):
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
                                   else "inline_equivalent_original" if "manifest_entrypoint_inlined_equivalent" in roles
                                   else "standalone_entry_requires_importer_audit" if "standalone_resource_entry" in roles
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
    return {"entry_url": entry, "entry_response_url": entry_response, "entry_is_html": entry_is_html,
            "entry_original": originals.get(entry), "manifests": manifests,
            "sources": list(selected.values()), "failures": failures, "inline_scripts": parsed.inline,
            "inlined_manifest_entrypoints": inline_entrypoints,
            "inline_entrypoint_comparisons": inline_comparisons,
            "retained_without_current_reference": [{"url": url, "path": item["path"],
                "status": "not_current_implementation_evidence"} for url, item in files.items()
                if item.get("path") and url not in selected
                and Path(urllib.parse.urlsplit(url).path).suffix.lower() in {".js", ".mjs", ".css"}],
            "unresolved_acquisition_references": acquired.get("unresolved", []),
            "source_semantics_complete": False, "target_execution": False}


_KEYBOARD_RUNTIME_ROUTE = "https://apps.razer.com/synapse/assets/js/keyboard-nav/rz-accessibility.min.js"
_KEYBOARD_LOADER = (
    'function(){if(window.RzAccessibility)return Promise.resolve(window.RzAccessibility);'
    'if(pending)return pending;const e=window.__RZ_A11Y_SRC__||'
    '"/synapse/assets/js/keyboard-nav/rz-accessibility.min.js";'
    'return pending=new Promise((t,o)=>{const n=document.createElement("script");'
    'n.src=e,n.async=!0,n.setAttribute("data-rz-accessibility","");'
    'const i=setTimeout(()=>r("timed out after 10000ms"),1e4),'
    'r=t=>{clearTimeout(i),pending=null,n.remove(),'
    'o(new Error("[rz-accessibility] "+t+": "+e))};'
    'n.addEventListener("load",()=>{clearTimeout(i),window.RzAccessibility?'
    't(window.RzAccessibility):r("loaded but window.RzAccessibility is not defined")}),'
    'n.addEventListener("error",()=>r("failed to load")),document.head.appendChild(n)}),pending}'
)
_KEYBOARD_EFFECT = (
    '()=>{let e=!1;return load().then(t=>{e||t&&t.init(document.getElementById("root"))})'
    '.catch(e=>{console.warn("[rz-accessibility] failed to load keyboard-navigation runtime:",e)}),'
    '()=>{e=!0}}'
)


def _keyboard_component_owner(node):
    while node is not None:
        if node.type in {"function_declaration", "class_declaration"}:
            return node
        if node.type == "variable_declarator":
            value = node.child_by_field_name("value")
            if value is not None and value.type in {"arrow_function", "function_expression", "class"}:
                return node
        node = node.parent
    raise ValueError("Keyboard navigation has no complete named component owner")


def _keyboard_unwrap(node):
    while node.type in {"parenthesized_expression", "sequence_expression"}:
        node = node.named_children[-1]
    return node


def _keyboard_range(audit, source, node):
    raw = audit.read(source)[node.start_byte:node.end_byte]
    return {"original": audit.sources[source["path"]], "syntax": node.type,
            "byte_start": node.start_byte, "byte_end": node.end_byte,
            "body_sha256": hashlib.sha256(raw).hexdigest()}


def _keyboard_startup_path(audit, source, node):
    """Require an invoked bootstrap or promise callback at each function boundary."""
    calls = []
    current = node
    while current is not None:
        if current.type in {"arrow_function", "function_expression", "function_declaration", "method_definition"}:
            caller = current.parent
            while caller is not None and caller.type == "parenthesized_expression":
                caller = caller.parent
            if caller is not None and caller.type == "arguments":
                call = caller.parent
                function = call.child_by_field_name("function") if call.type == "call_expression" else None
                if (function is None or function.type != "member_expression"
                        or function.child_by_field_name("property").text not in {b"then", b"catch", b"finally"}):
                    raise ValueError("Keyboard root is inside a callback without an audited startup invocation")
            elif caller is not None and caller.type == "call_expression" and _keyboard_unwrap(caller.child_by_field_name("function")) == current:
                call = caller
            else:
                raise ValueError("Keyboard root is inside an uninvoked function owner")
            calls.append(_keyboard_range(audit, source, call))
            current = call
        current = current.parent
    return calls


def _keyboard_require_binding(audit, source, function):
    import host
    return host.javascript_require_binding(audit, source, function)


def _keyboard_import_namespace(audit, originals, source, namespace):
    import host
    return host.javascript_import_namespace(audit, originals, source, namespace, role="keyboard_root_import")


def _keyboard_reactdom_external(audit, originals, source, namespace):
    provider, value, provider_owner = _keyboard_import_namespace(audit, originals, source, namespace)
    body = value.child_by_field_name("body")
    params = value.child_by_field_name("parameters") or value.child_by_field_name("parameter")
    params = params.named_children if params.type == "formal_parameters" else [params]
    statements = [statement for statement in body.named_children
                  if not (statement.type == "expression_statement" and statement.named_children[0].text == b'"use strict"')]
    if len(params) != 1 or len(statements) != 1 or statements[0].type != "expression_statement":
        raise ValueError("Keyboard root provider is not the actual ReactDOM external")
    assignment = statements[0].named_children[0]
    if (assignment.type != "assignment_expression" or assignment.child_by_field_name("left").text != params[0].text + b".exports"
            or assignment.child_by_field_name("right").text != b"ReactDOM"):
        raise ValueError("Keyboard root provider resolves a different global")
    return provider_owner


def _keyboard_create_root(audit, originals, source, node):
    import host
    function = _keyboard_unwrap(node)
    if function.type != "member_expression":
        raise ValueError("Keyboard root has no concrete createRoot binding")
    if function.child_by_field_name("property").text == b"createRoot":
        return []
    imported, factory, alias_owner = _keyboard_import_namespace(
        audit, originals, source, function.child_by_field_name("object"))
    params = factory.child_by_field_name("parameters").named_children
    if len(params) != 3:
        raise ValueError("Keyboard root alias factory has no exports parameter")
    aliases = []
    for candidate in host._help_nodes(factory):
        if candidate.type != "assignment_expression":
            continue
        target = candidate.child_by_field_name("left")
        if (target.type != "member_expression" or target.child_by_field_name("property").text != function.child_by_field_name("property").text
                or target.child_by_field_name("object").text != params[1].text):
            continue
        value = candidate.child_by_field_name("right")
        if value.type != "member_expression" or value.child_by_field_name("property").text != b"createRoot":
            raise ValueError("Keyboard root alias exports a different operation")
        aliases.append(value.child_by_field_name("object"))
    if len(aliases) != 1:
        raise ValueError("Keyboard root alias has no unique createRoot export")
    return [alias_owner, _keyboard_reactdom_external(audit, originals, imported, aliases[0])]


def _keyboard_legacy_root(audit, originals, source, receiver):
    import host
    if receiver.type != "call_expression" or receiver.child_by_field_name("arguments").named_child_count:
        raise ValueError("Keyboard legacy render has no audited ReactDOM receiver")
    getter = receiver.child_by_field_name("function")
    if getter.type != "identifier":
        raise ValueError("Keyboard legacy ReactDOM getter has no stable binding")
    binding = audit.binding("keyboard_reactdom_getter", source, getter.text.decode(), getter.start_byte)
    declaration = host._help_binding_node(audit, source, binding)
    value = declaration.child_by_field_name("value")
    if value is None or value.type != "call_expression":
        raise ValueError("Keyboard legacy root is not the real imported ReactDOM getter")
    function = value.child_by_field_name("function")
    args = value.child_by_field_name("arguments").named_children
    if function.type != "member_expression" or function.child_by_field_name("property").text != b"n" or len(args) != 1:
        raise ValueError("Keyboard legacy ReactDOM import has changed its interop call")
    require = function.child_by_field_name("object")
    actual_require = _keyboard_require_binding(audit, source, require)
    interop = []
    for hit in re.finditer(rb"(?<![\w$])" + re.escape(require.text) + rb"\.n\s*=", audit.read(source)):
        node = audit.tree(source).root_node.descendant_for_byte_range(hit.start(), hit.end() - 1)
        while node is not None and node.type != "assignment_expression":
            node = node.parent
        if node is None:
            continue
        resolved = audit.binding("keyboard_reactdom_interop_require", source, require.text.decode(), hit.start())
        if resolved.get("start_byte") != actual_require.get("start_byte") or resolved.get("end_byte") != actual_require.get("end_byte"):
            continue
        expected = 'interop=e=>{var t=e&&e.__esModule?()=>e.default:()=>e;return require.d(t,{a:t}),t}'
        if (host.help_runtime_shape(audit.parser, "interop=" + node.child_by_field_name("right").text.decode())
                != host.help_runtime_shape(audit.parser, expected)):
            raise ValueError("Keyboard legacy root uses a different interop getter")
        interop.append(audit.retain_range("keyboard_reactdom_interop", source, node.start_byte, node.end_byte, node.type))
    if len(interop) != 1:
        raise ValueError("Keyboard legacy root has no unique actual interop implementation")
    return [binding["owner_id"], *interop, _keyboard_reactdom_external(audit, originals, source, args[0])]


def _keyboard_dom_target(audit, originals, source, value):
    import host
    original, target = host._help_runtime_value(audit, originals, source, value)
    if target.text != b'document.getElementById("root")':
        raise ValueError("Keyboard root uses an unaudited DOM target")
    return _keyboard_range(audit, original, target)


def _keyboard_root_call(audit, originals, source, node):
    current = node
    while current is not None:
        if current.type == "call_expression":
            function = current.child_by_field_name("function")
            if function.type == "member_expression" and function.child_by_field_name("property").text == b"render":
                created = function.child_by_field_name("object")
                render_args = current.child_by_field_name("arguments").named_children
                if len(render_args) == 2:
                    aliases = _keyboard_legacy_root(audit, originals, source, created)
                    target = _keyboard_dom_target(audit, originals, source, render_args[1])
                    return {"owner": audit.retain_range("keyboard_root_render", source,
                                current.start_byte, current.end_byte, current.type),
                            "reactdom_legacy_aliases": aliases, "dom_target": target,
                            "startup_invocations": _keyboard_startup_path(audit, source, current)}
                if created.type == "call_expression":
                    factory = created.child_by_field_name("function")
                    aliases = _keyboard_create_root(audit, originals, source, factory)
                    arguments = created.child_by_field_name("arguments").named_children
                    if len(arguments) != 1:
                        raise ValueError("Keyboard root uses an unaudited DOM target")
                    target = _keyboard_dom_target(audit, originals, source, arguments[0])
                    return {"owner": audit.retain_range("keyboard_root_render", source,
                                current.start_byte, current.end_byte, current.type),
                            "create_root_aliases": aliases, "dom_target": target,
                            "startup_invocations": _keyboard_startup_path(audit, source, current)}
        current = current.parent
    raise ValueError("Keyboard wrapper has no actual createRoot/render consumer")


def keyboard_navigation_loaders(audit, originals, main):
    import host
    raw = audit.read(main)
    owners = {}
    for hit in re.finditer(rb"__RZ_A11Y_SRC__", raw):
        property_node = audit.tree(main).root_node.descendant_for_byte_range(hit.start(), hit.end() - 1)
        if property_node.type != "property_identifier":
            continue
        member = property_node.parent
        if member.type != "member_expression" or member.child_by_field_name("object").text != b"window":
            raise ValueError("Keyboard loader source override has an unaudited owner")
        node = member
        while node is not None and node.type not in {"function_expression", "arrow_function", "function_declaration"}:
            node = node.parent
        if node is None:
            raise ValueError("Keyboard source override has no complete loader function")
        owners[(node.start_byte, node.end_byte)] = node
    results = []
    for loader in owners.values():
        retained = audit.retain_range("keyboard_loader", main, loader.start_byte, loader.end_byte, loader.type)
        if (host.help_runtime_shape(audit.parser, "load=" + loader.text.decode())
                != host.help_runtime_shape(audit.parser, "load=" + _KEYBOARD_LOADER)):
            raise ValueError("Keyboard loader differs from the complete cache, timeout and error contract")
        call = loader.parent
        if call.type != "call_expression" or call.child_by_field_name("function") != loader or call.child_by_field_name("arguments").named_child_count:
            raise ValueError("Keyboard loader is not immediately invoked by its effect")
        effect = call.parent
        while effect is not None and effect.type not in {"arrow_function", "function_expression"}:
            effect = effect.parent
        if effect is None:
            raise ValueError("Keyboard loader has no complete effect owner")
        body = effect.text[:loader.start_byte - effect.start_byte] + b"load" + effect.text[loader.end_byte - effect.start_byte:]
        if (host.help_runtime_shape(audit.parser, "effect=" + body.decode())
                != host.help_runtime_shape(audit.parser, "effect=" + _KEYBOARD_EFFECT)):
            raise ValueError("Keyboard effect changed initialization, failure or unmount cancellation")
        arguments = effect.parent
        if arguments.type != "arguments" or arguments.parent.type != "call_expression":
            raise ValueError("Keyboard effect is not passed to a lifecycle hook")
        hook = arguments.parent
        function = _keyboard_unwrap(hook.child_by_field_name("function"))
        args = arguments.named_children
        if (function.type != "member_expression" or function.child_by_field_name("property").text != b"useEffect"
                or len(args) != 2 or args[0] != effect or args[1].type != "array" or args[1].named_child_count):
            raise ValueError("Keyboard effect has an unaudited hook or dependency list")
        cache_returns = [node for node in host._help_nodes(loader) if node.type == "return_statement"
                         and node.named_child_count == 1 and node.named_children[0].type == "identifier"]
        if len(cache_returns) != 1:
            raise ValueError("Keyboard promise cache has no unique source owner")
        reference = cache_returns[0].named_children[0]
        cache = audit.binding("keyboard_loader_cache", main, reference.text.decode(), reference.start_byte)
        cache_node = host._help_binding_node(audit, main, cache)
        if (cache_node.type != "variable_declarator" or cache_node.child_by_field_name("value") is None
                or cache_node.child_by_field_name("value").text != b"null"
                or effect.start_byte <= cache_node.start_byte < effect.end_byte):
            raise ValueError("Keyboard loader cache does not retain the audited initial null owner")
        component = _keyboard_component_owner(hook)
        component_owner = audit.retain_range("keyboard_wrapper", main,
                                            component.start_byte, component.end_byte, component.type)
        flow = host.javascript_component_mounts(audit, originals, main, component, role="keyboard")
        roots = []
        for mount in flow["mounts"]:
            source = next(item for item in originals if item["path"] == mount["original"]["path"])
            owner = audit.owners[mount["owner"]]
            element = audit.tree(source).root_node.descendant_for_byte_range(owner["byte_start"], owner["byte_end"] - 1)
            roots.append({"mount": mount, **_keyboard_root_call(audit, originals, source, element)})
        results.append({"loader": retained, "effect": audit.retain_range("keyboard_effect", main,
                            effect.start_byte, effect.end_byte, effect.type),
                        "hook": audit.retain_range("keyboard_hook", main, hook.start_byte, hook.end_byte, hook.type),
                        "wrapper": component_owner, "cache": cache["owner_id"], "roots": roots,
                        "binding_links": flow["binding_links"], "default_runtime_route": _KEYBOARD_RUNTIME_ROUTE,
                        "late_completion_guard": True, "timeout_ms": 10000,
                        "failure_resets_cache_and_removes_script": True,
                        "source_unmount_calls_runtime_destroy": False})
    return results


def keyboard_navigation_shared_runtime(audit, original):
    import host
    tree = audit.tree(original)
    declarations = [node for node in tree.root_node.named_children if node.type != "comment"]
    if len(declarations) != 1 or declarations[0].type != "variable_declaration" or declarations[0].named_child_count != 1:
        raise ValueError("Shared keyboard runtime has no unique complete top-level declaration")
    declaration = declarations[0].named_children[0]
    if declaration.type != "variable_declarator" or declaration.child_by_field_name("name").text != b"RzAccessibility":
        raise ValueError("Shared keyboard runtime exports a different browser identity")
    call = declaration.child_by_field_name("value")
    if call.type != "call_expression" or call.child_by_field_name("function").type != "function_expression":
        raise ValueError("Shared keyboard runtime is not the current complete IIFE")
    function = call.child_by_field_name("function")
    params = function.child_by_field_name("parameters").named_children
    args = call.child_by_field_name("arguments").named_children
    if len(params) != 1 or params[0].type != "identifier" or len(args) != 1 or args[0].text != b"{}":
        raise ValueError("Shared keyboard runtime has changed its export object initialization")
    exports = {}
    for node in host._help_nodes(function):
        if node.type != "assignment_expression":
            continue
        target = node.child_by_field_name("left")
        if target.type != "member_expression" or target.child_by_field_name("object").text != params[0].text:
            continue
        reference = target.child_by_field_name("object")
        binding = audit.binding("keyboard_runtime_export_object", original, reference.text.decode(), reference.start_byte)
        if binding.get("start_byte") != params[0].start_byte or binding.get("end_byte") != params[0].end_byte:
            continue
        name = target.child_by_field_name("property").text.decode()
        value = node.child_by_field_name("right")
        if name in exports or value.type != "identifier":
            raise ValueError("Shared keyboard runtime has a repeated or dynamic export")
        implementation = audit.binding("keyboard_runtime_" + name, original, value.text.decode(), value.start_byte)
        if implementation["resolution"] != "declaration":
            raise ValueError("Shared keyboard runtime export has no complete lexical implementation")
        exports[name] = implementation["owner_id"]
    required = {"init", "openGuide", "closeGuide", "isGuideOpen", "isInitialized", "default"}
    if set(exports) != required:
        raise ValueError("Shared keyboard runtime export set differs from its audited API")
    return {"iife": audit.retain_range("keyboard_shared_runtime", original,
                        declaration.start_byte, declaration.end_byte, declaration.type),
            "exports": exports, "original": audit.sources[original["path"]], "target_execution": False}


_KEYBOARD_CHROMA_COMPONENT = (
    'function Component(e){const{app:t}=e;let s="";switch(t){'
    'case"chroma":s=keys.IFV;break;case"synapse":s=keys.njo;break;default:s=""}'
    'return(0,jsx.jsx)(Block,{title:keys.r6A,hasSwitch:!1,children:(0,jsx.jsx)("div",'
    '{className:"setting-block",children:(0,jsx.jsxs)("div",{className:"setting-row",children:['
    '(0,jsx.jsx)("div",{className:"thx-btn test",onClick:()=>{var e;'
    'return null===(e=window.RzAccessibility)||void 0===e?void 0:e.openGuide()},'
    'role:"button",tabIndex:0,children:(0,jsx.jsx)(Translator.A,{text:keys.o1z})}),'
    '(0,jsx.jsx)("div",{className:"setting-name",children:(0,jsx.jsx)(Translator.A,{text:s})})]})})})}'
)
_KEYBOARD_TRANSLATE = (
    'get=(e,t={})=>{var s;const n={...t};let a=n._lang;if(null===dictionary)return"";'
    'a?delete n._lang:a=language(),dictionary[a]||(a=locales.Nm);'
    'let o=null!==(s=dictionary[a][e])&&void 0!==s?s:dictionary[locales.Nm][e];'
    'if(o){let e;for(const[t,s]of Object.entries(n))e=new RegExp(`{{${t}}}`,"g"),o=o.replace(e,s)}'
    'else console.debug("[getTextItem][%s]",e);const r=o||e;'
    'return t.a&&t.aEnd&&"string"===typeof r&&!r.includes("<a")?`${t.a}${r}${t.aEnd}`:r}'
)
_KEYBOARD_UPDATE_LANGUAGE = (
    'update=(e,t,s=()=>{})=>a=>{const o=language(e);'
    'null===dictionary&&a({type:actions.Ztp,payload:"unknown"});'
    'new Promise(e=>{t?t().then(({default:t})=>{providers=t,e(providers)}):e(providers)})'
    '.then(e=>{const t=[e[locales.Nm]()];o!==locales.Nm&&t.push(e[o]()),'
    'Promise.all(t).then(([e,r])=>{dictionary={[locales.Nm]:e},t.length>1&&(dictionary[o]=r),'
    'a({type:actions.Ztp,payload:o}),s()})})}'
)


def _keyboard_lazy_root_exports(audit, originals, main, roots):
    import host
    exports = []
    for root in roots:
        owner = audit.owners[root["owner"]]
        node = audit.tree(main).root_node.descendant_for_byte_range(owner["byte_start"], owner["byte_end"] - 1)
        callback = node
        while callback is not None and callback.type not in {"arrow_function", "function_expression"}:
            callback = callback.parent
        if callback is None:
            continue
        parameters = callback.child_by_field_name("parameters")
        if parameters is None or parameters.named_child_count != 1 or parameters.named_children[0].type != "object_pattern":
            continue
        fields = parameters.named_children[0].named_children
        if len(fields) != 1 or fields[0].type != "pair_pattern":
            continue
        key, value = fields[0].child_by_field_name("key"), fields[0].child_by_field_name("value")
        if key is None or value is None or value.type != "identifier":
            continue
        arguments = callback.parent
        if arguments.type != "arguments" or arguments.parent.type != "call_expression":
            continue
        then = arguments.parent.child_by_field_name("function")
        if then.type != "member_expression" or then.child_by_field_name("property").text != b"then":
            continue
        requested = then.child_by_field_name("object")
        if requested.type != "call_expression":
            continue
        previous = requested.child_by_field_name("function")
        calls = requested.child_by_field_name("arguments").named_children
        if (previous.type != "member_expression" or previous.child_by_field_name("property").text != b"then"
                or len(calls) != 1 or calls[0].type != "call_expression"):
            continue
        bound = calls[0]
        binding = bound.child_by_field_name("function")
        args = bound.child_by_field_name("arguments").named_children
        if (binding.type != "member_expression" or binding.child_by_field_name("property").text != b"bind"
                or len(args) != 2 or args[1].type != "number"):
            continue
        require = binding.child_by_field_name("object")
        if args[0].text != require.text:
            raise ValueError("Keyboard lazy root has a different require receiver")
        _keyboard_require_binding(audit, main, require)
        references = [ref for ref in host._help_nodes(node) if ref.type == "identifier" and ref.text == value.text
                      and ref.parent.type == "arguments" and ref.parent.named_children[0] == ref]
        if not references or any(audit.binding("keyboard_lazy_root_parameter", main, ref.text.decode(), ref.start_byte).get("start_byte") != value.start_byte
                                 for ref in references):
            raise ValueError("Keyboard lazy root does not render the imported export")
        module_id, export_name = int(args[1].text), key.text.decode()
        exports.append({"module_id": module_id, "export": export_name, "root": root["owner"],
                        "request": audit.retain_range("keyboard_lazy_root_request", main,
                                                     bound.start_byte, bound.end_byte, bound.type),
                        "provider": audit.webpack_export("keyboard_lazy_root_provider", originals, module_id, export_name)})
    if not exports:
        raise ValueError("Keyboard settings has no audited lazy root export")
    return exports


def _keyboard_component_paths(audit, originals, original, component, exports):
    import host
    pending, visited, paths, reached = [(original, component)], set(), [], False
    terminals = {(row["module_id"], row["export"]) for row in exports}
    while pending:
        source, current = pending.pop()
        identity = (source["path"], current.start_byte, current.end_byte)
        if identity in visited:
            continue
        visited.add(identity)
        owner = audit.retain_range("keyboard_guide_component", source, current.start_byte, current.end_byte, current.type)
        flow = host.javascript_component_mounts(audit, originals, source, current,
                                                role="keyboard_guide", terminal_exports=terminals)
        paths.append({"component": owner, **flow})
        reached |= bool(flow["terminal_exports"])
        for mount in flow["mounts"]:
            parent_source = next(row for row in originals if row["path"] == mount["original"]["path"])
            record = audit.owners[mount["owner"]]
            element = audit.tree(parent_source).root_node.descendant_for_byte_range(record["byte_start"], record["byte_end"] - 1)
            pending.append((parent_source, _keyboard_component_owner(element.parent)))
    if not reached:
        raise ValueError("Keyboard guide component has no path to the actual root export")
    return paths


def keyboard_navigation_translation_fallback(audit, originals, original, component):
    import host
    keys = {"r6A": "KEYBOARD_NAVIGATION", "o1z": "VIEW_ALL_SHORTCUTS",
            "IFV": "KEYBOARD_NAVIGATION_CHROMA_MSG", "njo": "KEYBOARD_NAVIGATION_SYNAPSE_MSG"}
    constants = {}
    for symbol, expected in keys.items():
        references = [node for node in host._help_nodes(component) if node.type == "member_expression"
                      and node.child_by_field_name("property").text.decode() == symbol]
        if len(references) != 1:
            raise ValueError("Keyboard guide text has no unique source constant: " + symbol)
        source, value = host._help_runtime_value(audit, originals, original, references[0])
        if value.type != "string" or json.loads(value.text) != expected:
            raise ValueError("Keyboard guide constant resolves to a different translation key")
        constants[expected] = _keyboard_range(audit, source, value)
    translators = []
    for call in host._help_nodes(component):
        if call.type != "call_expression":
            continue
        args = call.child_by_field_name("arguments").named_children
        if len(args) < 2 or args[0].type != "member_expression" or args[1].type != "object":
            continue
        if not any(pair.type == "pair" and pair.child_by_field_name("key").text == b"text" for pair in args[1].named_children):
            continue
        source, connected = host._help_runtime_value(audit, originals, original, args[0])
        if connected.type != "call_expression" or connected.child_by_field_name("arguments").named_child_count != 1:
            raise ValueError("Keyboard translation component has no connected state owner")
        source, translator = host._help_runtime_value(audit, originals, source, connected.child_by_field_name("arguments").named_children[0])
        translators.append((source, translator))
    unique = {(source["path"], node.start_byte, node.end_byte) for source, node in translators}
    if len(unique) != 1:
        raise ValueError("Keyboard guide rows do not use one verified translation component")
    source, translator = translators[0]
    readers = []
    for node in host._help_nodes(translator):
        if node.type == "call_expression":
            function = _keyboard_unwrap(node.child_by_field_name("function"))
            if function.type == "member_expression" and function.child_by_field_name("property").text == b"getTextItem":
                readers.append(host._help_runtime_value(audit, originals, source, function))
    if not readers or len({(row["path"], node.start_byte, node.end_byte) for row, node in readers}) != 1:
        raise ValueError("Keyboard translator has no unique actual getTextItem implementation")
    source, getter = readers[0]
    if (host.help_runtime_shape(audit.parser, "get=" + getter.text.decode())
            != host.help_runtime_shape(audit.parser, _KEYBOARD_TRANSLATE)):
        raise ValueError("Keyboard translation differs from the complete missing-language/key fallback")
    factory = host._help_factory(getter)
    if factory is None:
        raise ValueError("Keyboard translation reader has no complete module owner")
    update = audit.webpack_export("keyboard_language_update", originals, factory["module_id"], "updateLanguage")
    update_source = next(row for row in originals if row["path"] == update["original"]["path"])
    updater = host._help_binding_node(audit, update_source, update["binding"])
    if (host.help_runtime_shape(audit.parser, update["binding"]["body"])
            != host.help_runtime_shape(audit.parser, _KEYBOARD_UPDATE_LANGUAGE)):
        raise ValueError("Keyboard language publisher does not load the audited fallback dictionary")
    defaults = []
    for source_row, node in ((source, getter), (update_source, updater)):
        for member in host._help_nodes(node):
            if member.type == "member_expression" and member.child_by_field_name("property").text == b"Nm":
                constant_source, value = host._help_runtime_value(audit, originals, source_row, member)
                if value.type != "string" or json.loads(value.text) != "en":
                    raise ValueError("Keyboard translation fallback language is not the actual English constant")
                defaults.append(_keyboard_range(audit, constant_source, value))
    if not defaults:
        raise ValueError("Keyboard translation has no resolved default language")
    null_checks = [node for node in host._help_nodes(getter) if node.type == "binary_expression"
                   and node.child_by_field_name("left").type == "null"
                   and node.child_by_field_name("right").type == "identifier"]
    if len(null_checks) != 1:
        raise ValueError("Keyboard translation dictionary state is ambiguous")
    state = null_checks[0].child_by_field_name("right")
    read_binding = audit.binding("keyboard_language_dictionary", source, state.text.decode(), state.start_byte)
    writes = [node.child_by_field_name("left") for node in host._help_nodes(updater) if node.type == "assignment_expression"
              and node.child_by_field_name("left").type == "identifier" and node.child_by_field_name("right").type == "object"]
    if len(writes) != 1:
        raise ValueError("Keyboard language publisher has no unique dictionary write")
    write_binding = audit.binding("keyboard_language_dictionary_writer", update_source, writes[0].text.decode(), writes[0].start_byte)
    if (source["path"], read_binding.get("start_byte"), read_binding.get("end_byte")) != (
            update_source["path"], write_binding.get("start_byte"), write_binding.get("end_byte")):
        raise ValueError("Keyboard translation reader and language publisher use different dictionaries")
    return {"constants": constants, "translator": audit.retain_range("keyboard_translator", translators[0][0],
                translator.start_byte, translator.end_byte, translator.type),
            "get_text": audit.retain_range("keyboard_get_text", source, getter.start_byte, getter.end_byte, getter.type),
            "update_language": update, "dictionary": read_binding["owner_id"], "default_language": "en",
            "default_language_constants": defaults,
            "missing_dictionary": "empty_until_loaded", "missing_language": "english",
            "missing_key": "english_then_source_key"}


def keyboard_navigation_chroma_guide(audit, originals, main, loaders):
    import host
    components = {}
    for original in originals:
        raw = audit.read(original)
        for match in re.finditer(rb"openGuide", raw):
            node = audit.tree(original).root_node.descendant_for_byte_range(match.start(), match.end() - 1)
            if node.type != "property_identifier":
                continue
            component = _keyboard_component_owner(node)
            components[(original["path"], component.start_byte, component.end_byte)] = (original, component)
    if len(components) != 1:
        raise ValueError("Chroma keyboard guide has no unique complete component")
    original, component = next(iter(components.values()))
    if (host.help_runtime_shape(audit.parser, "component=" + component.text.decode())
            != host.help_runtime_shape(audit.parser, "component=" + _KEYBOARD_CHROMA_COMPONENT)):
        raise ValueError("Chroma keyboard guide changed its action, state or accessible button contract")
    exports = _keyboard_lazy_root_exports(audit, originals, main, [root for loader in loaders for root in loader["roots"]])
    paths = _keyboard_component_paths(audit, originals, original, component, exports)
    if not any(mount["props"].get("app") == '"chroma"' for path in paths for mount in path["mounts"]):
        raise ValueError("Keyboard guide does not receive the Chroma application branch at its actual mount")
    return {"component": audit.retain_range("keyboard_chroma_guide", original,
                        component.start_byte, component.end_byte, component.type),
            "root_exports": exports, "mount_paths": paths,
            "translation": keyboard_navigation_translation_fallback(audit, originals, original, component)}


def audit_keyboard_navigation(args):
    import concurrent.futures
    import frontend
    import host
    routes = frontend.current_acquisition_routes()
    observer = host.HostLocalAudit()
    observations = frontend.latest_deployment_observations(observer)
    for name in ("tools/evidence.py", "tools/host.py", "tools/frontend.py"):
        observer.artifact(a.ROOT / name)
    all_routes = sorted(route for route in routes if urllib.parse.urlsplit(route).path.endswith("/")
                        and not re.search(r"/products/\d+/mw/$", urllib.parse.urlsplit(route).path))
    products = set(getattr(args, "product", None) or ())
    requested = [route for route in all_routes if not products
                 or re.fullmatch(r"https://apps\.razer\.com/synapse/products/(\d+)/ui/", route)
                 and int(re.fullmatch(r"https://apps\.razer\.com/synapse/products/(\d+)/ui/", route)[1]) in products]
    if not requested or products and {int(re.search(r"/products/(\d+)/", route)[1]) for route in requested} != products:
        raise ValueError("Keyboard navigation query contains an unavailable product route")
    report = {"recorded_at_utc": a.stamp(), "tool": observer.families.identity(Path(__file__)),
              "parser": observer.parser_versions, "requested_routes": requested, "all_current_page_routes": all_routes,
              "pages": [], "failures": [], "complete_source_owners": [], "verified_artifacts": [],
              "source_acquisition_complete": False, "mounted_consumers_complete": False,
              "source_chain_complete": False, "coverage_complete": False,
              "query_writes_product_code": False, "target_execution": False, "runtime_acceptance": "unverified",
              "scope": "Actual current main loaders, retained effect/cache owners, invoked roots, shared keyboard IIFE, and mounted Chroma guide translation"}

    def collect_artifacts(audit):
        report["complete_source_owners"].extend(audit.owners.values())
        for name, identity in audit.artifacts.items():
            previous = observer.artifacts.get(name)
            if previous is not None and previous != identity:
                report["failures"].append({"path": name, "error": "Keyboard query observed conflicting artifact identities"})
            else:
                observer.artifacts[name] = identity

    shared_audit = None
    try:
        shared = frontend.current_deployment_source(_KEYBOARD_RUNTIME_ROUTE,
            candidates=routes.get(_KEYBOARD_RUNTIME_ROUTE, []), observations=observations)
        shared_audit = shared.get("audit")
        report["shared_source"] = {name: shared[name] for name in ("status", "acquisition", "entry") if name in shared}
        if shared["status"] != "verified":
            raise ValueError("Current shared keyboard runtime is officially unavailable")
        report["shared_runtime"] = keyboard_navigation_shared_runtime(shared_audit, shared["entry"])
        report["shared_source"]["freshness"] = frontend.verify_latest_deployment_observations(
            shared["acquired"], [shared["entry"]], shared_audit, observations=observations)
    except (OSError, ValueError, KeyError, TypeError, AttributeError, IndexError, StopIteration) as error:
        report["failures"].append({"route": _KEYBOARD_RUNTIME_ROUTE, "error": f"{type(error).__name__}: {error}"})
    finally:
        if shared_audit is not None:
            collect_artifacts(shared_audit)

    def inspect_page(route):
        row = {"route": route, "failures": [], "source_semantics_complete": False, "runtime_acceptance": "unverified"}
        audit = None
        try:
            match = re.fullmatch(r"https://apps\.razer\.com/synapse/products/(\d+)/ui/", route)
            source = (frontend.current_product_source(int(match[1]), "ui", candidates=routes[route], observations=observations)
                      if match else frontend.current_deployment_source(route, candidates=routes[route], observations=observations))
            audit = source.get("audit")
            row.update({name: source[name] for name in ("status", "acquisition", "entry", "manifest") if name in source})
            if source["status"] != "verified":
                raise ValueError("Current keyboard page source is officially unavailable")
            row["selection"] = {name: source["selection"][name] for name in ("name", "canonical_entry_url", "candidate_decisions")
                                if name in source["selection"]}
            mains = [item for item in source["selection"]["deployment"]["sources"] if item["identity_verified"]
                     and item["deployment_role"] == "html_include"
                     and re.search(r"/main\.[\w]+\.js$", urllib.parse.urlsplit(item["url"]).path)]
            if len(mains) != 1:
                raise ValueError("Current keyboard page has no unique mounted main")
            main = source["files"][mains[0]["url"]]
            row["main"] = main
            originals = list({item["path"]: item for item in source["selection"]["originals"]
                              if item["path"].endswith(".js")}.values())
            row["loaders"] = keyboard_navigation_loaders(audit, originals, main)
            if not row["loaders"]:
                row["status"] = "no_keyboard_loader_in_current_main"
                raise ValueError("Current mounted main declares no audited keyboard-navigation loader")
            if route == "https://apps.razer.com/chroma-app/settings/":
                row["chroma_guide"] = keyboard_navigation_chroma_guide(audit, originals, main, row["loaders"])
            row["freshness"] = frontend.verify_latest_deployment_observations(
                source["acquired"], [item for item in source["files"].values() if item.get("path") in audit.parsed],
                audit, observations=observations)
            row["changed_during_query"] = audit.final_snapshot()
            if row["changed_during_query"]:
                raise ValueError("Current keyboard page originals changed during query")
            row["source_semantics_complete"] = True
        except (OSError, ValueError, KeyError, TypeError, AttributeError, IndexError, StopIteration) as error:
            row["failures"].append(f"{type(error).__name__}: {error}")
        return row, audit

    with concurrent.futures.ThreadPoolExecutor(max_workers=2) as executor:
        futures = {executor.submit(inspect_page, route): route for route in requested}
        for future in concurrent.futures.as_completed(futures):
            row, audit = future.result()
            futures.pop(future)
            report["pages"].append(row)
            if audit is not None:
                collect_artifacts(audit)
            report["failures"].extend({"route": row["route"], "error": error} for error in row["failures"])
            print(json.dumps({"query": "keyboard-navigation", "route": row["route"], "failures": row["failures"]}), flush=True)
    report["pages"].sort(key=lambda row: row["route"])
    report["changed_inputs"] = observer.final_snapshot()
    if report["changed_inputs"]:
        report["failures"].append({"error": "Keyboard query inputs changed", "paths": report["changed_inputs"]})
    report["verified_artifacts"] = list(observer.artifacts.values())
    report["complete_source_owners"] = list({row["id"]: row for row in report["complete_source_owners"]}.values())
    report["source_acquisition_complete"] = (report.get("shared_source", {}).get("status") == "verified"
        and all(row.get("acquisition") is not None and row.get("main") is not None for row in report["pages"]))
    report["mounted_consumers_complete"] = all(row["source_semantics_complete"] for row in report["pages"])
    report["source_chain_complete"] = not report["failures"] and report["mounted_consumers_complete"]
    report["coverage_complete"] = requested == all_routes and report["source_chain_complete"]
    report["verification_boundary"] = (
        "Static caller, lexical/export resolution and original bytes do not establish GPUI focus/shortcut behavior, "
        "runtime window acceptance, or device/DLL comparisons; absent current loaders and unresolved mounts remain incomplete.")
    host.publish_host_static_query(report, a.ROOT / ".analysis/results/host-current/local-audit.json", a, "keyboard_navigation")
    if report["failures"]:
        raise ValueError("Current keyboard-navigation source query remains unresolved")
    return report


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
    verification = parsers.add_parser("verify-analysis-tools", help="Pure provenance, byte-range and static UI-tree regression tests")
    verification.add_argument("--test", action="append", help="Select a maintained test module, class or method; repeat to verify affected scopes")
