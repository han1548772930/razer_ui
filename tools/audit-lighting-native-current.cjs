// Parse current source as Acorn data; never require/import vendor JavaScript.
const fs=require('fs'),path=require('path'),assert=require('assert'),acorn=require('acorn');
const {walk,key,hash}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..');
const file='.ref/applications/synapse/lighting-engine/static/js/main.3588205d.js';
const source=fs.readFileSync(path.join(root,file),'utf8');
const manifest=JSON.parse(fs.readFileSync(path.join(root,'.ref/applications/synapse/lighting-engine/asset-manifest.json'),'utf8'));
assert.equal(manifest.files['main.js'],'./static/js/main.3588205d.js');
assert.equal(hash(Buffer.from(source)),JSON.parse(fs.readFileSync(path.join(root,file+'.http.json'),'utf8')).sha256);
const ast=acorn.parse(source,{ecmaVersion:'latest'}),receipts=[];
function receipt(node,role){return {role,file,sha256:hash(Buffer.from(source)),offset:node.start,end:node.end,source:source.slice(node.start,node.end)};}
const assignments=new Set(['initLightingDriver','lightingDriverApi','createLightingDeviceEx','destroyLightingDeviceAsync','addMultiLinkSeqDevice','processEvent','init']);
walk(ast,node=>{
  if(node.type==='AssignmentExpression'&&node.left.type==='MemberExpression'&&assignments.has(key(node.left.property))){
    const body=source.slice(node.start,node.end);
    if(/lightingDriver|LightingEngineApi|RzLightingApi|LightingDriverDLL/.test(body))receipts.push(receipt(node,'assigned-method:'+key(node.left.property)));
  }
  if(node.type==='CallExpression'&&node.callee.type==='MemberExpression'&&key(node.callee.property)==='resolve'
     &&node.arguments[0]?.type==='CallExpression'){
    const body=source.slice(node.start,node.end);
    if(body.includes('LightingEngineModule.initRzLightingEngine()'))receipts.push(receipt(node,'resource-loader'));
  }
  if(node.type==='VariableDeclarator'&&node.init?.type==='ArrowFunctionExpression'){
    const body=source.slice(node.start,node.end);
    if(body.includes('doLightingDriverAction'))receipts.push(receipt(node,'host-ipc-helper'));
  }
  if(node.type==='MethodDefinition'&&assignments.has(key(node.key))){
    const body=source.slice(node.start,node.end);
    if(/lightingDriver|LightingEngineApi|RzLightingApi/.test(body))receipts.push(receipt(node,'assigned-method:'+key(node.key)));
  }
});
const loader=receipts.find(r=>r.role==='resource-loader');assert(loader);
assert(loader.source.includes('"LightingDriverDLL"===e.name'));
assert(loader.source.includes('"LightingEngineDLL"===e.name'));
assert(loader.source.includes('n=t.filePath'));
const driverInit=receipts.find(r=>r.role==='assigned-method:initLightingDriver'&&r.source.includes('action:"InitDLL"'));assert(driverInit);
assert(driverInit.source.includes('Apps\\\\Synapse\\\\lighting_driver.dll'));
assert(receipts.some(r=>r.role==='host-ipc-helper'));
const acquisition=JSON.parse(fs.readFileSync(path.join(root,'docs/re/lighting-native-current-acquisition.json'),'utf8'));
const bindings=acquisition.resources.map(r=>({library_id:r.resource_name==='LightingDriverDLL'?'lighting_driver':'RzLightingEngineApi',
  resource_name:r.resource_name,file:r.file,sha256:r.sha256,bytes:r.bytes,md5:r.md5,machine:r.machine,exports:r.exports,
  input:r.input,path_rules:['apps_synapse'],bindings:[{product_id:null,resource_name:r.resource_name,resource_version:r.version,
  install_relative_path:null,install_path_template:'userDataDir/Apps/{installedResources[synapse ?? Common].find(name == '+r.resource_name+').filePath}',
  fallback_relative_path:'Synapse/'+(r.resource_name==='LightingDriverDLL'?'lighting_driver.dll':'RzLightingEngineApi.dll'),
  source:'docs/re/application-resource-metadata-2026-10-09/background-resources.json',
  loader_receipt_role:'resource-loader',loader_source:file}],
  declared_functions:r.resource_name==='LightingEngineDLL'?[
    {name:'GetDLLVersion',returns:'pointer',args:[]},{name:'FreeMalloc',returns:'void',args:['pointer']},
    {name:'RzLightingApi',returns:'pointer',args:['string']},{name:'SetNodeFFIEvent',returns:'bool',args:['pointer']},
    {name:'PollEvents',returns:'pointer',args:[]},{name:'UsePollingMode',returns:'void',args:['bool']},
    {name:'RzLightingApiNoReturn',returns:'void',args:['string']},{name:'GetFeatureSet',returns:'pointer',args:[]},
    {name:'SetOperatingMode',returns:'void',args:['int']}]:[]}));
// Exact current source receipts, not names alone, establish the loader linkage.
for(const binding of bindings)for(const declaration of binding.declared_functions){
  const ffi=receipts.find(r=>r.role==='assigned-method:init'&&r.source.includes(declaration.name+':['));assert(ffi,declaration.name);
  declaration.receipt={path:ffi.file,sha256:ffi.sha256,offset:ffi.offset,end:ffi.end,source:ffi.source};
}
const result={schema_version:1,method:'Current manifest-verified AST loader and native PE metadata, no vendor execution',
  offset_unit:'UTF-16 JavaScript code units',receipts,resources:bindings,
  limitations:['Resources and selected native functions are acquired; complete lighting engine rendering, platform adapters and UI/runtime closure remain separate work.']};
const output=path.join(root,'docs/re/lighting-native-current-source-evidence.json');const encoded=JSON.stringify(result,null,2)+'\n';
if(process.argv.includes('--check'))assert.equal(fs.readFileSync(output,'utf8'),encoded);else fs.writeFileSync(output,encoded);
console.log(JSON.stringify({source_receipts:receipts.length,attributed_resources:bindings.length}));
