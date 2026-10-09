// Full current application caller-side native/process audit. Static AST only.
const fs=require('fs'),path=require('path'),zlib=require('zlib'),acorn=require('acorn');
const {walk,key,hash}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..'),read=p=>fs.readFileSync(path.join(root,p));
const catalogPath='docs/re/application-catalog.json',indexPath='docs/re/all-application-chains-current.json';
const catalog=JSON.parse(read(catalogPath)),index=JSON.parse(read(indexPath));
const graph=JSON.parse(zlib.gunzipSync(read('docs/re/'+index.data_file)));
const origins=new Map();for(const app of catalog.applications)for(const f of app.files||[])if(f.result==='ok'&&/\.js$/.test(f.path)){
  if(!origins.has(f.path))origins.set(f.path,[]);origins.get(f.path).push({route:app.route,sha256:f.sha256,url:f.url});
}
const outputs=[],receipts=new Map(),semanticAnchors=[];
const peDocs=['docs/re/native-library-pe-current-evidence.json','docs/re/host-native-library-pe-current-evidence.json'];
const peRows=peDocs.flatMap(file=>{const e=JSON.parse(read(file));return(e.resources||e.files).map(r=>({...r,evidence:file}));});
// Points are from manually followed current source, not old minified symbols.
// Select complete AST bodies/declarations for reproducible semantic review.
const semanticPlan={
  '.ref/applications/alisha/static/js/main.83ea24ca.js':[
    ['alisha-updater-config-and-actions',840086,/ArrowFunctionExpression/],
    ['alisha-updater-path-input',840040,/VariableDeclarator/],
    ['alisha-updater-wrapper',644759,/ClassExpression/]],
  '.ref/applications/sophie-lite/static/js/main.bb22144c.chunk.js':[
    ['lite-audio-wrapper',314492,/ClassExpression/],['lite-updater-wrapper',438592,/ClassExpression/],
    ['lite-updater-init-migration',443682,/FunctionDeclaration/],
    ['lite-updater-native-path',443565,/VariableDeclarator/],
    ['lite-audio-root-init',520429,/CallExpression/],['lite-audio-root-path-input',520390,/VariableDeclarator/]],
  '.ref/applications/sophie/static/js/main.0dad7d2f.chunk.js':[
    ['sophie-updater-init-migration',533786,/FunctionExpression/],
    ['sophie-audio-init-path',718490,/FunctionExpression/],
    ['sophie-audio-caller-and-event-consumption',718700,/FunctionExpression/],
    ['sophie-audio-owned-instance',718470,/VariableDeclarator/]],
  '.ref/applications/natalie/static/js/main.35e04e8c.chunk.js':[
    ['natalie-init-native-path-procs-and-initial-state',369318,/FunctionExpression/],
    ['natalie-core-factory',372008,/FunctionExpression/],
    ['natalie-updater-init-migration',379582,/FunctionExpression/]],
  '.ref/applications/synapse/chroma-studio/static/js/main.6b22e9cc.js':[
    ['studio-native-wrapper-configure-return-events-free',233238,/ClassExpression/]],
  '.ref/applications/synapse/update-fw/static/js/main.69cc5fbd.js':[
    ['firmware-unzip-helper-arguments-hash-result-and-cleanup',1853383,/ArrowFunctionExpression/]],
  '.ref/applications/synapse/alexa/static/js/main.05f102d2.js':[
    ['alexa-process-launch-job-match-and-exit-consumption',573269,/ArrowFunctionExpression/]],
  '.ref/applications/synapse/dashboard/static/js/2973.acc7b128.chunk.js':[
    ['shared-thx-wrapper-dual-channels',724293,/ClassDeclaration/],
    ['shared-stream-mixer-wrapper-and-descriptor',1282500,/ClassDeclaration/],
    ['shared-nanoleaf-legacy-wrapper',1304640,/ClassDeclaration/],
    ['shared-loupedeck-helper-lifecycle',1077321,/ClassDeclaration/]],
};
function contextName(n,p,text){
  if(n.id?.name)return n.id.name;
  if(p?.type==='VariableDeclarator')return text.slice(p.id.start,p.id.end);
  if(p?.type==='AssignmentExpression')return text.slice(p.left.start,p.left.end);
  if(['Property','PropertyDefinition','MethodDefinition'].includes(p?.type))return String(key(p.key));
  return n.type+'@'+n.start;
}
const direct=/^(?:FFILibrary|FFIObject|doDLL(?:Main)?Action(?:Async)?|callDLLApi(?:Async)?|_sendDLLAction|getProc|simpleLaunchUserAppProcess(?:NoWait|Json)?|getProcessesByName|killProcessByName|killProcess|launchProcess|launchUserAppProcess)$/;
for(const input of graph.sources){
  const raw=read(input.path),text=raw.toString('utf8'),fingerprint=hash(raw);
  if(fingerprint!==input.sha256)throw Error('Stale graph '+input.path);
  for(const origin of origins.get(input.path)||[])if(origin.sha256!==fingerprint)throw Error('Stale catalog '+input.path);
  const tree=acorn.parse(text,{ecmaVersion:'latest',sourceType:'module'}),parents=new WeakMap(),nodes=[];
  function collect(n,p){if(!n?.type)return;parents.set(n,p);nodes.push(n);for(const v of Object.values(n)){
    if(Array.isArray(v))for(const c of v)if(c?.type)collect(c,n);else{}else if(v?.type)collect(v,n);
  }}collect(tree,null);
  function moduleFor(n){return input.modules.filter(m=>m.offset<=n.start&&m.end>=n.end).sort((a,b)=>(a.end-a.offset)-(b.end-b.offset))[0]||null;}
  function nearest(n,rx){for(let p=parents.get(n);p;p=parents.get(p))if(rx.test(p.type))return p;return null;}
  function receipt(n,role){
    const id=input.path+':'+n.start+':'+n.end;
    if(!receipts.has(id)){const m=moduleFor(n);receipts.set(id,{id,path:input.path,sha256:fingerprint,offset:n.start,end:n.end,node_type:n.type,module_id:m?.module_id??null,source:text.slice(n.start,n.end),roles:[]});}
    if(!receipts.get(id).roles.includes(role))receipts.get(id).roles.push(role);return id;
  }
  for(const [id,point,type]of semanticPlan[input.path]||[]){
    const candidates=nodes.filter(n=>type.test(n.type)&&n.start<=point&&n.end>point).sort((a,b)=>(a.end-a.start)-(b.end-b.start));
    if(!candidates.length)throw Error('Unresolved semantic anchor '+id);
    semanticAnchors.push({id,receipt:receipt(candidates[0],'manually_reviewed_semantic_anchor')});
  }
  const callee=n=>n.callee?.type==='SequenceExpression'?n.callee.expressions.at(-1):n.callee;
  const calls=nodes.filter(n=>['CallExpression','NewExpression'].includes(n.type));
  const receivers=new Set(calls.filter(n=>key(callee(n)?.property)==='getProc').map(n=>text.slice(callee(n).object.start,callee(n).object.end)));
  const receiverMembers=new Set(calls.filter(n=>key(callee(n)?.property)==='getProc').map(n=>{
    const object=callee(n).object;return moduleFor(n)?.module_id+':'+key(object.property);
  }));
  const isNativeReceiver=(object,n)=>object&&(receivers.has(text.slice(object.start,object.end))||
    object.type==='MemberExpression'&&receiverMembers.has(moduleFor(n)?.module_id+':'+key(object.property)));
  const boundaries=[];const wrapperNodes=new Set();
  for(const n of calls){const c=callee(n),method=key(c?.property)||c?.name;let kind;
    if(typeof method==='string'&&direct.test(method))kind=method==='getProc'?'legacy_proc_declaration':method==='FFILibrary'?'legacy_library_constructor':/DLL/.test(method)?'electron_native_bridge_or_wrapper':'external_process_api';
    const receiver=c?.type==='MemberExpression'?text.slice(c.object.start,c.object.end):null;
    if(['call','upgrade','setAllocator','setEventInterface','free','close','destroy'].includes(method)&&isNativeReceiver(c.object,n))kind='legacy_instance_operation';
    if(n.type==='NewExpression'&&parents.get(n)?.type==='AssignmentExpression'&&isNativeReceiver(parents.get(n).left,n)&&!kind)kind='native_owned_receiver_constructor_alias';
    if(!kind)continue;
    const fn=nearest(n,/Function/),cls=nearest(n,/Class(?:Expression|Declaration)/),m=moduleFor(n);
    boundaries.push({kind,method,receiver,args:n.arguments.map(a=>text.slice(a.start,a.end)),receipt:receipt(n,kind),
      function:fn?{name:contextName(fn,parents.get(fn),text),receipt:receipt(fn,'boundary_enclosing_function')}:null,
      class:cls?{name:contextName(cls,parents.get(cls),text),receipt:receipt(cls,'boundary_enclosing_class')}:null,module_id:m?.module_id??null});
    if(cls)wrapperNodes.add(cls);else if(fn)wrapperNodes.add(fn);
    // Babel/transpiled constructors have nested methods but no Class node.
    // Keep the complete smallest outer function containing native init and
    // constructor/method definitions, not only the one inner FFI call.
    if(!cls&&fn){let outer=nearest(fn,/Function/);if(outer&&outer!==fn&&
      !input.modules.some(m=>m.offset===outer.start)&&text.slice(outer.start,outer.end).length<120000&&
      /FFILibrary|getProc/.test(text.slice(outer.start,outer.end)))wrapperNodes.add(outer);}
  }
  const wrappers=[...wrapperNodes].map(n=>({name:contextName(n,parents.get(n),text),receipt:receipt(n,'wrapper_body'),module_id:moduleFor(n)?.module_id??null,
    class_base:n.superClass?text.slice(n.superClass.start,n.superClass.end):null,
    // Exact syntax only. These labels do not assert reachability or read safety.
    json_decoders:nodes.filter(x=>n.start<=x.start&&n.end>=x.end&&x.type==='CallExpression'&&x.callee.type==='MemberExpression'&&x.callee.object.name==='JSON'&&key(x.callee.property)==='parse').map(x=>receipt(x,'json_decode')),
    catch_blocks:nodes.filter(x=>n.start<=x.start&&n.end>=x.end&&x.type==='CatchClause').map(x=>receipt(x,'error_handling')),
    callback_assignments:nodes.filter(x=>n.start<=x.start&&n.end>=x.end&&x.type==='AssignmentExpression'&&/^(onffievent|onffiexception)$/.test(key(x.left.property))).map(x=>receipt(x,'legacy_callback_assignment')),
    unload_listeners:nodes.filter(x=>n.start<=x.start&&n.end>=x.end&&x.type==='CallExpression'&&['addEventListener','removeEventListener'].includes(key(callee(x)?.property))&&/unload/.test(x.arguments[0]?.value||'')).map(x=>receipt(x,'unload_listener')),
  }));
  const nativeModules=new Set(boundaries.map(b=>b.module_id).filter(n=>n!==null));
  const incoming=[];
  for(const m of input.modules)for(const d of [...m.dependencies,...m.lazy])if(nativeModules.has(d.module_id))incoming.push({from_module:m.module_id,to_module:d.module_id,offset:d.offset});
  const names=new Set();for(const n of wrapperNodes){if(n.id?.name)names.add(n.id.name);const p=parents.get(n);if(p?.type==='VariableDeclarator'&&p.id.name)names.add(p.id.name);}
  const initialization_candidates=calls.filter(n=>n.type==='NewExpression'&&names.has(n.callee.name)||n.type==='CallExpression'&&key(callee(n)?.property)==='init'&&/rzUpdater|natalie|surround|thx|chroma|nanoleaf/i.test(text.slice(n.start,n.end))).map(n=>({receipt:receipt(n,'initialization_candidate'),module_id:moduleFor(n)?.module_id??null,scope:'Syntax candidate; module inclusion alone does not prove mounted application reachability'}));
  const excluded_actor_spawn=calls.filter(n=>key(callee(n)?.property)==='spawn').map(n=>({offset:n.start,end:n.end,source:text.slice(n.start,n.end),reason:'No child_process attribution; XState/Promise actor spawn is not OS process spawn'}));
  const os_process_imports=nodes.filter(n=>n.type==='ImportDeclaration'&&/^(node:)?child_process$/.test(n.source.value)||n.type==='CallExpression'&&n.callee.name==='require'&&/^(node:)?child_process$/.test(n.arguments[0]?.value||'')).map(n=>receipt(n,'child_process_import'));
  const ffi_api_objects=nodes.filter(n=>n.type==='ObjectExpression'&&n.properties.length&&n.properties.every(p=>p.type==='Property'&&p.value.type==='ArrayExpression'&&p.value.elements[0]?.type==='Literal'&&/^(?:void|bool|int|uint|uint8|uint16|uint32|int8|int16|int32|float|double|pointer|string|long|ulong|int64|uint64)$/.test(p.value.elements[0].value)&&p.value.elements[1]?.type==='ArrayExpression')).map(n=>({receipt:receipt(n,'modern_ffi_api_declaration'),functions:n.properties.map(p=>({name:key(p.key),returns:p.value.elements[0].value,args:p.value.elements[1].elements.map(a=>a.value??text.slice(a.start,a.end))})),module_id:moduleFor(n)?.module_id??null}));
  const binaryLiterals=new Map();for(const n of nodes)if(n.type==='Literal'&&typeof n.value==='string'&&/\.(?:dll|dylib|exe|node)$/i.test(n.value)){
    if(!binaryLiterals.has(n.value))binaryLiterals.set(n.value,{literal:n.value,occurrences:[],exact_basename_pe_matches:[]});binaryLiterals.get(n.value).occurrences.push({offset:n.start,end:n.end});
  }
  for(const item of binaryLiterals.values())item.exact_basename_pe_matches=peRows.filter(r=>path.win32.basename(item.literal).toLowerCase()===r.file.toLowerCase()).map(r=>({path:r.path,sha256:r.sha256,evidence:r.evidence,attribution:'Exact filename only, not a proof of runtime load or wrapper ABI identity'}));
  outputs.push({path:input.path,sha256:fingerprint,bytes:raw.length,parse_status:'parsed',routes:(origins.get(input.path)||[]).map(o=>o.route),boundary_count:boundaries.length,boundaries,wrappers,ffi_api_objects,binary_literal_candidates:[...binaryLiterals.values()],incoming_same_file_module_edges:incoming,initialization_candidates,excluded_actor_spawn,os_process_imports});
}
const applications=index.applications.map(a=>{const files=outputs.filter(f=>a.js_files.includes(f.path)),mods=graph.sources.filter(f=>a.js_files.includes(f.path)).flatMap(f=>f.modules.map(m=>({...m,path:f.path}))),nativeModules=new Set(files.flatMap(f=>f.boundaries.map(b=>b.module_id)).filter(n=>n!==null));
  const incoming=mods.flatMap(m=>[...m.dependencies,...m.lazy].filter(d=>nativeModules.has(d.module_id)).map(d=>({from_path:m.path,from_module:m.module_id,to_module:d.module_id,offset:d.offset,target_candidates:mods.filter(t=>t.module_id===d.module_id).map(t=>({path:t.path,offset:t.offset,end:t.end})),scope:'Exact module reference, not a constructor invocation or mounted route proof'})));
  return {route:a.route,html:a.html,manifest:a.manifest,js_files:files.map(f=>({path:f.path,sha256:f.sha256,boundary_count:f.boundary_count})),boundary_count:files.reduce((n,f)=>n+f.boundary_count,0),incoming_native_module_references:incoming,zero_boundary_scope:'Only explicit currently catalogued native/process APIs scanned; window services, USB/BLE, dynamic strings and external HTML scripts are separate.'};});
const result={schema_version:1,scope:'Full 755 current application JS files: caller-side legacy FFI, modern DLL bridges/wrappers and external-process APIs; no name-based read/write classification.',
  method:'All sources statically parsed by Acorn, matched to current catalog and full application index hashes; no vendor JS, DLL, application, build or tests executed.',
  offset_unit:'UTF-16 code units, end exclusive; file SHA-256 hashes actual source bytes',generator_sha256:hash(read('tools/audit-application-native-current.cjs')),catalog_sha256:hash(read(catalogPath)),
  summary:{endpoints:applications.length,html_applications:applications.filter(a=>a.html).length,parsed_js:outputs.length,files_with_boundaries:outputs.filter(f=>f.boundary_count).length,
    boundary_calls:outputs.reduce((n,f)=>n+f.boundary_count,0),receipts:receipts.size,legacy_constructors:outputs.flatMap(f=>f.boundaries).filter(b=>b.kind==='legacy_library_constructor').length,
    native_owned_constructor_aliases:outputs.flatMap(f=>f.boundaries).filter(b=>b.kind==='native_owned_receiver_constructor_alias').length,
    modern_ffi_api_objects:outputs.reduce((n,f)=>n+f.ffi_api_objects.length,0),semantic_anchors:semanticAnchors.length,
    os_child_process_imports:outputs.reduce((n,f)=>n+f.os_process_imports.length,0),actor_spawn_exclusions:outputs.reduce((n,f)=>n+f.excluded_actor_spawn.length,0),complete_application_semantics_claimed:0},
  applications,sources:outputs,receipts:[...receipts.values()],semantic_anchors:semanticAnchors,
  limitations:['Shared wrapper inclusion and same-file module edges do not prove route reachability.','Legacy ABI signature strings are source declarations, not current host ffi-napi ABI equivalence.','Return fallbacks and catches may mask actual native failure; source defaults do not prove successful hardware reads.','No binary execution or C/C++ original engineering source reconstruction claimed.','Non-member/dynamic external process actions and externally loaded HTML framework scripts require separate provenance.']};
const target='docs/re/application-native-current-evidence.json';const serialized=JSON.stringify(result,null,2)+'\n';
if(process.argv.includes('--check')){if(read(target).toString('utf8')!==serialized)throw Error('Stale '+target);}else fs.writeFileSync(path.join(root,target),serialized);
console.log(JSON.stringify(result.summary));
