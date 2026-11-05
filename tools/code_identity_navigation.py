"""Current mounted navigation roots, using verified originals and lexical ASTs.

Only components reached from a real root render are considered. Conditions and
unresolved wrappers remain evidence; this does not establish page behavior or
device completion. Sources are parsed once per product session and byte SHA.
"""
from concurrent.futures import ProcessPoolExecutor
from contextlib import closing
import hashlib
import json
from pathlib import Path
from collections import Counter
from functools import lru_cache

import analysis as a
import code_identity as ci
import families
import frontend
import host
import evidence

EXTRACTION_VERSION = "mounted-navigation-v4"

# Current shared router/loader contracts. Local identifiers are normalized by
# the existing lexical helper; property names, operators and literals remain.
ASYNC_ROUTER = ('R().memo(function({$name:e,children:t}){const[n,o]=R().useState(null);'
    'return R().useEffect(()=>{console.debug("[AsyncRouter][%s]",e);const n=t[e]||t.default;'
    'o(n?R().cloneElement(n,{$name:e}):null)},[e]),n})')
ASYNC_ROUTER_BODY = ('R().memo(function(e){let{$name:t,children:n}=e;const[E,o]=R().useState(null);'
    'return R().useEffect(()=>{console.debug("[AsyncRouter][%s]",t);const e=n[t]||n.default;'
    'o(e?R().cloneElement(e,{$name:t}):null)},[t]),E})')


@lru_cache(maxsize=256)
def predicate_key(raw):
    tree=host.HostLocalAudit().parser.parse(b'('+raw+b');')
    if tree.root_node.has_error:return None
    function=tree.root_node.named_children[0].named_children[0].named_children[0]
    params=function.child_by_field_name('parameters')
    params=list(params.named_children) if params else [function.child_by_field_name('parameter')]
    body=function.child_by_field_name('body')
    if body is not None and body.type=='statement_block':
        rows=[r for r in body.named_children if r.type=='return_statement']
        body=rows[0].named_children[0] if len(rows)==1 and rows[0].named_children else None
    if len(params)!=1 or params[0] is None or body is None or body.type!='binary_expression' or not any(c.type=='===' for c in body.children):return None
    fields=[]
    for side in ('left','right'):
        value=body.child_by_field_name(side)
        if value.type=='member_expression' and value.child_by_field_name('object').text==params[0].text:
            fields.append(value.child_by_field_name('property').text.decode())
    return fields[0] if len(fields)==1 else None


def annotate_selection(report):
    """Keep complete occurrences and mark first-match shadowing from actual predicates."""
    seen={}
    for page in report['pages']:
        syntax=report['owners'][page['component']]['syntax']
        page['binding_resolution']='implementation_root' if syntax in {'class','class_declaration','arrow_function','function_expression','function_declaration'} and page.get('binding_resolution_basis')!='verified_async_component_wrapper' else 'unresolved_wrapper'
        occurrence={k:page.get(k) for k in ('kind','mode','row','component','conditions','mount_path','props','props_are_partial')}
        page['occurrence_sha256']=ci.digest(occurrence)
        if page.get('standalone'):
            page['selection_status']='standalone_consumer'
            continue
        if page.get('selection_method')=='keyed_selector':
            page['selection_status']='keyed_selection_candidate'
            continue
        owner=report['owners'][page['lookup_predicate']]
        src=a.resolve(owner['original']['path'],'.analysis/inputs')
        with src.open('rb') as stream:
            stream.seek(owner['start_byte']);raw=stream.read(owner['end_byte']-owner['start_byte'])
        if hashlib.sha256(raw).hexdigest()!=owner['raw_sha256']:raise ValueError('Retained navigation predicate changed')
        key=predicate_key(raw)
        page['lookup_key']=key
        array=page.get('navigation_array') or next((key for key in page['mount_path'] if 'navigation_array' in report['owners'][key]['roles']),page['mount_path'][-2])
        scope=(page['mode'],tuple(page['mount_path'][:page['mount_path'].index(array)+1]),ci.digest(page.get('conditions',[])))
        value=page['kind'] if key=='name' else page.get('source_id') if key=='id' else None
        identity=(*scope,key,value)
        if value is None:
            page['selection_status']='predicate_scope_unresolved'
        elif identity in seen:
            page['selection_status']='shadowed_by_prior_match'
            page['shadowed_by']=seen[identity]
        else:
            page['selection_status']='preceding_name_unresolved' if page.get('unresolved_preceding_names') else 'first_match_candidate'
            seen[identity]=page['row']


def _write_projection(db, report):
    db.execute('DELETE FROM navigation_pages WHERE product_id=?',(report['product_id'],))
    for page in report['pages']:
        db.execute('INSERT OR IGNORE INTO navigation_pages VALUES(?,?,?,?,?,?)',(report['product_id'],page['kind'],page['mode'],
            page['component'],ci.digest(page.get('conditions',[])),json.dumps(page,ensure_ascii=False,sort_keys=True,separators=(',',':'))))


def rebuild_projection(db_path=ci.DATABASE, legacy_extractor=None):
    with closing(ci.connect(db_path)) as db:
        with db:
            db.execute('BEGIN IMMEDIATE')
            db.execute('DROP TABLE IF EXISTS navigation_pages')
            db.execute('CREATE TABLE navigation_pages(product_id INTEGER,kind TEXT,mode TEXT,component_owner TEXT,condition_scope TEXT,evidence TEXT,PRIMARY KEY(product_id,evidence))')
            count=shadowed=0
            for pid,encoded in list(db.execute('SELECT product_id,receipt FROM navigation_products')):
                report=json.loads(encoded)
                if report.get('extractor') is None and legacy_extractor and report.get('extraction_version')==legacy_extractor['version']:
                    report['extractor']=legacy_extractor
                deployment=db.execute("SELECT acquisition,acquisition_sha FROM deployments WHERE product_id=? AND layer='ui'",(pid,)).fetchone()
                if deployment and report.get('acquisition_sha256')==deployment[1]:report['acquisition']=deployment[0]
                report.setdefault('source_inputs',list({o['original']['sha256']:o['original'] for o in report['owners'].values()}.values()))
                annotate_selection(report)
                _write_projection(db,report)
                db.execute('UPDATE navigation_products SET receipt=? WHERE product_id=?',(json.dumps(report,ensure_ascii=False),pid))
                count+=len(report['pages']);shadowed+=sum(p.get('selection_status')=='shadowed_by_prior_match' for p in report['pages'])
            actual=db.execute('SELECT count(*) FROM navigation_pages').fetchone()[0]
    return {'pages_in_reports':count,'projected_occurrences':actual,'shadowed_occurrences':shadowed}


class Navigation:
    def __init__(self, db_path, product):
        self.product = product
        self.pid = product["product_id"]
        self.audit = host.HostLocalAudit()
        self.db = ci.connect(db_path)
        self.sources, self.asts = {}, {}
        self.owners, self.pages, self.roots, self.failures = {}, [], [], []
        self.known_pages = {}
        self.visited = set()
        self.steps = Counter()

    def close(self):
        self.db.close()

    def source(self, original):
        sha = original["sha256"]
        if sha not in self.asts:
            raw = self.audit.read(original)
            tree = self.audit.tree(original)
            self.sources[sha] = {key: original[key] for key in ("path", "sha256", "bytes")}
            self.asts[sha] = families.RegistrySourceAst(raw, tree)
        return self.asts[sha]

    def shape(self, node, expected):
        return host.help_runtime_shape(self.audit.parser, "value=" + node.text.decode()) == host.help_runtime_shape(self.audit.parser, "value=" + expected)

    def retain(self, src, node, role):
        key = src["sha256"] + ":" + str(node.start_byte) + ":" + str(node.end_byte)
        if key not in self.owners:
            self.owners[key] = {"original": {k: src[k] for k in ("path", "sha256", "bytes")},
                "start_byte": node.start_byte, "end_byte": node.end_byte, "syntax": node.type,
                "raw_sha256": hashlib.sha256(node.text).hexdigest(), "normalized_sha256": ci.normalized_ast(node), "roles": []}
        if role not in self.owners[key]["roles"]:
            self.owners[key]["roles"].append(role)
        return key

    def fail(self, src, node, reason, mode):
        self.failures.append({"owner": self.retain(src, node, "unresolved"), "mode": mode, "reason": str(reason)})

    def jsx(self, src, node, props=None):
        ast = self.source(src)
        node = ast.unwrap(node)
        if node.type != "call_expression":
            raise ValueError("Mounted expression is not JSX")
        callee = ast.unwrap(node.child_by_field_name("function"))
        args = ast.arguments(node)
        if not (ast.member(callee,b"jsx") or ast.member(callee,b"jsxs")) or len(args) not in {2,3} or args[1].type != "object":
            raise ValueError("Mounted JSX argument shape is unresolved")
        fields = {}
        for item in args[1].named_children:
            if item.type == "comment":
                continue
            if item.type == "spread_element":
                value = item.named_children[0]
                self.retain(src,item,"jsx_props_spread")
                if value.text == b"this.props" and props is not None:
                    for key, supplied in props.items():
                        if key != "__unknown_props__" and supplied is not None and supplied[0]["sha256"] == src["sha256"]:
                            fields[key] = supplied[1]
                    if props.get("__unknown_props__"):
                        fields["__unknown_props__"] = True
                else:
                    fields.clear()
                    fields["__unknown_props__"] = True
                continue
            if item.type == "pair":
                key = item.child_by_field_name("key")
                if key.type not in {"property_identifier","string"}:
                    raise ValueError("JSX prop key is dynamic")
                import json5
                name = json5.loads(key.text.decode()) if key.type=="string" else key.text.decode()
                fields[name] = item.child_by_field_name("value")
            elif item.type == "shorthand_property_identifier":
                fields[item.text.decode()] = item
            else:
                raise ValueError("JSX prop shape is unresolved")
        if len(args)==3:
            self.retain(src,args[2],"jsx_key")
        return args[0],fields

    def module(self, module_id):
        rows = list(self.db.execute("""SELECT DISTINCT f.*,s.path,s.bytes FROM factories f
            JOIN sources s ON s.sha256=f.source_sha JOIN deployment_sources ds ON ds.path=s.path
            WHERE ds.product_id=? AND ds.layer='ui' AND f.module_id=?""", (self.pid, str(module_id))))
        unique = {(r["source_sha"], r["start_byte"], r["end_byte"]): r for r in rows}
        if len(unique) != 1:
            raise ValueError("Current UI module is absent or ambiguous: " + str(module_id))
        row = next(iter(unique.values()))
        src = {"path": row["path"], "sha256": row["source_sha"], "bytes": row["bytes"]}
        ast = self.source(src)
        node = ast.tree.root_node.descendant_for_byte_range(row["start_byte"], row["end_byte"] - 1)
        self.retain(src, node, "factory")
        return src, ast, {"byte_start": node.start_byte, "byte_end": node.end_byte}

    def exported(self, mid, name):
        src, ast, factory = self.module(mid)
        value, _ = ast.exported_value(factory, name)
        self.retain(src, value, "export_getter_value")
        return self.resolve(src, value)

    def transparent_provider(self, src, tag):
        if tag.type != "member_expression" or tag.child_by_field_name("property").text != b"Provider":
            return None
        ast = self.source(src)
        _, imported = self.resolve(src, tag.child_by_field_name("object"))
        if imported.type != "call_expression":
            return None
        args = ast.arguments(imported)
        if len(args) != 1 or args[0].type != "number":
            return None
        host.javascript_require_binding(self.audit, src, imported.child_by_field_name("function"))
        other_src, other_ast, factory = self.module(families.javascript_integer_literal(args[0].text.decode()))
        node = other_ast.tree.root_node.descendant_for_byte_range(factory["byte_start"], factory["byte_end"] - 1)
        params = node.child_by_field_name("parameters")
        params = list(params.named_children) if params else [node.child_by_field_name("parameter")]
        if len(params) != 1 or params[0] is None:
            return None
        statements = [s for s in node.child_by_field_name("body").named_children if s.type != "comment"
                      and not (s.type == "expression_statement" and s.named_children[0].type == "string")]
        if len(statements) != 1 or statements[0].type != "expression_statement":
            return None
        assignment = statements[0].named_children[0]
        if (assignment.type == "assignment_expression" and assignment.child_by_field_name("left").text == params[0].text + b".exports"
                and assignment.child_by_field_name("right").text == b"ReactRedux"):
            return self.retain(other_src, node, "react_redux_provider_external")
        return None

    def external_namespace(self,src,expression,expected):
        try:
            current_src,value=self.resolve(src,expression)
            ast=self.source(current_src)
            if value.type=='call_expression' and not ast.arguments(value):
                getter_src,getter=self.resolve(current_src,value.child_by_field_name('function'))
                getter_ast=self.source(getter_src)
                if getter.type!='call_expression' or not getter_ast.member(getter.child_by_field_name('function'),b'n') or len(getter_ast.arguments(getter))!=1:return None
                host.javascript_require_binding(self.audit,getter_src,getter.child_by_field_name('function').child_by_field_name('object'))
                current_src,value=self.resolve(getter_src,getter_ast.arguments(getter)[0]);ast=self.source(current_src)
            if value.type!='call_expression' or len(ast.arguments(value))!=1 or ast.arguments(value)[0].type!='number':return None
            host.javascript_require_binding(self.audit,current_src,value.child_by_field_name('function'))
            provider_src,provider_ast,span=self.module(families.javascript_integer_literal(ast.arguments(value)[0].text.decode()))
            factory=provider_ast.tree.root_node.descendant_for_byte_range(span['byte_start'],span['byte_end']-1)
            params=factory.child_by_field_name('parameters');params=list(params.named_children) if params else [factory.child_by_field_name('parameter')]
            statements=[s for s in factory.child_by_field_name('body').named_children if s.type!='comment' and not(s.type=='expression_statement' and s.named_children[0].type=='string')]
            if len(params)!=1 or params[0] is None or len(statements)!=1 or statements[0].type!='expression_statement':return None
            assignment=statements[0].named_children[0]
            if assignment.type=='assignment_expression' and assignment.child_by_field_name('left').text==params[0].text+b'.exports' and assignment.child_by_field_name('right').text==expected.encode():
                return self.retain(provider_src,factory,'framework_external_'+expected)
        except (ValueError,AttributeError):return None
        return None

    def resolve(self, src, node, chain=()):
        ast = self.source(src)
        while node.type in {"parenthesized_expression", "sequence_expression"}:
            if node.type == "sequence_expression":
                for prefix in node.named_children[:-1]:
                    self.retain(src, prefix, "binding_initializer_effect")
            node = node.named_children[-1]
        key = (src["sha256"], node.start_byte, node.end_byte)
        if key in chain:
            raise ValueError("Mounted component binding cycle")
        if node.type == "identifier":
            declaration = ast.declaration(node)
            self.retain(src, declaration, "binding")
            if declaration.type in {"class_declaration", "function_declaration"}:
                return src, declaration
            value = declaration.child_by_field_name("value")
            if value is None:
                raise ValueError("Mounted binding has no value")
            return self.resolve(src, value, (*chain, key))
        if node.type == "member_expression":
            obj_src, obj = self.resolve(src, node.child_by_field_name("object"), (*chain, key))
            obj_ast = self.source(obj_src)
            if obj.type=='object':
                fields=obj_ast.fields(obj);name=node.child_by_field_name('property').text.decode()
                if name not in fields:raise ValueError('Static object member is absent: '+name)
                return self.resolve(obj_src,fields[name],(*chain,key))
            if obj.type == "call_expression":
                args = obj_ast.arguments(obj)
                if len(args) == 1 and args[0].type == "number":
                    host.javascript_require_binding(self.audit, obj_src, obj.child_by_field_name("function"))
                    return self.exported(families.javascript_integer_literal(args[0].text.decode()),
                                         node.child_by_field_name("property").text.decode())
            return src, node
        if node.type == "call_expression":
            wrapper = ast.unwrap(node.child_by_field_name("function"))
            if wrapper.type == "call_expression" and ast.member(ast.unwrap(wrapper.child_by_field_name("function")), b"connect"):
                args = ast.arguments(node)
                if len(args) != 1:
                    raise ValueError("Connect wrapper has extra arguments")
                self.retain(src, node, "connected_state_and_actions")
                return self.resolve(src, args[0], (*chain, key))
            if wrapper.type in {"function_expression", "arrow_function"}:
                args = ast.arguments(node)
                if args and self.forwarding_hoc(src, wrapper):
                    self.retain(src, wrapper, "component_forwarding_hoc")
                    return self.resolve(src, args[0], (*chain, key))
            try:
                wrapper_src,forwarder=self.resolve(src,wrapper,(*chain,key))
            except (ValueError,AttributeError):
                wrapper_src,forwarder=src,wrapper
            if forwarder.type in {'function_declaration','function_expression','arrow_function'} and self.forwarding_hoc(wrapper_src,forwarder):
                args=ast.arguments(node)
                if args:
                    self.retain(wrapper_src,forwarder,'component_forwarding_hoc')
                    self.retain(src,node,'hoc_invocation')
                    for argument in args[1:]:
                        self.retain(src,argument,'hoc_argument')
                        try:
                            hook_src,hook=self.resolve(src,argument)
                            if hook.type in {'arrow_function','function_expression','function_declaration'}:self.retain(hook_src,hook,'hoc_argument_callable')
                        except (ValueError,AttributeError):pass
                    return self.resolve(src,args[0],(*chain,key))
        return src, node

    def forwarding_hoc(self, src, function):
        ast = self.source(src)
        params = function.child_by_field_name("parameters")
        params = list(params.named_children) if params else [function.child_by_field_name("parameter")]
        if not params or params[0] is None or params[0].type != "identifier":
            return False
        returned = self.returns(function)
        if len(returned) != 1:
            return False
        call = ast.unwrap(returned[0])
        if call.type in {'arrow_function','function_expression'}:
            output=self.returns(call)
            if len(output)!=1:return False
            jsx=ast.unwrap(output[0])
            if jsx.type!='call_expression':return False
            callee=ast.unwrap(jsx.child_by_field_name('function'))
            args=ast.arguments(jsx)
            if not (ast.member(callee,b'jsx') or ast.member(callee,b'jsxs')) or len(args)!=2 or args[0].type!='identifier':return False
            return ast.bindings.resolve(args[0].text.decode(),args[0].start_byte).get('start_byte')==params[0].start_byte
        if call.type != "call_expression":
            return False
        callee, args = ast.unwrap(call.child_by_field_name("function")), ast.arguments(call)
        if callee.type in {"function_expression", "arrow_function"} and args:
            argument = args[0]
            if argument.type == "identifier":
                resolved = ast.bindings.resolve(argument.text.decode(), argument.start_byte)
                if resolved.get("start_byte") == params[0].start_byte:
                    return self.forwarding_hoc(src, callee)
        if (callee.type != "call_expression" or not ast.member(ast.unwrap(callee.child_by_field_name("function")), b"connect")
                or len(args) != 1 or args[0].type not in {"arrow_function", "function_expression"}):
            return False
        returned = self.returns(args[0])
        if len(returned) != 1:
            return False
        jsx = ast.unwrap(returned[0])
        if jsx.type != "call_expression" or not (ast.member(ast.unwrap(jsx.child_by_field_name("function")), b"jsx")
                or ast.member(ast.unwrap(jsx.child_by_field_name("function")), b"jsxs")):
            return False
        values = ast.arguments(jsx)
        if len(values) != 2 or values[0].type != "identifier":
            return False
        binding = ast.bindings.resolve(values[0].text.decode(), values[0].start_byte)
        return binding.get("start_byte") == params[0].start_byte

    @staticmethod
    def returns(function):
        body = function.child_by_field_name("body")
        if body is None:
            return []
        if body.type != "statement_block":
            return [body]
        results, pending = [], list(reversed(body.named_children))
        while pending:
            node = pending.pop()
            if node.type in {"arrow_function", "function_expression", "function_declaration", "class_declaration"}:
                continue
            if node.type == "return_statement":
                if node.named_children:
                    results.append(node.named_children[0])
            else:
                pending.extend(reversed(node.named_children))
        return results

    def parameter_props(self, src, function, props):
        parameters = function.child_by_field_name("parameters")
        params = list(parameters.named_children) if parameters else [function.child_by_field_name("parameter")]
        bound = {}
        if len(params) == 1 and params[0] is not None and params[0].type == "object_pattern":
            bound.update(self.pattern_props(params[0], props))
        elif len(params)==1 and params[0] is not None and params[0].type=="identifier":
            ast = self.source(src)
            for item in ast.walk(function.child_by_field_name("body")):
                if (item.type=="variable_declarator" and item.child_by_field_name("value") is not None
                        and item.child_by_field_name("value").text==params[0].text):
                    resolved=ast.bindings.resolve(params[0].text.decode(),item.child_by_field_name("value").start_byte)
                    if resolved.get("start_byte")==params[0].start_byte:
                        bound.update(self.pattern_props(item.child_by_field_name("name"),props))
        return bound

    def display_guard(self, src, condition):
        ast = self.source(src)
        modes = []
        for node in ast.walk(condition):
            if node.type != "binary_expression" or not any(child.type == "===" for child in node.children):
                continue
            left, right = node.child_by_field_name("left"), node.child_by_field_name("right")
            literal, reference = (left, right) if left.type == "string" else (right, left)
            if literal.type != "string":
                continue
            try:
                value = ast.resolve(reference)
            except ValueError:
                if reference.type!='identifier':continue
                binding=ast.bindings.resolve(reference.text.decode(),reference.start_byte)
                if binding.get('resolution')!='destructuring_requires_value_analysis':continue
                declaration=ast.tree.root_node.descendant_for_byte_range(binding['start_byte'],binding['end_byte']-1)
                pattern=declaration.child_by_field_name('name');initial=declaration.child_by_field_name('value')
                if pattern is None or pattern.type!='object_pattern' or initial is None or initial.type!='call_expression':continue
                if not any(p.type=='pair_pattern' and p.child_by_field_name('key').text==b'displayMode' and p.child_by_field_name('value').text==reference.text for p in pattern.named_children):continue
                if not ast.member(initial.child_by_field_name('function'),b'useMemo'):continue
                args=ast.arguments(initial)
                if not args or args[0].type not in {'arrow_function','function_expression'}:continue
                returns=self.returns(args[0])
                if len(returns)!=1 or returns[0].type!='object':continue
                try:value=ast.fields(returns[0])['displayMode']
                except (KeyError,ValueError):continue
                self.retain(src,declaration,'display_query_binding')
            if value.type != "call_expression" or not ast.member(value.child_by_field_name("function"), b"get"):
                continue
            arguments = ast.arguments(value)
            receiver = value.child_by_field_name("function").child_by_field_name("object")
            if len(arguments) == 1 and arguments[0].text in {b'"displayMode"', b"'displayMode'"} and ast.member(receiver, b"searchParams"):
                import json5
                modes.append(json5.loads(literal.text.decode()))
        return modes[0] if len(set(modes)) == 1 else None

    def returned_mode(self, src, returned, mode):
        parent = returned.parent
        while parent is not None and parent.type not in {"arrow_function", "function_expression", "function_declaration", "method_definition"}:
            if parent.type == "if_statement":
                condition = parent.child_by_field_name("condition")
                branch = self.display_guard(src, condition)
                if branch:
                    self.roots.append({"mode": branch, "selector": self.retain(src, condition, "display_condition"),
                                       "expression": self.retain(src, returned, "display_branch")})
                    return branch
            parent = parent.parent
        return mode

    @staticmethod
    def pattern_props(pattern, props):
        bound = {}
        if pattern.type == "object_pattern":
            for field in pattern.named_children:
                if field.type == "pair_pattern":
                    key, value = field.child_by_field_name("key"), field.child_by_field_name("value")
                    if value.type == "identifier" and (key.text.decode() in props or not props.get("__unknown_props__")):
                        bound[value.text.decode()] = props.get(key.text.decode())
                elif field.type == "shorthand_property_identifier_pattern" and (field.text.decode() in props or not props.get("__unknown_props__")):
                    bound[field.text.decode()] = props.get(field.text.decode())
        return bound

    def visit(self, src, expression, mode, path, *, component=None, props=None, bound=None):
        self.steps[mode] += 1
        if self.steps[mode] > 500:
            raise ValueError("Mounted graph exceeded its bounded traversal")
        ast = self.source(src)
        props, bound = props or {}, bound or {}
        node = expression
        if node.type in {"parenthesized_expression", "sequence_expression"}:
            if node.named_children:
                self.visit(src, node.named_children[-1], mode, path, component=component, props=props, bound=bound)
            return
        if node.type in {"null", "false", "true", "string", "number", "undefined"}:
            return
        if node.type == "identifier" and node.text.decode() in bound:
            value = bound[node.text.decode()]
            if value is not None:
                other_src, other = value
                self.visit(other_src, other, mode, path)
            return
        if node.type == "member_expression" and node.text == b"this.props.children" and "children" in props:
            self.visit(*props["children"], mode, path)
            return
        if node.type in {"ternary_expression", "binary_expression", "array"}:
            if node.type == "ternary_expression":
                guard = self.retain(src, node.child_by_field_name("condition"), "render_condition")
                display_mode = self.display_guard(src, node.child_by_field_name("condition"))
                for arm in (("alternative","consequence") if display_mode else ("consequence","alternative")):
                    branch_mode = display_mode if arm == "consequence" and display_mode else mode
                    if display_mode and arm == "consequence":
                        self.roots.append({"mode": branch_mode, "selector": guard,
                                           "expression": self.retain(src, node.child_by_field_name(arm), "display_branch")})
                    if branch_mode in {"default", "chromaApp"}:
                        self.visit(src, node.child_by_field_name(arm), branch_mode, [*path, guard], component=component, props=props, bound=bound)
            elif node.type == "array":
                for child in ast.array_items(node):
                    self.visit(src, child, mode, path, component=component, props=props, bound=bound)
            else:
                if any(child.type in {"&&", "||", "??"} for child in node.children):
                    self.visit(src, node.child_by_field_name("right"), mode,
                               [*path, self.retain(src, node.child_by_field_name("left"), "render_condition")],
                               component=component, props=props, bound=bound)
            return
        if node.type == "call_expression":
            callee = ast.unwrap(node.child_by_field_name("function"))
            if callee.type in {"arrow_function","function_expression"} and not ast.arguments(node):
                invoked=self.retain(src,node,"render_iife_call")
                for returned in self.returns(callee):
                    branch_mode=self.returned_mode(src,returned,mode)
                    if branch_mode in {"default","chromaApp"}:
                        self.visit(src,returned,branch_mode,[*path,invoked],component=component,props=props,bound=bound)
                return
            if component is not None and callee.type == "member_expression" and callee.child_by_field_name("object").type == "this":
                method = ast.callable_member(component, callee.child_by_field_name("property").text)
                method_owner = self.retain(src, method, "invoked_render_method")
                for returned in self.returns(method):
                    branch_mode = self.returned_mode(src, returned, mode)
                    if branch_mode in {"default", "chromaApp"}:
                        self.visit(src, returned, branch_mode, [*path, method_owner], component=component, props=props, bound=bound)
                return
            if ast.member(callee, b"jsx") or ast.member(callee, b"jsxs"):
                tag, fields = self.jsx(src,node,props)
                mount = self.retain(src, node, "jsx_consumer")
                passed = {key: (src, value) for key, value in fields.items() if key != "__unknown_props__"}
                if fields.get("__unknown_props__"):
                    passed["__unknown_props__"] = True
                if "$name" in fields and "children" in fields and fields["children"].type == "object":
                    selector_src, selector = self.resolve(src, tag)
                    if not any(self.shape(selector,shape) for shape in (ASYNC_ROUTER,ASYNC_ROUTER_BODY)):
                        raise ValueError("Mounted display selector differs from its complete cloneElement/state contract")
                    selector_owner = self.retain(selector_src, selector, "display_selector_component")
                    modes={};key_owners={}
                    for pair in fields['children'].named_children:
                        if pair.type!='pair':raise ValueError('Selector map contains an unresolved spread')
                        key_node=pair.child_by_field_name('key')
                        if key_node.type=='computed_property_name' and key_node.named_child_count==1:
                            key_src,resolved_key=self.resolve(src,key_node.named_children[0])
                            if resolved_key.type not in {'string','number'}:raise ValueError('Selector computed key is not a resolved literal')
                            import json5
                            name=str(json5.loads(resolved_key.text.decode()))
                            key_owners[name]=self.retain(key_src,resolved_key,'selector_key')
                        elif key_node.type in {'property_identifier','string'}:
                            import json5
                            name=json5.loads(key_node.text.decode()) if key_node.type=='string' else key_node.text.decode()
                            key_owners[name]=self.retain(src,key_node,'selector_key')
                        else:raise ValueError('Selector key shape is unresolved')
                        if name in modes:raise ValueError('Selector map repeats a key')
                        modes[name]=pair.child_by_field_name('value')
                    if "default" not in modes:
                        raise ValueError("Mounted display selector has no default branch")
                    display=bool(set(modes)&{'macro','chromaApp','armory','multiDevicePairing'})
                    branches=['default',*sorted(set(modes)-{'default'})] if display else [*sorted(set(modes)-{'default'}),'default']
                    for branch in branches:
                        branch_mode=branch if display else mode
                        self.roots.append({"mode":branch_mode,"selector_key":branch,"selector_kind":"display_mode" if display else "state_key",
                            "selector":selector_owner,"expression":self.retain(src,modes[branch],"selector_branch")})
                        if branch_mode in {"default","chromaApp"}:
                            self.visit(src,modes[branch],branch_mode,[*path,mount,selector_owner,key_owners[branch]])
                    return
                if "lazy" in fields:
                    loader_src, loader = self.resolve(src, tag)
                    actual = host.help_runtime_shape(self.audit.parser, "value=" + loader.text.decode())
                    if not any(actual == host.help_runtime_shape(self.audit.parser, template) for template in (
                            host._JAVASCRIPT_LAZY_COMPONENT, host._JAVASCRIPT_LAZY_COMPONENT_BODY_DESTRUCTURE)):
                        raise ValueError("Mounted lazy component differs from its invocation/result/cleanup contract")
                    self.lazy(src, fields["lazy"], mode, [*path, mount, self.retain(loader_src, loader, "async_component_lifecycle")])
                    return
                if tag.type == "string" or ast.member(tag, b"Fragment"):
                    if "children" in fields:
                        self.visit(src, fields["children"], mode, [*path, mount], component=component, props=props, bound=bound)
                    return
                try:
                    provider = self.transparent_provider(src, tag)
                    if provider and "children" in fields:
                        self.visit(src, fields["children"], mode, [*path, mount, provider], component=component, props=props, bound=bound)
                        return
                    child_src, child = self.resolve(src, tag)
                    self.component(child_src, child, mode, [*path, mount], passed)
                except ValueError as error:
                    self.fail(src, node, error, mode)
                return
        if node.type == "identifier":
            try:
                other_src, other = self.resolve(src, node)
                self.visit(other_src, other, mode, path, component=component, props=props, bound=bound)
            except ValueError as error:
                self.fail(src, node, error, mode)

    def component(self, src, node, mode, path, props):
        signature = tuple(sorted((key, value[0]["sha256"], value[1].start_byte, value[1].end_byte)
                                 for key,value in props.items() if key != "__unknown_props__" and value is not None))
        key = (src["sha256"], node.start_byte, node.end_byte, mode, signature, bool(props.get("__unknown_props__")))
        if key in self.visited:
            return
        self.visited.add(key)
        owner = self.retain(src, node, "mounted_component")
        if owner in path:
            self.fail(src,node,"Recursive component data flow remains unresolved",mode)
            return
        if key[:3] in self.known_pages:
            for kind in self.known_pages[key[:3]]:
                self.pages.append({"kind": kind, "mode": mode, "component": owner,
                                   "mount_path": [*path, owner], "standalone": True,
                                   "props": {name: self.retain(*value, "jsx_prop") for name, value in props.items() if name != "__unknown_props__"},
                                   "props_are_partial": bool(props.get("__unknown_props__"))})
            return
        ast = self.source(src)
        if node.type in {"class", "class_declaration"}:
            state = []
            for item in ast.walk(node.child_by_field_name("body")):
                if item.type == "assignment_expression" and item.child_by_field_name("left").text == b"this.state":
                    value = item.child_by_field_name("right")
                    if value.type == "object":
                        candidate=None
                        for field in value.named_children:
                            if field.type=="pair" and field.child_by_field_name("key").text in {b"navs",b'"navs"'}:
                                candidate=field.child_by_field_name("value")
                            elif field.type=="spread_element" and candidate is not None:
                                raise ValueError("Navigation state spread may replace its navs initializer")
                        if candidate is not None:state.append(candidate)
            if state:
                self.navigation(src, node, state, mode, [*path, owner])
                return
            renderer = ast.callable_member(node, b"render")
            local_props = {}
            for declaration in ast.walk(renderer.child_by_field_name("body")):
                if (declaration.type == "variable_declarator" and declaration.child_by_field_name("value") is not None
                        and declaration.child_by_field_name("value").text == b"this.props"):
                    local_props.update(self.pattern_props(declaration.child_by_field_name("name"), props))
            for returned in self.returns(renderer):
                branch_mode = self.returned_mode(src, returned, mode)
                if branch_mode in {"default", "chromaApp"}:
                    self.visit(src, returned, branch_mode, [*path, owner, self.retain(src, renderer, "render")], component=node, props=props, bound=local_props)
        elif node.type in {"function_declaration", "function_expression", "arrow_function"}:
            for returned in self.returns(node):
                branch_mode = self.returned_mode(src, returned, mode)
                if branch_mode in {"default", "chromaApp"}:
                    self.visit(src, returned, branch_mode, [*path, owner], props=props, bound=self.parameter_props(src,node,props))
        else:
            raise ValueError("Mounted component is not a local callable/class: " + node.type)

    def lazy(self, src, loader, mode, path):
        other_src,other,request=self.lazy_target(src,loader)
        self.component(other_src,other,mode,[*path,request],{})

    def lazy_target(self, src, loader):
        ast = self.source(src)
        if loader.type not in {"arrow_function", "function_expression"}:
            raise ValueError("Lazy root is not a literal callback")
        values = self.returns(loader)
        if len(values) != 1:
            raise ValueError("Lazy root has multiple return paths")
        call = ast.unwrap(values[0])
        if call.type != "call_expression" or not ast.member(call.child_by_field_name("function"), b"then"):
            raise ValueError("Lazy root has no direct import completion")
        args = ast.arguments(call)
        if len(args) != 1 or args[0].type != "call_expression" or not ast.member(args[0].child_by_field_name("function"), b"bind"):
            raise ValueError("Lazy root is not a literal require binding")
        require = args[0].child_by_field_name("function").child_by_field_name("object")
        bound = ast.arguments(args[0])
        if len(bound) != 2 or bound[0].text != require.text or bound[1].type != "number":
            raise ValueError("Lazy root has an inconsistent module binding")
        mid = families.javascript_integer_literal(bound[1].text.decode())
        host.javascript_require_binding(self.audit, src, require)
        other_src, other = self.exported(mid, "default")
        return other_src,other,self.retain(src,loader,"lazy_import_callback")

    def page_target(self,src,tag,props):
        page_src,page=self.resolve(src,tag)
        wrappers=[];issue=None
        if page.type=='call_expression':
            ast=self.source(page_src);function=ast.unwrap(page.child_by_field_name('function'));args=ast.arguments(page)
            if ast.member(function,b'lazy') and len(args)==1:
                external=self.external_namespace(page_src,function.child_by_field_name('object'),'React')
                if external:
                    wrappers.extend([external,self.retain(page_src,page,'react_lazy_page')])
                    try:
                        page_src,page,request=self.lazy_target(page_src,args[0]);wrappers.append(request)
                    except ValueError as error:issue=str(error)
        if 'lazy' in props:
            actual=host.help_runtime_shape(self.audit.parser,'value='+page.text.decode())
            if any(actual==host.help_runtime_shape(self.audit.parser,shape) for shape in (host._JAVASCRIPT_LAZY_COMPONENT,host._JAVASCRIPT_LAZY_COMPONENT_BODY_DESTRUCTURE)):
                wrappers.append(self.retain(page_src,page,'async_component_lifecycle'))
                try:
                    page_src,page,request=self.lazy_target(src,props['lazy']);wrappers.append(request)
                except ValueError as error:issue=str(error)
        return page_src,page,wrappers,issue

    def navigation_name(self, src, expression):
        import json5
        name_src, value = self.resolve(src, expression)
        if value.type == 'string':
            return name_src, value, json5.loads(value.text.decode())
        if value.type == 'call_expression':
            ast = self.source(name_src)
            callee = ast.unwrap(value.child_by_field_name('function'))
            if callee.type == 'member_expression' and not ast.arguments(value):
                method = callee.child_by_field_name('property').text
                if method in {b'toUpperCase', b'toLowerCase'}:
                    base_src, base = self.resolve(name_src, callee.child_by_field_name('object'))
                    if base.type == 'string':
                        text = json5.loads(base.text.decode())
                        if text.isascii():
                            self.retain(base_src, base, 'navigation_name_case_input')
                            self.retain(name_src, value, 'navigation_ascii_case_transform')
                            return name_src, value, text.upper() if method == b'toUpperCase' else text.lower()
        raise ValueError('Navigation kind is not a resolved string or verified ASCII case transform')

    def navigation_entries(self, src, ast, array, use):
        """Retain literal appends executed before the array reaches state.navs."""
        entries = [(entry, []) for entry in ast.array_items(array)]
        callable_types = {'arrow_function', 'function_expression', 'function_declaration', 'method_definition'}
        def enclosing(node):
            while node is not None and node.type not in callable_types:
                node = node.parent
            return node
        scope = enclosing(array)
        if scope is None or use.start_byte <= array.end_byte:
            return entries
        for call in ast.walk(scope):
            if (call.type != 'call_expression' or not array.end_byte <= call.start_byte < use.start_byte
                    or enclosing(call) != scope):
                continue
            callee = call.child_by_field_name('function')
            if callee.type != 'member_expression':
                continue
            try:
                receiver = ast.resolve(callee.child_by_field_name('object'))
            except ValueError:
                continue
            if (receiver.start_byte, receiver.end_byte) != (array.start_byte, array.end_byte):
                continue
            if callee.child_by_field_name('property').text != b'push':
                raise ValueError('Navigation array has an unsupported pre-state operation')
            args = ast.arguments(call)
            if not args or any(arg.type != 'object' for arg in args):
                raise ValueError('Navigation append is not a complete literal row')
            conditions = []
            parent = call.parent
            while parent is not None and parent != scope:
                if parent.type == 'binary_expression':
                    right = parent.child_by_field_name('right')
                    operator = next((c.type for c in parent.children if c.type in {'&&', '||'}), None)
                    if operator and right.start_byte <= call.start_byte < right.end_byte:
                        conditions.append({'owner': self.retain(src, parent.child_by_field_name('left'), 'navigation_append_condition'),
                                           'operator': operator, 'arm': 'consequence' if operator == '&&' else 'alternative'})
                elif parent.type in {'if_statement', 'ternary_expression'}:
                    consequence = parent.child_by_field_name('consequence')
                    conditions.append({'owner': self.retain(src, parent.child_by_field_name('condition'), 'navigation_append_condition'),
                                       'arm': 'consequence' if consequence.start_byte <= call.start_byte < consequence.end_byte else 'alternative'})
                elif parent.type in {'for_statement', 'for_in_statement', 'while_statement', 'do_statement', 'switch_statement'}:
                    raise ValueError('Navigation append has unresolved loop/switch execution')
                parent = parent.parent
            self.retain(src, call, 'navigation_literal_append')
            entries.extend((arg, conditions) for arg in args)
        return entries

    def keyed_navigation(self,src,node,state,mode,path):
        ast=self.source(src);render=ast.callable_member(node,b'render');candidates=[]
        for returned in self.returns(render):
            for call in ast.walk(returned):
                if call.type!='call_expression':continue
                function=ast.unwrap(call.child_by_field_name('function'))
                if not (ast.member(function,b'jsx') or ast.member(function,b'jsxs')):continue
                try:tag,props=self.jsx(src,call)
                except ValueError:continue
                if props.get('$name') is not None and props['$name'].text==b'this.state.active_view' and props.get('children') is not None and props['children'].type=='object':
                    candidates.append((call,tag,props))
        if len(candidates)!=1:raise ValueError('Keyed navigation has no unique live active_view selector')
        call,tag,props=candidates[0];selector_src,selector=self.resolve(src,tag)
        if not any(self.shape(selector,shape) for shape in (ASYNC_ROUTER,ASYNC_ROUTER_BODY)):raise ValueError('Keyed navigation selector contract differs')
        renderer=self.retain(src,render,'keyed_navigation_renderer');selector_owner=self.retain(selector_src,selector,'keyed_navigation_selector')
        for pair in props['children'].named_children:
            if pair.type!='pair':raise ValueError('Keyed navigation has dynamic child entries')
            key=pair.child_by_field_name('key')
            key_src,key_value,kind=self.navigation_name(src,key.named_children[0] if key.type=='computed_property_name' and key.named_child_count==1 else key)
            expression=pair.child_by_field_name('value');page_tag,passed=self.jsx(src,expression)
            page_src,page,wrappers,issue=self.page_target(src,page_tag,passed)
            owner=self.retain(page_src,page,'page_root')
            self.known_pages.setdefault((page_src['sha256'],page.start_byte,page.end_byte),set()).add(kind)
            result={'kind':kind,'mode':mode,'component':owner,'row':self.retain(src,pair,'keyed_navigation_row'),
                    'selection_method':'keyed_selector','name_owner':self.retain(key_src,key_value,'navigation_name'),
                    'conditions':[{'selector':self.retain(src,props['$name'],'active_view_selector'),'equals':kind}],
                    'mount_path':[*path,renderer,selector_owner,*wrappers],'standalone':False,
                    'navigation_consumers':[renderer,self.retain(src,state[0],'navigation_items_source')],
                    'props':{name:self.retain(src,value,'page_prop') for name,value in passed.items() if name!='__unknown_props__'},
                    'props_are_partial':bool(passed.get('__unknown_props__'))}
            if issue:result.update(binding_resolution_basis='verified_async_component_wrapper',lazy_resolution_error=issue)
            self.pages.append(result)

    def navigation(self, src, node, state, mode, path):
        ast = self.source(src)
        if len(state) != 1:
            raise ValueError("Navigation has multiple state initializers")
        try:renderer = ast.callable_member(node, b"renderView")
        except ValueError:
            self.keyed_navigation(src,node,state,mode,path)
            return
        render = ast.callable_member(node, b"render")
        if not any(n.type == "call_expression" and n.child_by_field_name("function").text == b"this.renderView"
                   for n in ast.walk(render.child_by_field_name("body"))):
            raise ValueError("Navigation renderView is not consumed by render")
        finds = [n for n in ast.walk(renderer.child_by_field_name("body")) if n.type == "call_expression"
                 and ast.member(n.child_by_field_name("function"), b"find")
                 and n.child_by_field_name("function").child_by_field_name("object").text == b"this.state.navs"]
        if len(finds) != 1:
            raise ValueError("Navigation has no unique live navs lookup")
        consumer = self.retain(src, renderer, "selected_page_renderer")
        predicate = ast.arguments(finds[0])
        if len(predicate) != 1:
            raise ValueError("Navigation lookup predicate is unresolved")
        selected = []
        projections = self.returns(renderer)
        for n in (n for projection in projections for n in ast.walk(projection)):
            if n.type == "member_expression" and n.child_by_field_name("property").text in {b"component", b"renderComponent"}:
                value = n.child_by_field_name("object")
                try:
                    resolved = ast.resolve(value)
                except ValueError:
                    continue
                if resolved.type == "binary_expression" and any(c.type == "||" for c in resolved.children):
                    if resolved.child_by_field_name("right").text == b"this.state.navs[0]":
                        self.retain(src, resolved, "missing_active_view_fallback")
                        resolved = resolved.child_by_field_name("left")
                if (resolved.start_byte, resolved.end_byte) == (finds[0].start_byte, finds[0].end_byte):
                    selected.append(n.child_by_field_name("property").text.decode())
        if len(set(selected)) != 1:
            raise ValueError("Navigation does not return a verified selected component field")
        field = selected[0]
        extras = []
        try:
            extras.append(self.retain(src, ast.callable_member(node, b"changeView"), "navigation_click_state_writer"))
        except ValueError:
            pass
        for n in ast.walk(render.child_by_field_name("body")):
            if n.type == "call_expression" and ast.member(n.child_by_field_name("function"), b"filter"):
                extras.append(self.retain(src, n, "navigation_filter"))
        branches = [(state[0], [])]
        while branches:
            expression, conditions = branches.pop()
            value = ast.resolve(expression)
            while value.type in {"call_expression","member_expression"}:
                if value.type=="call_expression":
                    function=ast.unwrap(value.child_by_field_name("function"));args=ast.arguments(value)
                    if function.text==b"Array.from" and len(args)==1 and ast.bindings.resolve('Array',function.start_byte).get('resolution')=='unresolved_identifier':
                        self.retain(src,value,"navigation_array_copy");value=ast.resolve(args[0]);continue
                    if function.type=="member_expression" and function.child_by_field_name("object").type=="this" and not args:
                        method=ast.callable_member(node,function.child_by_field_name("property").text)
                        returned=self.returns(method)
                        if len(returned)!=1:raise ValueError('Navigation getter has multiple return paths')
                        self.retain(src,method,"navigation_array_getter");value=ast.resolve(returned[0]);continue
                if value.type=="member_expression" and value.child_by_field_name("object").type=="this":
                    writers=[item.child_by_field_name('right') for item in ast.walk(node.child_by_field_name('body'))
                             if item.type=='assignment_expression' and item.child_by_field_name('left').text==value.text]
                    if len(writers)!=1:raise ValueError('Navigation backing field has multiple/absent writers')
                    self.retain(src,writers[0],"navigation_backing_array");value=ast.resolve(writers[0]);continue
                break
            if value.type == "ternary_expression":
                guard = self.retain(src, value.child_by_field_name("condition"), "navigation_condition")
                for arm in ("alternative", "consequence"):
                    branches.append((value.child_by_field_name(arm), [*conditions, {"owner": guard, "arm": arm}]))
                continue
            if value.type != "array":
                raise ValueError("Navigation state is not a literal array/conditional")
            array_owner = self.retain(src, value, "navigation_array")
            unresolved_names=[]
            for entry, insertion_conditions in self.navigation_entries(src, ast, value, expression):
                fields = ast.fields(entry)
                if not {"id", "name", field} <= fields.keys():
                    raise ValueError("Navigation row lacks identity or selected component")
                try:key_src,key,kind=self.navigation_name(src,fields['name'])
                except (ValueError,AttributeError) as error:
                    self.fail(src,fields['name'],error,mode)
                    unresolved_names.append(self.retain(src,fields['name'],'unresolved_navigation_name'))
                    continue
                expression = fields[field]
                if field == "renderComponent":
                    values = self.returns(expression)
                    if len(values) != 1:
                        raise ValueError("Page renderer is not a single return")
                    expression = values[0]
                tag, passed = self.jsx(src,expression)
                try:page_src,page,wrappers,lazy_issue=self.page_target(src,tag,passed)
                except (ValueError,AttributeError) as error:
                    self.fail(src,entry,error,mode)
                    continue
                page_owner = self.retain(page_src, page, "page_root")
                self.known_pages.setdefault((page_src["sha256"], page.start_byte, page.end_byte), set()).add(kind)
                self.pages.append({"kind": kind, "mode": mode, "component": page_owner, "row": self.retain(src, entry, "navigation_row"),
                    "source_id":families.javascript_integer_literal(fields['id'].text.decode()) if fields['id'].type=='number' else None,
                    "extra_row_fields":{key:self.retain(src,value,'navigation_row_option') for key,value in fields.items() if key not in {'id','name',field}},
                    "name_owner": self.retain(key_src, key, "navigation_name"), "conditions": [*conditions, *insertion_conditions],
                    "mount_path": [*path, array_owner, consumer,*wrappers], "standalone": False,
                    "navigation_array":array_owner,
                    "lookup_predicate": self.retain(src, predicate[0], "active_view_lookup"),
                    "navigation_consumers": [self.retain(src, render, "navigation_render"), *extras],
                    "props": {name: self.retain(src, prop, "page_prop") for name, prop in passed.items() if name != "__unknown_props__"},
                    "props_are_partial": bool(passed.get("__unknown_props__"))})
                if unresolved_names:self.pages[-1]['unresolved_preceding_names']=list(unresolved_names)
                if lazy_issue:self.pages[-1].update(binding_resolution_basis='verified_async_component_wrapper',lazy_resolution_error=lazy_issue)

    def run(self):
        originals = [dict(row) for row in self.db.execute("""SELECT s.* FROM deployment_sources ds JOIN sources s USING(path)
            WHERE ds.product_id=? AND ds.layer='ui' AND ds.path LIKE '%/main.%'""", (self.pid,))]
        if len(originals) != 1:
            raise ValueError("Current product has no unique main original")
        src = originals[0]
        ast = self.source(src)
        mounts = []
        for node in ast.walk(ast.tree.root_node):
            if node.type != "call_expression" or not ast.member(node.child_by_field_name("function"), b"render"):
                continue
            receiver = ast.unwrap(node.child_by_field_name("function").child_by_field_name("object"))
            if receiver.type == "call_expression":
                args = ast.arguments(receiver)
                target=ast.resolve(args[0]) if len(args)==1 else None
                if target is not None and target.text in {b'document.getElementById("root")', b"document.getElementById('root')"}:
                    factory=ast.unwrap(receiver.child_by_field_name('function'))
                    if not ast.member(factory,b'createRoot'):
                        try:evidence._keyboard_create_root(self.audit,[src],src,factory)
                        except (ValueError,AttributeError):continue
                    self.retain(src,target,'root_container')
                    mounts.append(node)
        if len(mounts) != 1 or len(ast.arguments(mounts[0])) != 1:
            raise ValueError("Current product has no unique explicit createRoot render")
        startup = evidence._keyboard_startup_path(self.audit, src, mounts[0])
        root = self.retain(src, mounts[0], "actual_root_render")
        self.visit(src, ast.arguments(mounts[0])[0], "default", [root])
        changed = self.audit.final_snapshot()
        if changed:
            raise ValueError("Navigation originals changed during inspection: " + str(changed))
        return {"product_id": self.pid, "family": self.product["family"], "acquisition": self.product["acquisition"],
                "extraction_version": EXTRACTION_VERSION, "startup_invocations": startup,
                "extractor": self.product.get("_extractor"), "source_inputs": list(self.audit.sources.values()),
                "acquisition_sha256": self.product["acquisition_sha"], "owners": self.owners,
                "display_roots": self.roots, "pages": self.pages, "failures": self.failures,
                "source_scope_complete": bool(self.pages) and not self.failures,
                "target_execution": False, "whole_capability_complete": False,
                "limitations": ["Guards and page props retain original owners; runtime values are not guessed.",
                                "Macro/armory/pairing display roots remain recorded outside the default/chroma page projection.",
                                "Navigation filters/click writers are retained, not a completed event/keyboard interaction audit."]}


def inspect_product(db_path, product):
    job = Navigation(db_path, product)
    try:
        return job.run()
    except (ValueError, KeyError, AttributeError, OSError) as error:
        return {"product_id": product["product_id"], "family": product["family"], "owners": job.owners,
                "extraction_version": EXTRACTION_VERSION, "acquisition_sha256": product["acquisition_sha"],
                "acquisition": product["acquisition"], "extractor": product.get("_extractor"),
                "source_inputs": list(job.audit.sources.values()),
                "pages": job.pages, "display_roots": job.roots, "source_scope_complete": False,
                "target_execution": False, "whole_capability_complete": False,
                "failures": [*job.failures, {"reason": str(error)}]}
    finally:
        job.close()


def build(db_path=ci.DATABASE, workers=2, products=None, refresh=False):
    if not 1 <= workers <= 4:
        raise ValueError("Navigation extraction requires one to four workers")
    with closing(ci.connect(db_path)) as db:
        all_scope = [dict(row) for row in db.execute("SELECT * FROM deployments WHERE layer='ui' AND failure IS NULL ORDER BY family,product_id")]
        scope = all_scope
        if products is not None:
            if set(products) - {row['product_id'] for row in all_scope}:
                raise ValueError('Navigation repair includes an unknown or unverified product')
            scope = [row for row in scope if row["product_id"] in products]
        db.executescript("""
            CREATE TABLE IF NOT EXISTS navigation_products(product_id INTEGER PRIMARY KEY, family TEXT, state TEXT, receipt TEXT);
            CREATE TABLE IF NOT EXISTS navigation_owners(owner_id TEXT PRIMARY KEY, source_sha TEXT, path TEXT,
                start_byte INTEGER,end_byte INTEGER,normalized_sha TEXT,raw_sha TEXT,syntax TEXT,roles TEXT);
            CREATE TABLE IF NOT EXISTS navigation_pages(product_id INTEGER,kind TEXT,mode TEXT,component_owner TEXT,
                condition_scope TEXT,evidence TEXT,PRIMARY KEY(product_id,evidence));
        """)
        previous = {row[0]: json.loads(row[1]) for row in db.execute("SELECT product_id,receipt FROM navigation_products")}
        pending = [row for row in scope if refresh or previous.get(row["product_id"], {}).get("extraction_version") != EXTRACTION_VERSION
                   or previous.get(row["product_id"], {}).get("acquisition_sha256") != row["acquisition_sha"]]
        for row in pending:
            row["_extractor"]={"path":a.rel(Path(__file__)),"sha256":a.sha(Path(__file__)),"version":EXTRACTION_VERSION}
        with ProcessPoolExecutor(max_workers=workers) as pool:
            for start in range(0, len(pending), workers):
                futures = [pool.submit(inspect_product, db_path, row) for row in pending[start:start + workers]]
                for future in futures:
                    report = future.result()
                    prior=previous.get(report['product_id'])
                    if prior and prior.get('failures'):
                        report['previous_unresolved']={'extractor':prior.get('extractor'),'failures':prior['failures']}
                    annotate_selection(report)
                    previous[report["product_id"]] = report
                    with db:
                        db.execute("INSERT OR REPLACE INTO navigation_products VALUES(?,?,?,?)", (report["product_id"],report["family"],
                                   "identified" if report["source_scope_complete"] else "partial" if report["pages"] else "unresolved", json.dumps(report,ensure_ascii=False)))
                        for owner_id, owner in report["owners"].items():
                            db.execute("INSERT OR REPLACE INTO navigation_owners VALUES(?,?,?,?,?,?,?,?,?)", (owner_id,
                                owner["original"]["sha256"],owner["original"]["path"],owner["start_byte"],owner["end_byte"],
                                owner["normalized_sha256"],owner["raw_sha256"],owner["syntax"],json.dumps(owner["roles"])))
                        _write_projection(db,report)
                print(json.dumps({"navigation_products":min(start+workers,len(pending)),"total":len(pending)}),flush=True)
        reports = [previous[row["product_id"]] for row in all_scope if row["product_id"] in previous]
        summary = {"schema_version":1,"kind":"current_mounted_navigation_identities","database":a.rel(Path(db_path)),
                   "requested_products":[row['product_id'] for row in scope],"updated_products":[row['product_id'] for row in pending],
                   "missing_products":[row['product_id'] for row in all_scope if row['product_id'] not in previous],
                   "products":len(reports),"identified":sum(r["source_scope_complete"] for r in reports),
                   "products_with_identified_pages":sum(bool(r["pages"]) for r in reports),
                   "pages":sum(len(r["pages"]) for r in reports),"target_execution":False,
                   "source_chain_complete":False,"unresolved":[{"product_id":r["product_id"],"failures":r["failures"]} for r in reports if not r["source_scope_complete"]]}
        destination=a.ROOT/".analysis/results/code-identity-navigation.json"
        a.save(destination,summary)
        return summary
