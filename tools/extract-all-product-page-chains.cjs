// Static current-source page graph. Parse vendor text as Acorn data only.
// A lexical/JSX reference is NOT proof that its condition executes at runtime.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), zlib = require('zlib');
const acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const sha = b => crypto.createHash('sha256').update(b).digest('hex');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const registration = JSON.parse(read('docs/re/product-registration-audit.json'));
const scannerHash=sha(fs.readFileSync(__filename)),registrationHash=sha(read('docs/re/product-registration-audit.json'));
const args = process.argv.slice(2), selected = args.find(a => /^--product=/.test(a));
const minimum=args.find(a=>/^--min-product=/.test(a)),maximum=args.find(a=>/^--max-product=/.test(a));
const rootOnly=args.includes('--refresh-unresolved-roots');
let refreshProducts;
if(rootOnly){const previous=JSON.parse(zlib.gunzipSync(fs.readFileSync(path.join(root,'docs/re/all-product-page-chains-current.json.gz'))).toString('utf8'));refreshProducts=new Set(previous.products.filter(p=>p.pages.some(page=>!page.components.length)).map(p=>p.product_id));}
const limit = 250;
const key = n => n?.name ?? n?.value;
function walk(n, f) {
  if (!n?.type || f(n) === false) return;
  for (const v of Object.values(n)) {
    if (Array.isArray(v)) {for (const c of v) if (c?.type) walk(c, f);}
    else if (v?.type) walk(v, f);
  }
}
function patternNames(pattern) {
  if(!pattern)return [];
  if(pattern.type==='Identifier')return [pattern.name];
  if(pattern.type==='RestElement')return patternNames(pattern.argument);
  if(pattern.type==='AssignmentPattern')return patternNames(pattern.left);
  if(pattern.type==='ArrayPattern')return pattern.elements.flatMap(patternNames);
  if(pattern.type==='ObjectPattern')return pattern.properties.flatMap(p=>patternNames(p.type==='RestElement'?p.argument:p.value));
  return [];
}
class Corpus {
  constructor(pid) {
    this.pid = pid; this.files = new Map(); this.modules = new Map(); this.moduleConflicts=new Map();this.missingModules=new Set();this.nodeScopes = new WeakMap();this.requireFunctions=new WeakMap();this.externalGlobals=new Map();this.externalLoaded=false;
    this.manifest = JSON.parse(read(`.ref/devices/${pid}/asset-manifest.json`));
    this.js = [...new Set(Object.values(this.manifest.files))].filter(f => /\.js$/.test(f)).map(f => `.ref/devices/${pid}/${f.replace(/^\.\//, '').replace(/^\/?synapse\/products\/\d+\/ui\//,'')}`);
    // Establish the whole product module-id namespace before tracing any page,
    // so a later lazy chunk cannot reveal a conflict after a target was chosen.
    for(const file of this.js)this.file(file);
  }
  file(file) {
    if (this.files.has(file)) return this.files.get(file);
    const text = read(file), ast = acorn.parse(text, {ecmaVersion:'latest', sourceType:'script'});
    const data = {file, text, sha256:sha(text), scopes:[],ast};
    this.files.set(file, data);
    const tables=new WeakSet(),pendingExports=[];
    // External UMD scripts own separate loaders/module-id namespaces. Their
    // numeric maps must never overwrite a product's webpack factory table.
    if(this.js.includes(file))walk(ast,n=>{if(n.type==='ObjectExpression'&&n.properties.length&&n.properties.every(p=>p.type==='Property'&&Number.isInteger(key(p.key))&&/FunctionExpression$/.test(p.value?.type)))tables.add(n);});
    const exportAssignment=(module,name,node,evidence,kind,scope,requirements)=>{
      if(node?.type==='UnaryExpression'&&node.operator==='void')return;
      pendingExports.push({module,name,node,evidence,kind,scope,requirements});
    };
    const defaultGetter=n=>n?.type==='ObjectExpression'?n.properties.find(p=>key(p.key)==='get')?.value:null;
    const returnValue=fn=>fn?.body?.type==='BlockStatement'?fn.body.body.find(n=>n.type==='ReturnStatement')?.argument:fn?.body;
    const visit = (n, scope, module, parent) => {
      if (!n?.type) return;
      if (n.type === 'Property' && tables.has(parent)) module = {id:key(n.key),fn:n.value,file,require:n.value.params[2]?.name,exportsName:n.value.params[1]?.name,moduleName:n.value.params[0]?.name,exports:new Map(),factory:n};
      if (n.type === 'VariableDeclarator') {
        let declarationScope=scope;
        if(parent?.kind==='var')while(declarationScope&&!['function','program'].includes(declarationScope.kind))declarationScope=declarationScope.parent;
        if(n.id.type==='Identifier')declarationScope?.defs.set(n.id.name,n.init);
        else for(const name of patternNames(n.id))declarationScope?.defs.set(name,null);
      }
      if (['FunctionDeclaration','ClassDeclaration'].includes(n.type)) scope?.defs.set(n.id.name,n);
      if (n.type === 'Program' || /Function/.test(n.type)) {
        scope = {parent:scope,defs:new Map(),module,file,kind:n.type==='Program'?'program':'function',start:n.start,end:n.end}; data.scopes.push(scope);
        for (const p of n.params ?? []) for(const name of patternNames(p))scope.defs.set(name,null);
        if(n.type==='FunctionExpression'&&n.id?.name)scope.defs.set(n.id.name,n);
        if (module?.fn === n) {
          module.scope=scope;const existing=this.modules.get(module.id);
          if(existing&&this.file(existing.file).text.slice(existing.fn.start,existing.fn.end)!==text.slice(n.start,n.end)) {
            this.moduleConflicts.set(module.id,[existing,module]);this.modules.delete(module.id);
          } else if(!existing&&!this.moduleConflicts.has(module.id))this.modules.set(module.id,module);
        }
      }
      if(['BlockStatement','ForStatement','ForInStatement','ForOfStatement','CatchClause','SwitchStatement'].includes(n.type)) {
        scope={parent:scope,defs:new Map(),module,file,kind:'block',start:n.start,end:n.end};data.scopes.push(scope);
        if(n.type==='CatchClause')for(const name of patternNames(n.param))scope.defs.set(name,null);
      }
      this.nodeScopes.set(n,scope);
      if (module && n.type === 'CallExpression' && n.callee.type === 'MemberExpression' && n.callee.object.name === module.require && key(n.callee.property) === 'd' && n.arguments[0]?.name===module.exportsName && n.arguments[1]?.type === 'ObjectExpression') {
        for (const p of n.arguments[1].properties) {
          const getter=p.value; let value=getter.body;
          if(value?.type==='BlockStatement') value=value.body.find(s=>s.type==='ReturnStatement')?.argument;
          exportAssignment(module,key(p.key),value,p,'webpack_export_getter',scope,[module.require,module.exportsName]);
        }
      }
      if(module&&n.type==='AssignmentExpression'&&n.operator==='='&&n.left.type==='MemberExpression') {
        const member=n.left;
        if(member.object.name===module.exportsName&&(!member.computed||typeof member.property.value==='string'))exportAssignment(module,key(member.property),n.right,n,'commonjs_named_assignment',scope,[module.exportsName]);
        if(member.object.name===module.moduleName&&key(member.property)==='exports')exportAssignment(module,'<module.exports>',n.right,n,'commonjs_module_assignment',scope,[module.moduleName]);
      }
      if(module&&n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&n.callee.object.name==='Object'&&key(n.callee.property)==='defineProperty'
        &&n.arguments[0]?.name===module.exportsName&&typeof n.arguments[1]?.value==='string') {
        const getter=defaultGetter(n.arguments[2]),value=returnValue(getter);
        if(value&&!this.lookup('Object',scope))exportAssignment(module,n.arguments[1].value,value,n,'defineProperty_export_getter',scope,[module.exportsName]);
      }
      for (const v of Object.values(n)) {
        if (Array.isArray(v)) {for (const c of v) if (c?.type) visit(c,scope,module,n);}
        else if (v?.type) visit(v,scope,module,n);
      }
    };
    visit(ast,null,null,null);
    for(const item of pendingExports) {
      if(!item.requirements.every(name=>{const binding=this.lookup(name,item.scope);return binding?.scope===item.module.scope&&binding.node===null;}))continue;
      if(item.kind==='defineProperty_export_getter'&&this.lookup('Object',item.scope))continue;
      const candidates=item.module.exports.get(item.name)??[];
      candidates.push({node:item.node,evidence:item.evidence,kind:item.kind});item.module.exports.set(item.name,candidates);
    }
    return data;
  }
  module(id) {
    if(this.moduleConflicts.has(id))return null;
    if (this.modules.has(id)) return this.modules.get(id);
    if(this.missingModules.has(id))return null;
    const locator=new RegExp(`(?:[,{])\\s*${id}\\s*(?::|\\()`);
    for (const file of this.js) {
      if (this.files.has(file) || !locator.test(read(file))) continue;
      this.file(file); if(this.modules.has(id))return this.modules.get(id);
    }
    this.missingModules.add(id);return null;
  }
  lookup(name,scope) {while(scope){if(scope.defs.has(name))return {node:scope.defs.get(name),scope};scope=scope.parent;}return null;}
  isRequire(name,scope) {
    const local=this.lookup(name,scope);
    if(name&&name===scope?.module?.require&&local?.scope===scope.module.scope&&local.node===null)return {kind:'webpack_factory_require_parameter',node:scope.module.fn.params[2],scope:scope.module.scope};
    const fn=this.lookup(name,scope)?.node;if(!fn||!/Function/.test(fn.type))return false;
    if(this.requireFunctions.has(fn))return this.requireFunctions.get(fn);
    let valid=false;
    walk(fn.body,n=>{if(n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&n.callee.computed&&n.arguments.length===3&&n.arguments[0].type==='Identifier'&&n.arguments[1].type==='MemberExpression'&&n.arguments[1].object.name===n.arguments[0].name&&key(n.arguments[1].property)==='exports'&&n.arguments[2].type==='Identifier'&&n.arguments[2].name===name)valid={kind:'webpack_runtime_factory_invocation',node:n,scope:this.nodeScopes.get(n)};});
    this.requireFunctions.set(fn,valid);return valid;
  }
  loadExternalGlobals() {
    if(this.externalLoaded)return;this.externalLoaded=true;
    const htmlPath=`.ref/devices/${this.pid}/index.html`,html=read(htmlPath);
    for(const match of html.matchAll(/<script\b[^>]*\bsrc="([^"]+)"[^>]*>/g)) {
      const url=match[1];if(!url.startsWith('/synapse/assets/'))continue;
      const file='.ref/applications'+url;
      if(!fs.existsSync(path.join(root,file)))continue;
      const data=this.file(file);
      walk(data.ast,n=>{
        if(n.type!=='CallExpression'||!/FunctionExpression$/.test(n.callee.type))return;
        for(let argIndex=0;argIndex<n.arguments.length;argIndex++) {
          const factory=n.arguments[argIndex],factoryName=n.callee.params[argIndex]?.name;
          if(!factoryName||!/FunctionExpression$/.test(factory?.type))continue;
          walk(n.callee.body,call=>{
            if(call.type!=='CallExpression'||call.callee.name!==factoryName)return;
            const assignment=call.arguments[0];
            if(assignment?.type!=='AssignmentExpression'||assignment.left.type!=='MemberExpression'||assignment.right.type!=='ObjectExpression')return;
            const globalName=key(assignment.left.property),exportParam=factory.params[0]?.name,exports=new Map(),dependencies=new Map();
            if(typeof globalName!=='string'||!exportParam)return;
            for(let index=1;index<call.arguments.length;index++)if(factory.params[index]?.name&&call.arguments[index]?.type==='MemberExpression')dependencies.set(factory.params[index].name,key(call.arguments[index].property));
            const factoryScope=this.nodeScopes.get(factory);
            walk(factory.body,node=>{if(node.type==='AssignmentExpression'&&node.operator==='='&&node.left.type==='MemberExpression'&&node.left.object.name===exportParam
              &&(!node.left.computed||typeof node.left.property.value==='string')){
              const binding=this.lookup(exportParam,this.nodeScopes.get(node));if(binding?.scope!==factoryScope||binding.node!==null)return;
              const name=key(node.left.property),list=exports.get(name)??[];list.push({node:node.right,evidence:node,kind:'umd_global_named_assignment'});exports.set(name,list);}});
            if(this.externalGlobals.has(globalName))return;
            this.externalGlobals.set(globalName,{globalName,exports,dependencies,scope:this.nodeScopes.get(factory),file,
              html:{path:htmlPath,sha256:sha(html),offset:match.index,end:match.index+match[0].length,source:match[0]},umd:n});
          });
        }
      });
    }
  }
  page(nav,item) {
    const data=this.file(nav.source);
    if(data.sha256!==nav.sha256)throw Error(`Changed navigation ${nav.source}`);
    const scope=data.scopes.filter(s=>s.start<=item.offset&&s.end>item.offset).sort((a,b)=>(a.end-a.start)-(b.end-b.start))[0];
    const nodes=[],edges=[],unresolved=[],resolutions=[],seen=new Map(),resolving=new Set();let truncated=false;
    const sourceFile=s=>s?.file??s?.module?.file??nav.source;
    const snippet=(n,s)=>this.file(sourceFile(s)).text.slice(n.start,n.end);
    const ref=(n,s)=>({path:sourceFile(s),sha256:this.file(sourceFile(s)).sha256,offset:n.start,end:n.end,source:snippet(n,s)});
    const proof=(kind,from,expression,evidence,target)=>{const value={kind,from,expression,evidence,target};resolutions.push(value);};
    const missing=(expression,reason,from,candidate_factories)=>{unresolved.push({from,expression,reason,...candidate_factories?{candidate_factories}:null});};
    const uniqueValue=values=>{if(!values?.length)return null;const concrete=values.filter(v=>!(v.node.type==='UnaryExpression'&&v.node.operator==='void'));return concrete.length===1?concrete[0]:null;};
    const globalExport=(globalName,exportName,from)=>{
      this.loadExternalGlobals();const global=this.externalGlobals.get(globalName),value=uniqueValue(global?.exports.get(exportName));
      if(!value){missing(`${globalName}.${exportName}`,global?'umd_export_not_uniquely_resolved':'global_provider_not_proved',from);return;}
      proof(value.kind,from,`${globalName}.${exportName}`,[global.html,ref(value.evidence,this.nodeScopes.get(value.evidence)??global.scope)],{global:globalName,property:exportName});
      resolve(value.node,this.nodeScopes.get(value.node)??global.scope,from,`${globalName}.${exportName}`);
    };
    const resolveNamespace=(n,s,name,from,evidence)=>{
      if(n.type==='CallExpression'&&this.isRequire(n.callee.name,s)&&Number.isInteger(n.arguments[0]?.value)) {proof('commonjs_require_reexport',from,`${name}`,evidence,{module_id:n.arguments[0].value,export:name});exported(n.arguments[0].value,name,from);return true;}
      if(n.type==='Identifier') {
        this.loadExternalGlobals();
        const b=this.lookup(n.name,s);
        if(!b&&this.externalGlobals.has(n.name)){proof('commonjs_global_export',from,`${n.name}.${name}`,evidence,{global:n.name,property:name});globalExport(n.name,name,from);return true;}
        if(b?.node)return resolveNamespace(b.node,b.scope,name,from,[...evidence,ref(b.node,b.scope)]);
      }
      if(n.type==='ObjectExpression') {const members=n.properties.filter(p=>key(p.key)===name&&!p.computed);if(members.length===1){proof('commonjs_object_export',from,name,[...evidence,ref(members[0],s)],{property:name});resolve(members[0].value,s,from,name);return true;}}
      return false;
    };
    const exported=(moduleId,exportName,from)=>{
      const identity=`export:${moduleId}:${exportName}`;if(resolving.has(identity)){missing(`${moduleId}:${exportName}`,'cyclic_export_reference',from);return;}resolving.add(identity);
      try{
      const module=this.module(moduleId);
      if(!module){const conflicts=this.moduleConflicts.get(moduleId);missing(`${moduleId}:${exportName}`,conflicts?'module_conflicting_factories':'module_not_located',from,
        conflicts?.map(m=>({path:m.file,sha256:this.file(m.file).sha256,offset:m.fn.start,end:m.fn.end,module_id:moduleId,node_type:m.fn.type})));return;}
      const value=uniqueValue(module.exports.get(exportName));
      if(value){proof(value.kind,from,`${moduleId}:${exportName}`,[ref(value.evidence,module.scope)],{module_id:moduleId,export:exportName});resolve(value.node,this.nodeScopes.get(value.node)??module.scope,from,`module:${moduleId}:${exportName}`);return;}
      const common=uniqueValue(module.exports.get('<module.exports>'));
      if(common){if(resolveNamespace(common.node,module.scope,exportName,from,[ref(common.evidence,module.scope)]))return;
        if(exportName==='default'){proof('commonjs_default_value',from,`${moduleId}:default`,[ref(common.evidence,module.scope)],{module_id:moduleId});resolve(common.node,module.scope,from,`module:${moduleId}:module.exports`);return;}}
      missing(`${moduleId}:${exportName}`,'export_not_resolved',from);
      }finally{resolving.delete(identity);}
    };
    const imported=(n,s,visited=new Set())=> {
      if(n?.type==='CallExpression') {
        const require=this.isRequire(n.callee.name,s);
        if(require&&Number.isInteger(n.arguments[0]?.value))return {id:n.arguments[0].value,evidence:[ref(n,s),ref(require.node,require.scope)],kind:'webpack_require'};
        if(n.arguments.length===1&&n.arguments[0]?.type==='CallExpression') {
          const inner=imported(n.arguments[0],s,visited);let helper=this.lookup(n.callee.name,s)?.node;
          if(helper?.type==='CallExpression'&&this.isRequire(helper.callee.name,s)&&Number.isInteger(helper.arguments[0]?.value)) {
            const helperModule=this.module(helper.arguments[0].value);
            helper=uniqueValue(helperModule?.exports.get('<module.exports>'))?.node;
          }
          if(inner&&helper&&/Function/.test(helper.type)) {
            const param=helper.params[0]?.name,returned=helper.body.type==='BlockStatement'?helper.body.body.find(n=>n.type==='ReturnStatement')?.argument:helper.body;
            const test=returned?.test,check=test?.type==='LogicalExpression'&&test.operator==='&&'&&test.left.name===param?test.right:null;
            const interop=returned?.type==='ConditionalExpression'&&check?.type==='MemberExpression'&&check.object.name===param&&key(check.property)==='__esModule'
              &&returned.consequent.name===param&&returned.alternate.type==='ObjectExpression'&&returned.alternate.properties.length===1
              &&key(returned.alternate.properties[0].key)==='default'&&returned.alternate.properties[0].value.name===param;
            if(interop)return {...inner,kind:'babel_esm_default_interop',evidence:[...inner.evidence,ref(helper,this.nodeScopes.get(helper)??s)]};
          }
        }
        return null;
      }
      if(n?.type!=='Identifier')return null;
      const b=this.lookup(n.name,s),v=b?.node;
      if(!v||visited.has(v))return null;visited=new Set([...visited,v]);
      if(v.type==='Identifier')return imported(v,b.scope,visited);
      if(v.type==='CallExpression')return imported(v,b.scope,visited);
      return null;
    };
    const resolve=(n,s,from,label)=> {
      if(!n)return;
      if(n.type==='Identifier') {
        const b=this.lookup(n.name,s);
        if(!b?.node){missing(n.name,'lexical_binding_or_parameter_not_resolved',from);return;}
        resolve(b.node,b.scope,from,n.name);return;
      }
      if(n.type==='MemberExpression') {
        const importedRef=imported(n.object,s);
        if(importedRef!==null){const expression=n.object.type==='Identifier'&&!n.computed?`${n.object.name}.${key(n.property)}`:snippet(n,s);proof(importedRef.kind,from,expression,importedRef.evidence,{module_id:importedRef.id,export:key(n.property)});exported(importedRef.id,key(n.property),from);return;}
        if(!n.computed&&n.object.type==='Identifier') {
          this.loadExternalGlobals();const b=this.lookup(n.object.name,s);let globalName=!b&&this.externalGlobals.has(n.object.name)?n.object.name:null;
          if(!globalName)for(const global of this.externalGlobals.values())if(global.file===sourceFile(s)&&b?.scope===global.scope&&b.node===null)globalName=global.dependencies.get(n.object.name)??null;
          if(globalName){globalExport(globalName,key(n.property),from);return;}
          if(b?.node?.type==='ObjectExpression') {
            const props=b.node.properties.filter(p=>!p.computed&&key(p.key)===key(n.property));
            if(props.length===1){proof('object_literal_member',from,snippet(n,s),[ref(props[0],b.scope)],{property:key(n.property)});resolve(props[0].value,b.scope,from);return;}
          }
        }
        missing(n.object.type==='Identifier'&&!n.computed?`${n.object.name}.${key(n.property)}`:snippet(n,s),'member_component_not_resolved',from);return;
      }
      if(n.type==='Literal')return;
      const file=sourceFile(s),identity=`${file}:${n.start}:${n.end}`;
      if(seen.has(identity)){edges.push({from,to:seen.get(identity)});return;}
      if(nodes.length>=limit){truncated=true;return;}
      const index=nodes.length;seen.set(identity,index);edges.push({from,to:index});
      const state=[],calls=[],classes=[],conditions=[],lazy=[];
      const receipt={symbol:label??n.type,path:file,sha256:this.file(file).sha256,offset:n.start,end:n.end,node_type:n.type,state,calls,css_classes:classes,conditions,lazy_modules:lazy};nodes.push(receipt);
      const frameworkToken=n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&n.callee.object.name==='Symbol'&&key(n.callee.property)==='for'&&typeof n.arguments[0]?.value==='string'&&n.arguments[0].value.startsWith('react.');
      if(frameworkToken){receipt.framework_token=n.arguments[0].value;proof('react_symbol_token',from,receipt.framework_token,[ref(n,s)],{kind:'framework_symbol',value:receipt.framework_token});}
      if(n.type==='CallExpression'&&!frameworkToken)for(const a of n.arguments)if(['Identifier','FunctionExpression','ArrowFunctionExpression','CallExpression','MemberExpression'].includes(a.type))resolve(a,this.nodeScopes.get(a)??s,index);
      walk(n,c=>{
        const cs=this.nodeScopes.get(c)??s;
        if(c.type==='CallExpression') {
          const name=c.callee.type==='MemberExpression'?key(c.callee.property):c.callee.name;
          if((c.callee.type==='SequenceExpression'&&['jsx','jsxs','createElement'].includes(key(c.callee.expressions.at(-1)?.property)))||['jsx','jsxs','createElement'].includes(name))resolve(c.arguments[0],this.nodeScopes.get(c.arguments[0])??cs,index);
          if(name==='bind'&&Number.isInteger(c.arguments[1]?.value)&&this.isRequire(c.callee.object?.name,cs)){lazy.push({module_id:c.arguments[1].value,offset:c.start});exported(c.arguments[1].value,'default',index);}
          if(['useState','useReducer','useSelector','setState','dispatch','useEffect','useMemo','useCallback','subscribe','unsubscribe'].includes(name))state.push({method:name,offset:c.start,end:c.end,expression:snippet(c,cs).slice(0,240)});
          if(typeof name==='string'&&/^(get|set|query|read|write|save|load|apply|send|remove|delete|add|update|fetch|on|emit|publish|request|register|unregister|reset|open|close|start|stop|connect|disconnect|invoke|execute)[A-Z_]/.test(name))calls.push({method:name,offset:c.start,end:c.end,expression:snippet(c,cs).slice(0,240)});
        }
        if(c.type==='Property'&&key(c.key)==='className'&&c.value.type==='Literal'&&typeof c.value.value==='string')classes.push({value:c.value.value,offset:c.value.start});
        if(c.type==='ConditionalExpression'||c.type==='IfStatement') {const t=c.test;conditions.push({offset:t.start,end:t.end,expression:snippet(t,cs).slice(0,240)});}
      });
    };
    let expr,recovered,recoveredRenderView;
    if(item.component===null){
      let navName;
      walk(data.ast,n=>{if(n.type==='ObjectExpression'&&n.start===item.offset)navName=n.properties.find(p=>key(p.key)==='name')?.value;});
      const owner=this.lookup(nav.owner,scope)?.node,nameExpression=navName?data.text.slice(navName.start,navName.end):null,candidates=[];
      if(owner&&nameExpression)walk(owner,n=>{if(n.type==='Property'&&n.computed&&data.text.slice(n.key.start,n.key.end)===nameExpression&&n.value.type==='CallExpression')candidates.push(n.value);});
      if(candidates.length===1){expr=candidates[0];recovered={path:nav.source,sha256:data.sha256,offset:expr.start,end:expr.end,expression:data.text.slice(expr.start,expr.end),key_expression:nameExpression,resolution:'unique_owner_computed_router_key_matches_navigation_name'};}
      else {
       const render=owner?.type==='ClassDeclaration'?owner.body.body.find(m=>key(m.key)==='render'):null,methods=[],renderCalls=[];
       if(render)walk(render,n=>{if(n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&n.callee.object.type==='ThisExpression'&&key(n.callee.property)==='renderView')renderCalls.push(n);});
       if(owner)walk(owner,n=>{if(n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&n.left.object.type==='ThisExpression'&&key(n.left.property)==='renderView'&&/Function/.test(n.right.type))methods.push(n.right);});
       if(renderCalls.length===1&&methods.length===1&&nameExpression){
        const method=methods[0],guards=[];
        walk(method.body,n=>{if(n.type==='IfStatement'&&n.test.type==='BinaryExpression'&&n.test.operator==='==='&&[n.test.left,n.test.right].some(v=>data.text.slice(v.start,v.end)===nameExpression))guards.push(n);});
        if(guards.length===1){
         const guard=guards[0],returns=[];walk(guard.consequent,n=>{if(n!==guard.consequent&&/Function/.test(n.type))return false;if(n.type==='ReturnStatement'&&n.argument)returns.push(n.argument);});
         const returned=returns.length===1?returns[0]:null,jsxs=[];
         if(returned)walk(returned,n=>{if(n.type==='CallExpression'&&n.callee.type==='SequenceExpression'&&['jsx','jsxs'].includes(key(n.callee.expressions.at(-1)?.property)))jsxs.push(n);});
         const hasNavLookup=method.body.body.some(statement=>{let found=false;walk(statement,n=>{if(n.type==='CallExpression'&&key(n.callee.property)==='find'&&n.callee.object.type==='MemberExpression'&&key(n.callee.object.property)==='navs')found=true;});return found;});
         if(jsxs.length===1&&hasNavLookup){
          expr=jsxs[0];recoveredRenderView=method;
          const receipt=n=>({path:nav.source,sha256:data.sha256,offset:n.start,end:n.end,expression:data.text.slice(n.start,n.end)});
          recovered={...receipt(expr),key_expression:nameExpression,resolution:'owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX',render:receipt(render),render_view:receipt(method),render_call:receipt(renderCalls[0]),navigation_name_guard:receipt(guard.test),return_expression:receipt(returned)};
         }
        }
       }
       if(!expr)missing(nameExpression??item.name?.value,`computed_router_root_candidates_${candidates.length}`,null);
      }
    } else try {expr=acorn.parseExpressionAt(item.component,0,{ecmaVersion:'latest'});}catch(error){missing(item.component,'root_expression_parse_error',null);}
    if(expr?.type==='SequenceExpression')expr=expr.expressions.at(-1);
    if(expr?.type==='ArrowFunctionExpression'&&expr.body.type==='CallExpression')expr=expr.body;
    else if(expr?.type==='FunctionExpression'&&expr.body.body.length===1&&expr.body.body[0].type==='ReturnStatement')expr=expr.body.body[0].argument;
    const component=expr?.type==='CallExpression'?expr.arguments[0]:null;
    // The root expression is a preserved snippet: resolve identifiers in the
    // real navigation lexical scope, not the snippet's artificial offsets.
    if(expr?.type==='ConditionalExpression') {
      const branches=[expr.consequent,expr.alternate];
      for(const branch of branches){if(branch.type==='CallExpression')resolve(branch.arguments[0],scope,null);}
      let actual;walk(data.ast,n=>{if(n.type==='ObjectExpression'&&n.start===item.offset)actual=n.properties.find(p=>key(p.key)==='component')?.value;});
      if(actual&&data.text.slice(actual.start,actual.end)===item.component)recovered={resolution:'conditional_JSX_root_both_branches_preserved',path:nav.source,sha256:data.sha256,offset:actual.start,end:actual.end,expression:item.component,condition_expression:data.text.slice(actual.test.start,actual.test.end),branches:[actual.consequent,actual.alternate].map(n=>({offset:n.start,end:n.end,expression:data.text.slice(n.start,n.end)}))};
    }else if(component?.type==='Identifier')resolve(component,scope,null,component.name);
    else if(component?.type==='MemberExpression')resolve(component,scope,null);
    else missing(item.component,'root_component_shape_not_resolved',null);
    if(expr)walk(expr,c=>{if(c.type==='CallExpression'&&key(c.callee.property)==='bind'&&Number.isInteger(c.arguments[1]?.value)&&this.isRequire(c.callee.object?.name,scope))exported(c.arguments[1].value,'default',null);});
    if(recoveredRenderView)resolve(recoveredRenderView,this.nodeScopes.get(recoveredRenderView)??scope,null,'this.renderView');
    return {page_id:item.key,page_key:item.name?.value,display_mode:nav.display_mode,primary:nav.primary,navigation:{path:nav.source,sha256:nav.sha256,offset:nav.offset,item_offset:item.offset,component:item.component,recovered_component:recovered,root_evidence:nav.root_evidence},components:nodes,edges,resolutions:[...new Map(resolutions.map(r=>[JSON.stringify(r),r])).values()],unresolved:[...new Map(unresolved.map(u=>[JSON.stringify(u),u])).values()],truncated,trace_status:nodes.length?'partial_static_reference_graph':'unresolved_root',semantic_review:'not_completed_by_graph',runtime_validation:'not_run',dll_writeback:'deferred'};
  }
}
module.exports={Corpus,walk,key};
if(require.main===module){
const products=[];
for(const product of registration.products.filter(p=>(!selected||p.product_id===Number(selected.split('=')[1]))&&(!minimum||p.product_id>=Number(minimum.split('=')[1]))&&(!maximum||p.product_id<=Number(maximum.split('=')[1])))) {
  const checkpoint=`.work/product-page-chain-cache/${product.product_id}.json.gz`;
  if(fs.existsSync(path.join(root,checkpoint))){
   const saved=JSON.parse(zlib.gunzipSync(fs.readFileSync(path.join(root,checkpoint))).toString('utf8'));
   if(saved.registration_sha256===registrationHash&&saved.scanner_sha256===scannerHash){
    // Migration from the initial graph scanner is restricted to non-null roots;
    // semantic enhancements concern computed router and function-wrapped roots.
    // Verify all
    // source receipts before retaining any cached graph.
    const seenFiles=new Map();for(const page of saved.product.pages)for(const ref of [page.navigation,...page.components]){if(!seenFiles.has(ref.path))seenFiles.set(ref.path,sha(read(ref.path)));if(seenFiles.get(ref.path)!==ref.sha256)throw Error(`Changed cached source ${ref.path}`);}
    for(const page of saved.product.pages){const nav=product.navigation.find(n=>n.items.some(i=>i.key===page.page_id));page.navigation.root_evidence=nav.root_evidence;}
    products.push(saved.product);console.log(`Product ${product.product_id}: resumed verified registration/current-source checkpoint`);continue;
   }
   if(rootOnly&&saved.registration_sha256===registrationHash&&!refreshProducts.has(product.product_id)){
    const seenFiles=new Map();for(const page of saved.product.pages)for(const ref of [page.navigation,...page.components]){if(!seenFiles.has(ref.path))seenFiles.set(ref.path,sha(read(ref.path)));if(seenFiles.get(ref.path)!==ref.sha256)throw Error(`Changed retained source ${ref.path}`);}
    saved.product.retained_graph_producer_sha256=saved.product.retained_graph_producer_sha256??saved.scanner_sha256;
    saved.product.current_update_scope='retained_source_verified_graph_unaffected_by_unresolved_root_refresh';
    const migrated={scanner_sha256:scannerHash,registration_sha256:registrationHash,product:saved.product};
    fs.writeFileSync(path.join(root,checkpoint),zlib.gzipSync(Buffer.from(JSON.stringify(migrated)),{level:9}));
    products.push(saved.product);console.log(`Product ${product.product_id}: retained unaffected graph with explicit original producer`);continue;
   }
  }
  const corpus=new Corpus(product.product_id),pages=[];
  for(const nav of product.navigation)for(const item of nav.items)pages.push(corpus.page(nav,item));
  const record={product_id:product.product_id,name:product.name,categories:product.categories,edition_ids:product.edition_ids,connection_aliases:product.connection_aliases,pages};products.push(record);
  fs.mkdirSync(path.dirname(path.join(root,checkpoint)),{recursive:true});fs.writeFileSync(path.join(root,checkpoint),zlib.gzipSync(Buffer.from(JSON.stringify({scanner_sha256:scannerHash,registration_sha256:registrationHash,product:record})),{level:9}));
  process.stdout.write(`Product ${product.product_id}: ${pages.length} roots, ${pages.reduce((n,p)=>n+p.components.length,0)} component references\n`);
}
const pages=products.flatMap(p=>p.pages),counts=list=>list.reduce((result,key)=>(result[key]=(result[key]??0)+1,result),{}),summary={products:products.length,primary_pages:pages.filter(p=>p.primary).length,independent_pages:pages.filter(p=>!p.primary).length,resolved_graph_roots:pages.filter(p=>p.components.length).length,unresolved_roots:pages.filter(p=>!p.components.length).length,truncated_graphs:pages.filter(p=>p.truncated).length,component_receipts:pages.reduce((n,p)=>n+p.components.length,0),resolution_receipts:pages.reduce((n,p)=>n+(p.resolutions?.length??0),0),resolution_kinds:counts(pages.flatMap(p=>(p.resolutions??[]).map(r=>r.kind))),unresolved_reasons:counts(pages.flatMap(p=>p.unresolved.map(u=>u.reason))),semantic_complete_pages_claimed:0};
const output={schema_version:1,source_date:'2026-10-02',scope:'All registered current product navigation roots; bounded lexical/JSX/import/lazy references, not execution or complete reverse engineering',scanner_sha256:scannerHash,registration_sha256:registrationHash,summary,limitations:['A reference graph does not establish branch reachability, state transitions, service message contracts, CSS cascade or visual parity.','Unresolved lexical parameters/member exports and bounded graphs are preserved explicitly.','Method prefixes are candidate observations only: exact snippets/offsets must be reviewed before assigning read/write/service meaning.','No application, vendor JavaScript, DLL, build or test executed.'],products};
const target=selected?`.work/product-${selected.split('=')[1]}-page-chains.json`:minimum||maximum?`.work/product-range-${minimum?.split('=')[1]??'start'}-${maximum?.split('=')[1]??'end'}-page-chains.json`:'docs/re/all-product-page-chains-current.json';
fs.mkdirSync(path.dirname(path.join(root,target)),{recursive:true});
if(selected||minimum||maximum)fs.writeFileSync(path.join(root,target),JSON.stringify(output,null,2)+'\n');
else {
 const full=Buffer.from(JSON.stringify(output)+'\n'),compressed=zlib.gzipSync(full,{level:9});
 fs.writeFileSync(path.join(root,target+'.gz'),compressed);
 const index={...output,data_file:path.basename(target)+'.gz',data_sha256:sha(compressed),uncompressed_sha256:sha(full),uncompressed_bytes:full.length,products:products.map(p=>({...p,pages:p.pages.map(({components,edges,resolutions,unresolved,...page})=>({...page,component_receipts:components.length,resolution_receipts:resolutions?.length??0,unresolved_references:unresolved.length,unresolved_reasons:[...new Set(unresolved.map(u=>u.reason))]}))}))};
 fs.writeFileSync(path.join(root,target),JSON.stringify(index,null,2)+'\n');
}
console.log(JSON.stringify(summary));
}
