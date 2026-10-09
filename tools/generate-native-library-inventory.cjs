// Current native binding declarations, inspected as Acorn data only.
// A declaration is inventory evidence, never authorization to call an export.
const fs = require('fs'), path = require('path'), acorn = require('acorn'), assert = require('assert');
const {walk, key, hash} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..'), sources = [], libraries = new Map(), unresolved = [];
const outputArg=process.argv.find(arg=>arg.startsWith('--output='));
const outputRelative=outputArg?outputArg.slice('--output='.length):'assets/data/native-library-inventory.json';
assert(/^docs\/re\/[a-z0-9-]+\.json$/.test(outputRelative)||outputRelative==='assets/data/native-library-inventory.json','Invalid inventory output path');
const auditOnly=outputRelative.startsWith('docs/');
const normalize = filename => path.win32.basename(filename).replace(/\.dll$/i, '').replace(/_v\d+(?:\.\d+)*$/i, '');
const get = id => {
  if (!libraries.has(id.toLowerCase())) libraries.set(id.toLowerCase(), {id, kind: 'device', channel: 'ConfigureFFI',
    file_names: new Set(), path_rules: new Set(), products: new Set(), declarations: new Map(), receipts: [], resources: [], sessions: []});
  return libraries.get(id.toLowerCase());
};
// Only manifest resources establish product ownership. A shared chunk's mere
// presence does not mean that its product uses every library in that chunk.
const pePath = path.join(root, 'docs/re/native-library-pe-current-evidence.json');
const pe = JSON.parse(fs.readFileSync(pePath, 'utf8'));
assert.equal(pe.failures.length, 0, 'Finish native resource acquisition first');
for (const resource of pe.resources) {
  const l = get(normalize(resource.file)); l.file_names.add(resource.file);
  for (const binding of resource.bindings) l.products.add(binding.product_id);
  l.resources.push({file: resource.file, sha256: resource.sha256, bytes: resource.bytes, md5: resource.md5,
    machine: resource.machine, exports: resource.exports.filter(e => e.name && !e.forwarder).map(e => e.name),
    bindings: resource.bindings});
  for (const b of resource.bindings) {
    const prefix = b.install_relative_path.split('/');
    if (prefix[0].toLowerCase() === 'common') l.path_rules.add(prefix.length > 2 ? 'apps_common_product' : 'apps_common');
    else if (prefix[0].toLowerCase() === 'synapse') l.path_rules.add(prefix.length > 2 ? 'apps_synapse_product' : 'apps_synapse');
  }
}
const hostPE = JSON.parse(fs.readFileSync(path.join(root,'docs/re/host-native-library-pe-current-evidence.json'),'utf8'));
for (const resource of hostPE.files) {
  const l=get(normalize(resource.file));
  l.resources.push({file:resource.file,sha256:resource.sha256,bytes:resource.bytes,md5:resource.md5,machine:resource.machine,
    exports:resource.exports.filter(e=>e.name&&!e.forwarder).map(e=>e.name),
    bindings:[{product_id:null,install_relative_path:'CommonDLL/'+resource.file,source:'docs/re/host-native-library-pe-current-evidence.json'}]});
  l.file_names.add(resource.file);l.path_rules.add('common_dll');
}
const files = [];
const astCache = new Map();
for (const item of fs.readdirSync(path.join(root, '.ref/middleware'), {withFileTypes: true})) {
  if (!item.isDirectory() || !/^\d+$/.test(item.name)) continue;
  const directory = `.ref/middleware/${item.name}`;
  const manifestPath = `${directory}/webpackManifest.json`;
  if (!fs.existsSync(path.join(root, manifestPath))) {
    unresolved.push({source_product_id:Number(item.name),path:manifestPath,reason:'Current middleware manifest was not acquired'});
    continue;
  }
  const manifestBody = fs.readFileSync(path.join(root, manifestPath));
  const receipt = JSON.parse(fs.readFileSync(path.join(root, manifestPath + '.http.json'), 'utf8'));
  assert.equal(hash(manifestBody), receipt.sha256);
  assert.equal(receipt.http_status, 200);
  assert.equal(receipt.source_url, `https://apps.razer.com/synapse/products/${item.name}/mw/webpackManifest.json`);
  assert.equal(receipt.final_url, receipt.source_url);
  const manifest = JSON.parse(manifestBody);
  for (const name of new Set(Object.values(manifest))) {
    if (!/^[\w.-]+\.js$/.test(name)) continue;
    const file = directory + '/' + name;
    if (!fs.existsSync(path.join(root, file))) continue;
    const body = fs.readFileSync(path.join(root, file)), r = JSON.parse(fs.readFileSync(path.join(root, file + '.http.json'), 'utf8'));
    assert.equal(hash(body), r.sha256); assert.equal(body.length, r.bytes); assert.equal(r.http_status, 200);
    assert.equal(r.source_url, `https://apps.razer.com/synapse/products/${item.name}/mw/${name}`);
    assert.equal(r.final_url, r.source_url);
    if (/ConfigureFFI|ffi-napi-rz/.test(body.toString('utf8'))) files.push({file, body, product: Number(item.name)});
  }
}
for (const [id, file] of [
  ['mapping_engine','mapping_engine/win/index.js'], ['simple_service','simple_service/win/index.js'],
  ['SysUtilsNative','sysutil/win/index.js'], ['lighting_driver','lighting/ffiLightingDriver.js'],
  ['IoTNative','IoT/IoTNativeAction.js'],
]) files.push({file: '.ref/host-4.0.827/electron/modules/' + file,
  body: fs.readFileSync(path.join(root, '.ref/host-4.0.827/electron/modules/' + file)), host: id});
const signature = node => {
  if (node?.type !== 'ArrayExpression' || node.elements.length !== 2) return null;
  const [returns, args] = node.elements;
  if (typeof returns?.value !== 'string' || args?.type !== 'ArrayExpression'
      || !args.elements.every(a => typeof a?.value === 'string')) return null;
  if (!/^(?:void|bool|pointer|char\*|string|u?int(?:8|16|32|64)?|long|ulong|ulonglong|longlong|float|double|size_t)$/.test(returns.value)) return null;
  return {returns: returns.value, args: args.elements.map(a => a.value)};
};
let scanned = 0;
for (const {file, body, product, host} of files) {
  const text = body.toString('utf8'), digest = hash(body);
  // Shared chunks are frequently byte-identical across products. Reusing a
  // bounded AST cache changes no receipts or ownership attribution.
  let ast = astCache.get(digest);
  if (ast) {astCache.delete(digest);astCache.set(digest,ast);}
  else {ast=acorn.parse(text,{ecmaVersion:'latest',sourceType:'script'});astCache.set(digest,ast);
    if(astCache.size>8)astCache.delete(astCache.keys().next().value);}
  sources.push({path: file, sha256: digest});
  const scopes = [];
  if (host) scopes.push(ast);
  else walk(ast, n => {if (['ClassDeclaration', 'ClassExpression'].includes(n.type)) {scopes.push(n); return false;}});
  for (const scope of scopes) {
    const names = new Set(), declarations = [], actions = [], calls=[];
    const scopeReceipt = n => ({path:file,sha256:digest,offset:n.start,end:n.end,source:text.slice(n.start,n.end)});
    const scanCalls = (node,method) => {
      if(!node?.type)return;
      if(node.type==='MethodDefinition')method={name:key(node.key),node};
      if(node.type==='AssignmentExpression'&&node.left.type==='MemberExpression'&&node.left.object.type==='ThisExpression'
        &&['ArrowFunctionExpression','FunctionExpression'].includes(node.right.type))method={name:key(node.left.property),node};
      if(node.type==='ObjectExpression'){
        const action=node.properties.find(p=>key(p.key)==='action')?.value;
        const payload=node.properties.find(p=>key(p.key)==='payload')?.value;
        const args=payload?.properties?.find(p=>key(p.key)==='actionArgs')?.value;
        if(typeof action?.value==='string')calls.push({name:action.value,
          argument:args?text.slice(args.start,args.end):'none',method:method?.name||'',
          decoder:method&&text.slice(method.node.start,method.node.end).includes('JSON.parse(')?'json':'raw',
          class_offset:scope.start,receipt:scopeReceipt(node)});
      }
      for(const value of Object.values(node)){
        if(Array.isArray(value))value.forEach(n=>scanCalls(n,method));
        else if(value?.type)scanCalls(value,method);
      }
    };
    scanCalls(scope,null);
    walk(scope, n => {
      if (n.type === 'Literal' && typeof n.value === 'string' && /[\w.-]+\.dll$/i.test(n.value)) {
        const name = path.win32.basename(n.value);
        if (name !== '.dll') names.add(name);
      }
      if (n.type === 'ObjectExpression' && n.properties.length
          && n.properties.every(p => p.type === 'Property' && signature(p.value))) {
        for (const property of n.properties) declarations.push({name: String(key(property.key)), ...signature(property.value),
          receipt: {path: file, sha256: digest, offset: property.start, end: property.end,
            source: text.slice(property.start, property.end)}});
      }
      if (n.type === 'ObjectExpression') {
        const action = n.properties.find(p => key(p.key) === 'action')?.value;
        if (typeof action?.value === 'string') actions.push(action.value);
      }
    });
    const ids = host ? [host] : [...new Set([...names].map(normalize))];
    if(!declarations.length){for(const id of ids){const l=get(id);for(const name of names)if(normalize(name)===id)l.file_names.add(name);
      if(l.receipts.length<4)l.receipts.push({path:file,sha256:digest,offset:scope.start,end:scope.end,source:text.slice(scope.start,Math.min(scope.start+200,scope.end))});}
      continue;}
    if (ids.length !== 1) {
      unresolved.push({path: file, sha256: digest, source_product_id: product,
        offset: scope.start, end: scope.end, library_candidates: ids,
        declared_functions: declarations.map(d => ({name: d.name, returns: d.returns, args: d.args})),
        reason: ids.length ? 'Multiple native libraries in one class; requires exact ConfigureFFI attribution'
          : 'Dynamic library name; requires resource/constructor tracing'});
      continue;
    }
    if (auditOnly && !host) {
      // A fallback DLL literal is not exact attribution when init accepts a
      // caller's DLL path. Matching PE names also cannot prove that the caller
      // supplies this fallback. Preserve the entire scope until traced.
      let inputDllAssignment=false;
      walk(scope,n=>{
        let fn;
        if(n.type==='MethodDefinition'&&/^init/i.test(key(n.key)||''))fn=n.value;
        if(n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&/^init/i.test(key(n.left.property)||'')&&/FunctionExpression/.test(n.right.type))fn=n.right;
        if(!fn?.params?.length)return;
        const params=new Set(fn.params.filter(p=>p.type==='Identifier').map(p=>p.name));
        walk(fn.body,child=>{
          if(child.type==='AssignmentExpression'&&child.left.type==='MemberExpression'&&child.left.object.type==='ThisExpression'
            &&['dllName','dllPath'].includes(key(child.left.property))&&child.right.type==='Identifier'&&params.has(child.right.name))inputDllAssignment=true;
        });
      });
      const candidate=get(ids[0]),missing=declarations.filter(d=>candidate.resources.length&&!candidate.resources.some(r=>r.exports.includes(d.name)));
      if(inputDllAssignment){
        unresolved.push({path:file,sha256:digest,source_product_id:product,offset:scope.start,end:scope.end,
          library_candidates:ids,declared_functions:declarations.map(d=>({name:d.name,returns:d.returns,args:d.args})),
          missing_from_default_library_resources:missing.map(d=>d.name),
          reason:'Default DLL literal with caller-supplied init path; actual factory/init argument tracing required even if default PE exports match'});
        continue;
      }
    }
    const l = get(ids[0]);
    for (const name of names) l.file_names.add(name);
    if (host) {
      l.kind = host === 'lighting_driver' ? 'lighting_driver' : host === 'IoTNative' ? 'iot' : 'engine';
      l.channel = host === 'lighting_driver' ? 'lightingDriver' : host;
      l.file_names.add(host + '.dll'); l.path_rules.add('common_dll');
    }
    // Receipts cover exact class declarations, with individual function ranges.
    if (l.receipts.length < 4) l.receipts.push({path: file, sha256: digest, offset: scope.start, end: scope.end,
      source: text.slice(scope.start, Math.min(scope.start + 200, scope.end))});
    const initializer=declarations.find(d=>d.name==='DeviceInitialize'&&d.returns==='bool'&&JSON.stringify(d.args)==='["string"]');
    const terminator=declarations.find(d=>d.name==='DeviceTerminate'&&d.returns==='bool'&&JSON.stringify(d.args)==='["string"]');
    const initializeCall=calls.find(c=>c.name==='DeviceInitialize'&&['this.containerId','this.deviceContainerId'].includes(c.argument));
    const terminateCall=calls.find(c=>c.name==='DeviceTerminate'&&c.argument===initializeCall?.argument);
    if(initializer&&terminator&&initializeCall&&terminateCall&&l.sessions.length<8)l.sessions.push({
      argument:initializeCall.argument,initialize:initializer.name,terminate:terminator.name,
      class_offset:scope.start,path:file,initialize_call:initializeCall,terminate_call:terminateCall,
      initialize_attempts:/<10;/.test(text.slice(scope.start,scope.end))?10:1,
      retry_delay_ms:/<10;/.test(text.slice(scope.start,scope.end))?100:0});
    for (const d of declarations) {
      const k = d.name + ':' + JSON.stringify([d.returns,d.args]);
      if (!l.declarations.has(k)) l.declarations.set(k, {...d, call_sites: [...new Set(actions)].filter(a => a === d.name),calls:calls.filter(c=>c.name===d.name)});
    }
  }
  if (++scanned % 40 === 0) console.log(`Static native binding scan ${scanned}/${files.length}`);
}
const outputLibraries = [...libraries.values()].map(l => {
  const byName = new Map();
  for (const d of l.declarations.values()) {const list = byName.get(d.name) || []; list.push(d); byName.set(d.name,list);}
  const conflicts = [...byName].filter(([,v]) => v.length > 1).map(([name, variants]) => ({name, variants}));
  return {id:l.id,kind:l.kind,channel:l.channel,file_names:[...l.file_names].sort(),path_rules:[...l.path_rules].sort(),
    products:[...l.products].sort((a,b)=>a-b), declared_functions:[...byName].filter(([,v])=>v.length===1)
      .map(([,v])=>v[0]).sort((a,b)=>a.name.localeCompare(b.name)), declaration_conflicts:conflicts,
    receipts:l.receipts, resources:l.resources,sessions:l.sessions};
}).sort((a,b)=>a.id.localeCompare(b.id));
const result = {schema_version:1, method:'Current manifest-verified Acorn class binding declarations + official manifest product ownership + static PE resources; no vendor execution',
  scope:{host_wrappers:5,middleware_products:fs.readdirSync(path.join(root,'.ref/middleware')).filter(n=>/^\d+$/.test(n)).length},
  sources, libraries:outputLibraries, unresolved_attribution:unresolved};
const output = path.join(root,outputRelative), encoded = JSON.stringify(result,null,2)+'\n';
if (process.argv.includes('--check')) assert.equal(hash(fs.readFileSync(output)),hash(Buffer.from(encoded)),'Stale native inventory at '+outputRelative);
else fs.writeFileSync(output,encoded);
console.log(JSON.stringify({libraries:outputLibraries.length,declared_functions:outputLibraries.reduce((n,l)=>n+l.declared_functions.length,0),
  unresolved_scopes:unresolved.length,conflicting_functions:outputLibraries.reduce((n,l)=>n+l.declaration_conflicts.length,0)}));
