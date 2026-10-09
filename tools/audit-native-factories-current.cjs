// Static caller / constructor / init / installed-resource tracing only.
// Never execute vendor JavaScript, native addons, DLLs, or application code.
const fs=require('fs'),path=require('path'),zlib=require('zlib'),acorn=require('acorn'),assert=require('assert');
const {walk,key,hash}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..'),read=p=>fs.readFileSync(path.join(root,p));
const inventoryPath='docs/re/native-library-full-current-inventory.json',moduleIndexPath='docs/re/middleware-code-current-evidence.json.gz';
const inventory=JSON.parse(read(inventoryPath)),index=JSON.parse(zlib.gunzipSync(read(moduleIndexPath)));
const focusArg=process.argv.find(x=>x.startsWith('--inspect-product=')),focus=focusArg?Number(focusArg.split('=')[1]):null;
const targets=inventory.unresolved_attribution.filter(x=>x.reason.includes('caller-supplied')&&(focus===null||x.source_product_id===focus));
const productIds=new Set(targets.map(x=>x.source_product_id)),files=index.files.filter(x=>productIds.has(x.product_id));
const texts=new Map(),products=new Map(),analysisCache=new Map(),parseFailures=[];
const text=f=>{if(!texts.has(f.path)){const bytes=read(f.path),r=JSON.parse(read(f.path+'.http.json'));
  assert.equal(hash(bytes),f.sha256);assert.equal(r.sha256,f.sha256);assert.equal(r.bytes,bytes.length);assert.equal(r.http_status,200);
  assert.equal(r.source_url,`https://apps.razer.com/synapse/products/${f.product_id}/mw/${path.basename(f.path)}`);assert.equal(r.final_url,r.source_url);
  texts.set(f.path,bytes.toString('utf8'));}return texts.get(f.path);};
for(const f of files){if(!products.has(f.product_id))products.set(f.product_id,{files:[],modules:new Map(),targets:[]});
  const p=products.get(f.product_id);p.files.push(f);
  for(const m of index.analyses[f.sha256].modules){if(!p.modules.has(m.id))p.modules.set(m.id,[]);p.modules.get(m.id).push({file:f,...m});}}
for(const [product,p] of products){const entry=index.manifests.find(x=>x.product_id===product),body=read(entry.path),r=JSON.parse(read(entry.path+'.http.json'));
  assert.equal(hash(body),entry.sha256);assert.equal(r.sha256,entry.sha256);assert.equal(r.bytes,body.length);assert.equal(r.http_status,200);
  const manifest=JSON.parse(body),declared=[...new Set(Object.values(manifest))].filter(x=>typeof x==='string'&&x.endsWith('.js')).sort();
  assert.deepEqual(p.files.map(f=>path.basename(f.path)).sort(),declared,'Manifest file list changed for '+product);}
for(const t of targets){const p=products.get(t.source_product_id),f=p.files.find(f=>f.path===t.path);assert(f);
  const module=[...p.modules.values()].flat().filter(m=>m.file.path===t.path&&m.start<=t.offset&&m.end>=t.end).sort((a,b)=>(a.end-a.start)-(b.end-b.start))[0];
  p.targets.push({...t,module_id:module?.id??null,module_start:module?.start??0,module_end:module?.end??text(f).length});}
function receipt(m,n){return {path:m.file.path,sha256:m.file.sha256,offset:n.start,end:n.end,source:text(m.file).slice(n.start,n.end)};}
function parse(m){const cacheKey=m.file.sha256+':'+m.start+':'+m.end;if(analysisCache.has(cacheKey))return analysisCache.get(cacheKey);
  let fn;try{const src=text(m.file).slice(m.start,m.end),concise=/^\([^)]*\)\s*\{/.test(src),prefix=concise?'(function':'(';
    fn=acorn.parse(prefix+src+')',{ecmaVersion:'latest'}).body[0].expression;
    walk(fn,n=>{n.start+=m.start-prefix.length;n.end+=m.start-prefix.length;});fn.start=m.start;}
  catch(e){parseFailures.push({path:m.file.path,module_id:m.id,message:e.message});return null;}
  const parents=new Map(),defs=new Map(),exports=new Map(),nodes=[];
  function visit(n,parent){if(!n?.type)return;parents.set(n,parent);nodes.push(n);
    for(const v of Object.values(n)){if(Array.isArray(v))v.forEach(x=>visit(x,n));else if(v?.type)visit(v,n);}}
  visit(fn,null);
  for(const statement of fn.body.body||[]){if(statement.type==='VariableDeclaration')for(const d of statement.declarations)if(d.id.type==='Identifier')defs.set(d.id.name,d.init);
    if(['ClassDeclaration','FunctionDeclaration'].includes(statement.type))defs.set(statement.id.name,statement);
    walk(statement,n=>{if(/Function|Class/.test(n.type))return false;
      if(n.type==='AssignmentExpression'&&n.operator==='='&&n.left.type==='MemberExpression'&&n.left.object.name===fn.params[1]?.name)exports.set(key(n.left.property),n.right);
      if(n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&n.callee.object.name===fn.params[2]?.name&&key(n.callee.property)==='d'&&n.arguments[0]?.name===fn.params[1]?.name){
        const getter=x=>x?.body?.type==='BlockStatement'?x.body.body.find(x=>x.type==='ReturnStatement')?.argument:x?.body;
        if(n.arguments[1]?.type==='ObjectExpression')for(const p of n.arguments[1].properties){const b=getter(p.value);if(b)exports.set(key(p.key),b);}
        if(n.arguments[1]?.type==='ArrayExpression'){const e=n.arguments[1].elements;for(let i=0;i<e.length;){const name=e[i++],kind=e[i++],value=kind?.value===0?e[i++]:getter(kind);assert.equal(typeof name?.value,'string');assert(value,'Unresolved export descriptor');exports.set(name.value,value);}}
      }
    });}
  const result={fn,parents,defs,exports,nodes};analysisCache.set(cacheKey,result);if(analysisCache.size>64)analysisCache.delete(analysisCache.keys().next().value);return result;
}
function candidates(product,id){return products.get(product).modules.get(id)||[];}
function model(product,id){const a=candidates(product,id);if(!a.length)return null;
  // Differing declarations of the same module ID require context; do not choose one.
  const byBody=new Map();for(const m of a){const b=text(m.file).slice(m.start,m.end);if(!byBody.has(b))byBody.set(b,m);}
  if(byBody.size!==1)return null;const m=[...byBody.values()][0],data=parse(m);return data?{...m,...data}:null;}
function ancestor(m,n,predicate){for(let p=n;p;p=m.parents.get(p))if(predicate(p))return p;return null;}
function binding(m,n,name){for(let p=n;p;p=m.parents.get(p)){
  if(['BlockStatement','Program','SwitchCase'].includes(p.type)){for(const s of p.body||p.consequent||[])if(s.type==='VariableDeclaration')for(const d of s.declarations){
    if(d.id.type==='Identifier'&&d.id.name===name)return {node:d.init,declaration:d};
    if(d.id.type==='ArrayPattern'){const i=d.id.elements.findIndex(x=>x?.name===name);if(i>=0)return {node:d.init,declaration:d,index:i};}
    if(d.id.type==='ObjectPattern'){const prop=d.id.properties.find(x=>x.value?.name===name);if(prop)return {node:d.init,declaration:d,property:key(prop.key)};}}}
  if(/Function/.test(p.type)&&p.params.some(x=>x.type==='Identifier'&&x.name===name))return {parameter:true,node:p};
 }return m.defs.has(name)?{node:m.defs.get(name)}:null;}
function unseq(n){return n?.type==='SequenceExpression'?n.expressions[n.expressions.length-1]:n?.type==='ChainExpression'?n.expression:n;}
function imported(m,n){n=unseq(n);if(n?.type!=='MemberExpression'||n.object.type!=='Identifier')return null;
  const b=binding(m,n,n.object.name),d=b&&!b.parameter?b.node:null;
  if(d?.type==='CallExpression'&&d.callee.name===m.fn.params[2]?.name&&binding(m,d,d.callee.name)?.node===m.fn&&Number.isInteger(d.arguments[0]?.value))return {id:d.arguments[0].value,export:key(n.property),declaration:d};return null;}
function exportedNode(product,m,n){const im=imported(m,n);if(!im)return null;const other=model(product,im.id);let d=other?.exports.get(im.export),seen=new Set();
  while(d?.type==='Identifier'&&!seen.has(d)){seen.add(d);d=other.defs.get(d.name);}return d?{model:other,node:d,import:im}:null;}
function loaderDependencies(product,m,d){const deps=[];walk(d,n=>{if(n.type!=='CallExpression')return;let v=exportedNode(product,m,n.callee);
  const cal=unseq(n.callee);if(!v&&cal?.type==='Identifier'){const b=binding(m,n,cal.name);if(b?.node&&!b.parameter&&/FunctionExpression/.test(b.node.type))v={model:m,node:b.node,import:{id:m.id,export:null},local_binding:cal.name};}if(!v)return;
  const source=receipt(v.model,v.node).source;if(source.includes('getItem')&&source.length<15000)deps.push({module_id:v.import.id,export:v.import.export,local_binding:v.local_binding,call:receipt(m,n),body:receipt(v.model,v.node),cache_key_constants:cacheKeys(product,v.model,v.node)});});return deps;}
function cacheKeys(product,m,n){const constants=[];walk(n,x=>{if(x.type==='CallExpression'&&x.callee.type==='MemberExpression'&&key(x.callee.property)==='getItem'&&x.arguments[0]?.value==='installedResources')constants.push({reference:receipt(m,x.arguments[0]),value:'installedResources',definition:receipt(m,x.arguments[0]),status:'literal_key_resolved'});
  if(x.type!=='MemberExpression'||key(x.property)!=='INSTALLED_RESOURCE_KEY')return;
  const v=exportedNode(product,m,x.object),prop=v?.node?.type==='ObjectExpression'?v.node.properties.find(p=>key(p.key)==='INSTALLED_RESOURCE_KEY'):null;
  constants.push({reference:receipt(m,x),value:prop?.value?.value??null,definition:prop?receipt(v.model,prop):null,status:prop?.value?.type==='Literal'?'literal_key_resolved':'key_declaration_not_resolved'});});return constants;}
function classTarget(product,m,n,at,seen=new Set()){n=unseq(n);if(!n||seen.has(n))return null;seen.add(n);
  if(['ClassDeclaration','ClassExpression'].includes(n.type)){return products.get(product).targets.find(t=>t.path===m.file.path&&t.offset===n.start)||null;}
  if(n.type==='Identifier'){const b=binding(m,at||n,n.name);if(b?.node&&!b.parameter){
    if(b.property){let call=b.node;while(['YieldExpression','AwaitExpression'].includes(call?.type))call=call.argument;
      const cal=unseq(call?.callee);if(call?.type==='CallExpression'&&cal?.type==='MemberExpression'&&cal.object.type==='Identifier'){
        const map=binding(m,call,cal.object.name)?.node,iteration=cal.computed&&cal.property.type==='Identifier'?ancestor(m,call,x=>x.type==='ForInStatement'&&x.right.type==='Identifier'&&x.right.name===cal.object.name&&x.left?.declarations?.some(d=>d.id.name===cal.property.name)):null;
        const props=map?.properties?.filter(x=>iteration||key(x.key)===key(cal.property))||[],foundTargets=[];
        for(const prop of props){const loads=[];walk(prop.value,x=>{if(x.type==='CallExpression'&&x.callee.type==='MemberExpression'&&key(x.callee.property)==='bind'&&x.callee.object.name===m.fn.params[2]?.name&&Number.isInteger(x.arguments[1]?.value))loads.push(x.arguments[1].value);});
          const ids=[...new Set(loads)];if(ids.length===1&&products.get(product).targets.some(t=>t.module_id===ids[0])){const other=model(product,ids[0]),v=other?.exports.get(b.property);if(v){const found=classTarget(product,other,v,v,new Set(seen));if(found)foundTargets.push({...found,lazy_class_resolution:{binding:b.declaration?receipt(m,b.declaration):null,factory_property:receipt(m,prop),computed_iteration:iteration?receipt(m,iteration):null,required_selector_key:iteration?key(prop.key):null,target_module_id:ids[0],export:b.property,export_getter:receipt(other,v)}});}}}
        if(foundTargets.length===1)return foundTargets[0];}
    }
    return classTarget(product,m,b.node,b.node,seen);}}
  const im=imported(m,n);if(im){const other=model(product,im.id),v=other?.exports.get(im.export);if(v)return classTarget(product,other,v,v,seen);}
  return null;
}
function instance(product,m,n,at,seen=new Set()){n=unseq(n);if(!n||seen.has(n))return null;seen.add(n);
  if(n.type==='NewExpression'){const target=classTarget(product,m,n.callee,n);return target?{target,construction:receipt(m,n),class_resolution:target.lazy_class_resolution||null}:null;}
  if(n.type==='AssignmentExpression')return instance(product,m,n.right,n,seen);
  if(n.type==='Identifier'){const b=binding(m,at||n,n.name);if(b&&!b.parameter)return instance(product,m,b.node,b.node,seen);
    if(b?.parameter){const call=m.parents.get(b.node),ix=b.node.params.findIndex(p=>p.type==='Identifier'&&p.name===n.name);
      if(call?.type==='CallExpression'&&unseq(call.callee)===b.node&&call.arguments[ix]){const resolved=instance(product,m,call.arguments[ix],call,new Set(seen));if(resolved)return {...resolved,immediate_parameter_forwarding:{parameter:n.name,function:receipt(m,b.node),invocation:receipt(m,call),argument:receipt(m,call.arguments[ix])}};}}}
  if(n.type==='MemberExpression'&&n.object.type==='ThisExpression'){
    const owner=ancestor(m,at||n,p=>/Class/.test(p.type));if(!owner)return null;
    const matches=m.nodes.filter(x=>x.type==='AssignmentExpression'&&x.left.type==='MemberExpression'&&x.left.object.type==='ThisExpression'&&key(x.left.property)===key(n.property)&&ancestor(m,x,p=>/Class/.test(p.type))===owner);
    const solved=matches.map(x=>instance(product,m,x.right,x,new Set(seen))).filter(Boolean);const keys=new Set(solved.map(x=>x.target.path+':'+x.target.offset));
    if(keys.size===1){const s=solved[0];return {...s,field_assignment:receipt(m,matches.find(x=>instance(product,m,x.right,x,new Set(seen))))};}}
  return null;
}
function argumentTrace(product,m,n,seen=new Set()){if(!n)return {kind:'omitted'};if(seen.has(n))return {kind:'cycle'};seen.add(n);
  if(n.type==='YieldExpression'||n.type==='AwaitExpression')return argumentTrace(product,m,n.argument,seen);
  if(n.type==='Literal')return {kind:'literal',value:n.value,receipt:receipt(m,n)};
  if(n.type==='Identifier'){const b=binding(m,n,n.name);if(b?.parameter){const ix=b.node.params.findIndex(p=>p.type==='Identifier'&&p.name===n.name);
    const callers=m.nodes.filter(c=>c.type==='CallExpression'&&unseq(c.callee)?.type==='Identifier'&&binding(m,c,unseq(c.callee).name)?.node===b.node);
    return {kind:'parameter',name:n.name,receipt:receipt(m,n),function:receipt(m,b.node),local_caller_candidates:callers.map(c=>({call:receipt(m,c),argument:c.arguments[ix]?receipt(m,c.arguments[ix]):null,for_of_context:ancestor(m,c,p=>p.type==='ForOfStatement')?receipt(m,ancestor(m,c,p=>p.type==='ForOfStatement')):null,enclosing_function:receipt(m,ancestor(m,c,p=>/Function/.test(p.type))),caveat:'Actual call-expression argument and guard context; runtime feature-list value remains unknown'}))};}
    if(b?.node)return {kind:'local_binding',name:n.name,array_index:b.index,object_property:b.property,declaration:b.declaration?receipt(m,b.declaration):null,value:argumentTrace(product,m,b.node,seen)};
    const assignments=m.nodes.filter(x=>x.type==='AssignmentExpression'&&x.left.type==='Identifier'&&x.left.name===n.name&&(!b?.declaration||binding(m,x,x.left.name)?.declaration===b.declaration));
    return {kind:'module_or_unresolved_variable',name:n.name,assignments:assignments.map(x=>({assignment:receipt(m,x),enclosing_function:receipt(m,ancestor(m,x,p=>/Function/.test(p.type))),cache_key_constants:cacheKeys(product,m,ancestor(m,x,p=>/Function/.test(p.type)))})),caveat:'Cross-function ordering and reaching definitions not proven'};}
  if(n.type==='MemberExpression'&&n.computed&&Number.isInteger(n.property.value))return {kind:'array_index',index:n.property.value,array:argumentTrace(product,m,n.object,seen)};
  if(n.type==='ArrayExpression')return {kind:'array',items:n.elements.map(x=>argumentTrace(product,m,x,new Set(seen)))};
  if(n.type==='CallExpression'){const im=imported(m,n.callee);let resolved=null;if(im){const other=model(product,im.id);let d=other?.exports.get(im.export);if(d?.type==='Identifier')d=other.defs.get(d.name);
      if(d)resolved={module_id:im.id,export:im.export,import:receipt(m,im.declaration),body:receipt(other,d),installed_resource_dependencies:loaderDependencies(product,other,d),resource_selector:/usedBy\.includes/.test(receipt(other,d).source)&&/filePath/.test(receipt(other,d).source),caveat:'Static function body; installed-resource contents and returned path are runtime data'};}
    return {kind:'call',receipt:receipt(m,n),resolved,args:n.arguments.map(x=>argumentTrace(product,m,x,new Set(seen)))};}
  return {kind:'expression',receipt:receipt(m,n),caveat:'Expression evaluation not performed'};
}
function storageIdentity(m,n){n=unseq(n);if(n?.type!=='MemberExpression')return null;
  const at=n;
  const parts=[];for(;n?.type==='MemberExpression';n=n.object){if(n.computed&&n.property.type!=='Literal')return null;parts.unshift(key(n.property));}
  if(n?.type!=='Identifier'||parts.length<2)return null;const b=binding(m,at,n.name),d=b&&!b.parameter?b.node:null;
  if(d?.type==='CallExpression'&&d.callee.name===m.fn.params[2]?.name&&binding(m,d,d.callee.name)?.node===m.fn&&Number.isInteger(d.arguments[0]?.value))return {key:d.arguments[0].value+':'+parts.join('.'),module_id:d.arguments[0].value,export:parts[0],properties:parts.slice(1),import:receipt(m,d)};return null;
}
function classIdentity(product,m,n,seen=new Set()){if(!n||seen.has(n))return null;seen.add(n);if(/Class/.test(n.type))return {path:m.file.path,sha256:m.file.sha256,offset:n.start,end:n.end};
  if(n.type==='Identifier'){const b=binding(m,n,n.name);if(b&&!b.parameter)return classIdentity(product,m,b.node,seen);}
  const v=exportedNode(product,m,n);return v?classIdentity(product,v.model,v.node,seen):null;}
function instanceGuards(product,m,n,storage){const guards=[];
  function collect(x){if(!x)return;if(x.type==='SequenceExpression'){collect(x.expressions.at(-1));return;}if(x.type==='LogicalExpression'&&x.operator==='&&'){collect(x.left);collect(x.right);return;}
    if(x.type!=='BinaryExpression'||x.operator!=='instanceof'||storageIdentity(m,x.left)?.key!==storage.key)return;
    guards.push({test:receipt(m,x),identity:classIdentity(product,m,x.right)});}
  for(let p=n;p;p=m.parents.get(p)){const tests=p.type==='IfStatement'&&n.start>=p.consequent.start&&n.end<=p.consequent.end?[p.test]:p.type==='LogicalExpression'&&p.operator==='&&'&&n.start>=p.right.start?[p.left]:[];
    tests.forEach(collect);}
  return guards;}
function returnedInstances(product,m,n){if(n?.type!=='YieldExpression')return [];const call=unseq(n.argument);if(call?.type!=='CallExpression')return [];
  const generator=call.arguments.find(x=>x?.type==='FunctionExpression'&&x.generator);if(!generator)return [];
  const helper=call.callee.type==='Identifier'?binding(m,call,call.callee.name)?.node:null;
  if(!helper||!/FunctionExpression/.test(helper.type))return [];
  const helperSource=receipt(m,helper).source;if(!helperSource.includes('.apply(')||!helperSource.includes('.next('))return [];
  const found=[];walk(generator,n=>{if(n.type!=='NewExpression'||ancestor(m,n,x=>/Function/.test(x.type))!==generator)return;
    const ret=ancestor(m,n,x=>x.type==='ReturnStatement');if(!ret||!((ret.argument===n)||(ret.argument?.type==='ConditionalExpression'&&[ret.argument.consequent,ret.argument.alternate].includes(n))))return;
    const i=instance(product,m,n,n);if(i)found.push({...i,conditional_return:{yielded_helper_call:receipt(m,call),helper_definition:receipt(m,helper),generator_return:receipt(m,ret),caveat:'A guarded constructor return path; active feature/branch and successful asynchronous completion are not asserted'}});});return found;}
const targetEvidence=[],callerLinks=[],referenceCandidates=[],resourceSelectors=[],scanned=[],storageFactoryChains=[],configuredFeatureCalls=[];
let productsDone=0;
for(const [product,p] of products){for(const f of p.files)text(f);
  const targetModuleIds=new Set(p.targets.filter(t=>t.module_id!==null).map(t=>t.module_id)),models=[];
  for(const t of p.targets){let m=t.module_id!==null?model(product,t.module_id):null;
    if(!m){const f=p.files.find(f=>f.path===t.path);const raw={file:f,start:t.offset,end:t.end,id:null},d=parse(raw);m=d?{...raw,...d}:null;}
    if(!m){targetEvidence.push({...t,reason:'Class/module parse or duplicate module context unresolved'});continue;}
    const c=m.nodes.find(n=>/Class/.test(n.type)&&n.start===t.offset),methods=[];
    if(c)walk(c,n=>{if(n.type==='MethodDefinition'&&/^init|initialize/.test(key(n.key)||''))methods.push({name:key(n.key),receipt:receipt(m,n)});
      if(n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&n.left.object.type==='ThisExpression'&&/^init|initialize/.test(key(n.left.property)||'')&&/FunctionExpression/.test(n.right.type))methods.push({name:key(n.left.property),receipt:receipt(m,n)});});
    targetEvidence.push({product_id:product,path:t.path,sha256:t.sha256,class_offset:t.offset,class_end:t.end,class_name:c?.id?.name,module_id:t.module_id,
      library_default_candidates:t.library_candidates,declared_function_count:t.declared_functions.length,init_methods:methods,
      exports:[...m.exports].filter(([,v])=>classTarget(product,m,v,v)?.offset===t.offset).map(([name,v])=>({name,receipt:receipt(m,v)})),
      status:'caller_parameter_unknown_until_instance_context',caveat:'Shared bundle presence is not evidence of active product path'});}
  for(const [id,locations] of p.modules){const first=locations[0],src=text(first.file).slice(first.start,first.end);
    const direct=[...targetModuleIds].some(t=>new RegExp('\\b'+t+'\\b').test(src));
    const resource=/usedBy\.includes/.test(src)&&/filePath/.test(src);
    if(!direct&&!targetModuleIds.has(id)&&!resource&&!/getFeatureParam\(["']hasDll/.test(src))continue;
    const m=model(product,id);if(!m){referenceCandidates.push({product_id:product,module_id:id,status:'ambiguous_or_unparsed_module_context'});continue;}
    models.push(m);
    scanned.push({product_id:product,module_id:id,path:m.file.path,sha256:m.file.sha256,offset:m.start,end:m.end});
    for(const n of m.nodes){if(n.type==='CallExpression'){
      const cal=unseq(n.callee);if(cal?.type==='Identifier'&&cal.name===m.fn.params[2]?.name&&targetModuleIds.has(n.arguments[0]?.value))referenceCandidates.push({product_id:product,caller_module_id:id,target_module_id:n.arguments[0].value,kind:'direct_require',receipt:receipt(m,n),status:'import_reference_not_instance_or_init_proof'});
      if(cal?.type==='MemberExpression'&&key(cal.property)==='bind'&&cal.object.name===m.fn.params[2]?.name&&targetModuleIds.has(n.arguments[1]?.value))referenceCandidates.push({product_id:product,caller_module_id:id,target_module_id:n.arguments[1].value,kind:'lazy_require_bind',receipt:receipt(m,n),status:'lazy_reference_activation_and_constructor_not_resolved'});
      if(cal?.type==='MemberExpression'&&['init','initialize','initElectron'].includes(key(cal.property))){const i=instance(product,m,cal.object,n);if(i)callerLinks.push({product_id:product,caller_module_id:id,target_path:i.target.path,target_class_offset:i.target.offset,target_module_id:i.target.module_id,method:key(cal.property),instance:i,call:receipt(m,n),argument:n.arguments.length?argumentTrace(product,m,n.arguments[0]):{kind:'omitted'},status:'constructor_receiver_and_init_argument_static_chain',caveat:'Conditional control flow and runtime resource value remain separate; does not prove invocation or library load success'});}
    }
    if(n.type==='VariableDeclarator'&&n.init){const body=receipt(m,n.init).source;if(body.includes('usedBy.includes')&&body.includes('filePath')&&body.length<15000)resourceSelectors.push({product_id:product,module_id:id,binding:n.id.name,receipt:receipt(m,n),status:'installed_resource_selector_body',caveat:'Resource selection is over installed cache; official manifest ownership does not substitute for cache returned value'});}
   }
  }
  const writes=[];
  for(const m of models)for(const n of m.nodes){
    if(n.type==='AssignmentExpression'&&n.operator==='='){
      const storage=storageIdentity(m,n.left);if(!storage)continue;
      const direct=instance(product,m,n.right,n);if(direct){writes.push({target:direct.target,storage,write:receipt(m,n),assignment:receipt(m,n),construction:direct.construction,class_resolution:direct.class_resolution,scope:receipt(m,ancestor(m,n,x=>/Function/.test(x.type))),caller_module_id:m.id});continue;}
      if(n.right.type!=='Identifier')continue;
      const b=binding(m,n,n.right.name);if(!b?.declaration)continue;
      for(const i of returnedInstances(product,m,b.node))writes.push({target:i.target,storage,write:receipt(m,n),assignment:receipt(m,b.declaration),construction:i.construction,class_resolution:i.class_resolution,conditional_return:i.conditional_return,scope:receipt(m,ancestor(m,b.declaration,x=>/Function/.test(x.type))),caller_module_id:m.id});
      const assignments=m.nodes.filter(x=>x.type==='AssignmentExpression'&&x.left.type==='Identifier'&&x.left.name===n.right.name&&binding(m,x,x.left.name)?.declaration===b.declaration);
      for(const a of assignments){const i=instance(product,m,a.right,a);if(i){const branch=ancestor(m,a,x=>x.type==='SwitchCase'),sw=ancestor(m,a,x=>x.type==='SwitchStatement'),disc=sw?.discriminant;
        const map=disc?.type==='MemberExpression'&&disc.object.type==='Identifier'?m.defs.get(disc.object.name):null;
        writes.push({target:i.target,storage,write:receipt(m,n),assignment:receipt(m,a),construction:i.construction,class_resolution:i.class_resolution,scope:receipt(m,ancestor(m,a,x=>/Function/.test(x.type))),switch_case:branch?receipt(m,branch):null,selector:disc?receipt(m,disc):null,feature_to_class_map:map?.type==='ObjectExpression'?receipt(m,map):null,caller_module_id:m.id});}}
    }
  }
  for(const m of models)for(const n of m.nodes){if(n.type!=='CallExpression')continue;const cal=unseq(n.callee);
    if(cal?.type!=='MemberExpression'||!['init','initialize','initElectron'].includes(key(cal.property)))continue;
    let aliases=[];const storageReceiver=storageIdentity(m,cal.object);
    if(storageReceiver)aliases=[{node:cal.object,storage:storageReceiver}];
    else if(cal.object.type==='Identifier'){
      const b=binding(m,n,cal.object.name);if(b?.parameter)continue;
      const a=m.nodes.filter(a=>a.type==='AssignmentExpression'&&a.left.type==='Identifier'&&a.left.name===cal.object.name&&(!b?.declaration||binding(m,a,a.left.name)?.declaration===b.declaration));if(b?.declaration&&b.node)a.push(b.declaration);
      aliases=a.map(node=>({node,storage:storageIdentity(m,node.right||node.init)}));}
    for(const {node:alias,storage} of aliases){if(!storage)continue;
      for(const w of writes.filter(w=>w.storage.key===storage.key)){
        const gates=instanceGuards(product,m,n,storage);if(gates.some(g=>!g.identity||g.identity.sha256!==w.target.sha256||g.identity.offset!==w.target.offset))continue;
        storageFactoryChains.push({product_id:product,target_path:w.target.path,target_class_offset:w.target.offset,target_module_id:w.target.module_id,factory_module_id:w.caller_module_id,
        construction:w.construction,class_resolution:w.class_resolution,conditional_return:w.conditional_return,constructor_assignment:w.assignment,runtime_storage_write:w.write,storage:w.storage,conditional_factory_scope:w.scope,factory_switch_case:w.switch_case,factory_selector:w.selector,feature_to_class_map:w.feature_to_class_map,
        init_module_id:m.id,receiver_alias:receipt(m,alias),call:receipt(m,n),receiver_instanceof_guards:gates,argument:n.arguments.length?argumentTrace(product,m,n.arguments[0]):{kind:'omitted'},
        status:'conditional_lazy_constructor_storage_and_init_chain',caveat:'Factory branch requires active feature selector; all alternatives remain separate. Source presence does not prove branch active or a runtime DLL path.'});}}
  }
  for(const f of p.files.filter(f=>path.basename(f.path).startsWith('main.'))){const s=text(f);
    // This initial feature call inventory is textual location evidence only;
    // interpretation is retained in parsed product/factory source, never executed.
    for(const match of s.matchAll(/\.useFeature\(\s*["'](?:rzDeviceType|thxVersion|hasDll)["']/g))configuredFeatureCalls.push({product_id:product,path:f.path,sha256:f.sha256,offset:match.index,end:Math.min(s.length,match.index+500),source:s.slice(match.index,Math.min(s.length,match.index+500)),status:'configuration_call_location_requires_full_AST_argument_and_condition_interpretation'});
  }
  // Bound text memory; a 64-module AST LRU shares identical bytes across products.
  texts.clear();if(++productsDone%20===0)console.log(`Static native factory products ${productsDone}/${products.size}`);
}
const linksByTarget=new Map();for(const l of callerLinks){const k=l.target_path+':'+l.target_class_offset;if(!linksByTarget.has(k))linksByTarget.set(k,[]);linksByTarget.get(k).push(l);}
for(const t of targetEvidence){const links=linksByTarget.get(t.path+':'+t.class_offset)||[];t.direct_constructor_init_contexts=links.length;
  t.conditional_storage_factory_contexts=storageFactoryChains.filter(x=>x.target_path===t.path&&x.target_class_offset===t.class_offset).length;
  t.status=links.length?'direct_instance_caller_context_found_runtime_path_not_observed':t.conditional_storage_factory_contexts?'conditional_factory_selector_not_yet_resolved':'init_input_caller_context_unresolved';
  t.remaining_unknowns=links.length?['Whether product entrypoint activates this shared manager path','Installed cache contents and selected resource filePath; no runtime DLL identity observed']:
    t.conditional_storage_factory_contexts?['Factory feature selector/configuration branch activation','Actual selected installed-resource record and filePath','Cross-function scheduling/reaching definition of the init argument']:
    [t.library_default_candidates[0]==='RzASRock'?'Generic computed device factory and returned-object forwarding not resolved':t.library_default_candidates[0]==='philipshuev2'?'Hue manager field/runtime rzDevice aliases and generic hasDll feature dispatcher not resolved':t.library_default_candidates[0]==='PhilipsHueNative'?'Legacy monolithic scope lacks resolved webpack caller context':'Alternate/older THX feature factory and runtime storage forwarding not resolved','Dynamic init argument remains unknown; fallback DLL literal and matching PE exports do not resolve it'];}
// Do not embed full target instance tables recursively in caller evidence.
for(const l of callerLinks){l.instance.target={path:l.target_path,class_offset:l.target_class_offset,module_id:l.target_module_id};}
const loaderCalls=callerLinks.filter(c=>c.argument.value?.resolved?.resource_selector);
const result={schema_version:1,date:'2026-10-09',method:'Manifest-receipt verified current middleware module ranges, Acorn module/class ASTs, lexical declarations and verified constructors; no vendor execution',
  inputs:[{path:inventoryPath,sha256:hash(read(inventoryPath))},{path:moduleIndexPath,sha256:hash(read(moduleIndexPath))}],
  summary:{source_products:products.size,manifest_source_files:files.length,caller_input_scopes:targets.length,scanned_module_contexts:scanned.length,direct_constructor_init_chains:callerLinks.length,scopes_with_direct_chain:targetEvidence.filter(t=>t.direct_constructor_init_contexts).length,direct_calls_with_resolved_resource_selector:loaderCalls.length,direct_calls_with_cache_getter_and_literal_key:loaderCalls.filter(c=>c.argument.value.resolved.installed_resource_dependencies.some(d=>d.cache_key_constants.some(k=>k.value==='installedResources'))).length,conditional_storage_factory_chains:storageFactoryChains.length,scopes_with_conditional_storage_chain:targetEvidence.filter(t=>t.conditional_storage_factory_contexts).length,unresolved_scope_contexts:targetEvidence.filter(t=>!t.direct_constructor_init_contexts&&!t.conditional_storage_factory_contexts).length,resource_selector_bodies:resourceSelectors.length,reference_candidates:referenceCandidates.length,parse_failures:parseFailures.length},
  targets:targetEvidence,direct_caller_chains:callerLinks,conditional_storage_factory_chains:storageFactoryChains,configured_feature_call_locations:configuredFeatureCalls,module_reference_candidates:referenceCandidates,resource_selector_bodies:resourceSelectors,scanned_modules:scanned,parse_failures:parseFailures,
  boundaries:['Direct constructor/receiver/argument dataflow is static evidence, not runtime execution, loaded DLL identity or successful device read.',
   'Imported class resolution is module-local and export-getter based; differing duplicate module bodies stay ambiguous.',
   'Local lexical variable/array destructuring and direct IIFE parameter forwarding are resolved only for captured forms; arbitrary dynamic properties, other forwarding, lazy feature branch activation and cross-function reaching definitions remain unresolved.',
   'Resource matching uses real installed-resource cache, name, usedBy and product state; manifest ownership and matching PE export names cannot supply runtime values.',
   'Only the 875 explicitly quarantined caller-DLL scopes are traced here; other unknown signature scopes and independent application FFI protocols remain separate.',
   'No existing runtime asset or declaration attribution is changed by this audit.']};
const output='docs/re/native-factory-current-evidence.json.gz',bytes=zlib.gzipSync(Buffer.from(JSON.stringify(result)),{level:9});
if(focus!==null){console.log(JSON.stringify(result));}
else {
  const families={};for(const t of targetEvidence){const name=t.library_default_candidates[0];if(!families[name])families[name]={scopes:0,direct:0,conditional:0,unresolved:0};const f=families[name];f.scopes++;if(t.direct_constructor_init_contexts)f.direct++;else if(t.conditional_storage_factory_contexts)f.conditional++;else f.unresolved++;}
  const summary={schema_version:1,date:result.date,evidence_path:output,evidence_sha256:hash(bytes),summary:result.summary,families,boundaries:result.boundaries};
  const summaryPath='docs/re/native-factory-current-summary.json',summaryBytes=Buffer.from(JSON.stringify(summary,null,2)+'\n');
  if(process.argv.includes('--check')){assert.equal(hash(Buffer.from(JSON.stringify(JSON.parse(zlib.gunzipSync(read(output)))))),hash(Buffer.from(JSON.stringify(result))),'Stale native factory evidence');assert.equal(hash(read(summaryPath)),hash(summaryBytes),'Stale native factory summary');}
  else {fs.writeFileSync(path.join(root,output),bytes);fs.writeFileSync(path.join(root,summaryPath),summaryBytes);}
}
console.log(JSON.stringify(result.summary));
