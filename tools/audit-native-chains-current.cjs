// Static current-source native boundary map. Never load a vendor module or DLL.
const fs = require('fs'), path = require('path'), assert = require('assert');
const acorn = require('acorn');
const {walk, key, hash} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const inputs = [];
function read(file) {
  assert(!/\.ref\/(?:frontend|synapse-asar|host-4\.0\.821)\//.test(file));
  const bytes = fs.readFileSync(path.join(root, file));
  if (!inputs.some(r => r.path === file)) inputs.push({path:file,sha256:hash(bytes),bytes:bytes.length});
  return bytes;
}
const json = file => JSON.parse(read(file));
const runtimeInventory = json('assets/data/native-library-inventory.json');
const auditInventoryPath=fs.existsSync(path.join(root,'docs/re/native-library-full-current-inventory.json'))
  ?'docs/re/native-library-full-current-inventory.json':'assets/data/native-library-inventory.json';
const inventory = auditInventoryPath==='assets/data/native-library-inventory.json'?runtimeInventory:json(auditInventoryPath);
const native = json('docs/re/native-library-pe-current-evidence.json');
const hostNative = json('docs/re/host-native-library-pe-current-evidence.json');
const catalog = json('docs/re/product-catalog.json');
const bindings = json('docs/re/middleware-device-bindings-current.json');
const extraction = json('.ref/host-4.0.827/native-evidence/EXTRACTION-RECEIPT.json');
const hid = json('docs/re/receiver-native-hid-current-evidence.json');
const serviceCode = json('docs/re/host-service-machine-code-current-evidence.json');
const fullAsar = json('docs/re/host-full-asar-current-evidence.json');
assert.equal(native.failures.length,0);
for (const r of inventory.sources) assert.equal(hash(read(r.path)),r.sha256);
function receipt(r) {
  const text = read(r.path).toString('utf8');
  assert.equal(hash(Buffer.from(text)),r.sha256);
  if(r.source !== undefined) assert.equal(text.slice(r.offset,r.end),r.source);
  return {path:r.path,sha256:r.sha256,offset:r.offset,end:r.end};
}
// Classification is a navigation hint, not a claim that a DLL has no side effects.
function classify(name) {
  if (/Callback|FFIEvent|Notification|Register.*Event|Unregister.*Event|UnRegister.*Event/i.test(name)) return 'event_or_subscription_name';
  if (/Initialize|Terminate|Startup|Shutdown|^(?:Init|UnInit|DeviceInit|DeviceTerminate|FreeMalloc|FreeString)$/.test(name)) return 'lifecycle_or_ownership_name';
  if (/^(?:get|is|has|enumerate|query)|_(?:Get|Is|Has)/i.test(name)) return 'query_name';
  if (/^(?:set|add|remove|enable|disable|start|stop|register|unregister|launch|open|close|select|reset|update|configure)|_(?:Set|Select|Send|Update|Open|Init|UnInit)/i.test(name)) return 'state_change_or_operation_name';
  return 'requires_body_and_caller_review';
}
const peRows = [...native.resources,...hostNative.files];
const binaries = peRows.map(r => {
  assert.equal(hash(read(r.path)),r.sha256);
  return {path:r.path,file:r.file,sha256:r.sha256,bytes:r.bytes,machine:r.machine,
    origin:r.url||r.archive_entry,bindings:r.bindings||[],exports:r.exports,
    imports:r.imports||null,internal_implementation:serviceCode.files.some(f=>f.sha256===r.sha256)?'selected_export_and_service_chain_disassembled':'not_semantically_reversed',
    caveat:'Export presence/RVA is not a C ABI proof, function body reconstruction, or hardware result'};
});
for (const r of extraction.files) assert.equal(hash(read(r.path)),r.sha256);
assert.equal(hash(read(hid.path)),hid.sha256);
const libraries = inventory.libraries.map(l => ({id:l.id,kind:l.kind,channel:l.channel,
  declared_product_owners:l.products,file_names:l.file_names,path_rules:l.path_rules,
  binary_variants:l.resources.map(r=>({file:r.file,sha256:r.sha256,bytes:r.bytes,machine:r.machine,
    declaration_missing_exports:l.declared_functions.filter(d=>!r.exports.includes(d.name)).map(d=>d.name),
    bindings:r.bindings,internal_implementation:serviceCode.files.some(f=>f.sha256===r.sha256)?'selected_export_and_service_chain_disassembled':'not_semantically_reversed'})),
  functions:l.declared_functions.map(d=>({name:d.name,returns:d.returns,args:d.args,
    navigation_class:classify(d.name),semantics_confirmed_by_name:false,
    declaration:receipt(d.receipt),call_sites:(d.calls||[]).map(c=>({method:c.method,argument:c.argument,
      decoder:c.decoder,receipt:receipt(c.receipt)}))})),
  declaration_conflicts:l.declaration_conflicts,
  source_session_chains:l.sessions,
  lifecycle_exports:l.declared_functions.filter(d=>classify(d.name)==='lifecycle_or_ownership_name').map(d=>d.name),
  callback_exports:l.declared_functions.filter(d=>classify(d.name)==='event_or_subscription_name').map(d=>d.name),
  internal_implementation:serviceCode.files.some(f=>l.resources.some(r=>r.sha256===f.sha256))?'selected_export_and_service_chain_disassembled':'not_semantically_reversed',
  caller_boundary:l.receipts.map(r=>({path:r.path,sha256:r.sha256,offset:r.offset,end:r.end})),
}));
const groups = new Map();
for (const r of inventory.unresolved_attribution) {
  const k=JSON.stringify(r.declared_functions||r.reason);
  if(!groups.has(k)) groups.set(k,{reason:r.reason,declared_functions:r.declared_functions||[],
    example:r,occurrences:[],product_ids:new Set()});
  const g=groups.get(k);g.occurrences.push({path:r.path,sha256:r.sha256,offset:r.offset,end:r.end});
  if(r.source_product_id!==undefined)g.product_ids.add(r.source_product_id);
}
const unresolved_groups=[...groups.values()].map((g,i)=>({id:`unresolved-native-${i+1}`,reason:g.reason,
  declared_functions:g.declared_functions,occurrence_count:g.occurrences.length,
  source_bundle_product_ids:[...g.product_ids].sort((a,b)=>a-b),example:g.example,
  occurrences:g.occurrences,ownership:'unproven; shared chunk inclusion does not establish product/library ownership'}));
const productBindings=new Map(bindings.products.map(p=>[p.product_id,p]));
const directoryIds=fs.readdirSync(path.join(root,'.ref/middleware')).filter(p=>/^\d+$/.test(p)).map(Number);
const productIds=[...new Set([...catalog.products.map(p=>p.product_id),...directoryIds])].sort((a,b)=>a-b);
const productNames=new Map(catalog.products.map(p=>[p.product_id,p.name]));
const products=productIds.map(product_id=>{
  const b=productBindings.get(product_id),manifest=`.ref/middleware/${product_id}/manifest.json`;
  return {product_id,name:productNames.get(product_id)||null,
    manifest_present:fs.existsSync(path.join(root,manifest)),
    manifest_identity:fs.existsSync(path.join(root,manifest))?{path:manifest,sha256:hash(read(manifest))}:null,
    library_owners:libraries.filter(l=>l.declared_product_owners.includes(product_id)).map(l=>l.id),
    device_info_status:b?.status||'not_in_device_bindings_audit',
    source_device_info_candidates:b?.device_info_candidates||[],
    source_feature_calls:b?.feature_calls||[],
    unresolved_shared_bundle_groups:unresolved_groups.filter(g=>g.source_bundle_product_ids.includes(product_id)).map(g=>g.id),
    caveat:'No declared DLL resource does not mean no native dependency: HID/USB/BLE, host services, inherited/dynamic libraries may apply'};
});
const hostPaths=[
  'electron/modules/ffi/ffiMain.js','electron/modules/ffi/FFIPreloadMain.js',
  'electron/modules/ffi_subprocess/index.js','electron/modules/ffi_subprocess/FFIProcess.js',
  'electron/modules/ffi_subprocess/FFIPreloadSubProcess.js','electron/modules/ffi_subprocess/_ffiprocess.js',
  'electron/UsbRzDeviceAction.js','electron/modules/IoT/IoTNativeAction.js',
  'electron/modules/lighting/ffiLightingDriver.js','electron/serviceFunction.js',
  'electron/modules/security/win/index.js','electron/modules/mapping_engine/win/index.js',
  'electron/modules/simple_service/win/index.js','electron/modules/sysutil/win/index.js',
  'node_modules/ffi-napi-rz/lib/library.js','node_modules/ffi-napi-rz/lib/dynamic_library.js',
  'node_modules/ffi-napi-rz/lib/callback.js','node_modules/ffi-napi-rz/lib/foreign_function.js',
  'node_modules/ffi-napi-rz/lib/_foreign_function.js','node_modules/node-rz-hid/nodehid.js',
  'node_modules/ffi-napi-rz/lib/freelibrary.js','node_modules/ffi-napi-rz/lib/bindings.js',
  'node_modules/rz-usb-detect/index.js','node_modules/node-rz-ble-endpoint-detection/index.js',
  'node_modules/node-ble-rz/index.js','node_modules/@serialport/bindings-cpp/dist/index.js',
  'node_modules/ref-napi/lib/ref.js','node_modules/usb/dist/index.js',
];
const host_chains=hostPaths.map(p=>{
  const file='.ref/host-4.0.827/'+p,body=read(file),text=body.toString('utf8');
  const ast=acorn.parse(text,{ecmaVersion:'latest'}),methods=[],requires=[],actions=[],callbacks=[];
  walk(ast,n=>{
    let name;
    if(n.type==='MethodDefinition'||n.type==='PropertyDefinition')name=key(n.key);
    else if(n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&/FunctionExpression/.test(n.right.type))name=text.slice(n.left.start,n.left.end);
    if(name)methods.push({name,offset:n.start,end:n.end});
    if(n.type==='CallExpression'&&n.callee.name==='require'&&typeof n.arguments[0]?.value==='string')requires.push(n.arguments[0].value);
    if(n.type==='SwitchCase'&&typeof n.test?.value==='string')actions.push({action:n.test.value,offset:n.start,end:n.end});
    if(n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&key(n.callee.property)==='Callback')callbacks.push({offset:n.start,end:n.end,source:text.slice(n.start,n.end)});
  });return {path:file,sha256:hash(body),methods,requires:[...new Set(requires)],actions,callbacks};
});
const listing=read('.ref/host-4.0.827/source-evidence/inner-archive-list.txt').toString('utf8');
const archiveNative=[...listing.matchAll(/^Path = (win-unpacked\\[^\r\n]+\.(?:dll|node|exe))$/gim)].map(m=>m[1]);
const host_archive_native_entries=archiveNative.map(archive_entry=>{
  const normalized=archive_entry.replace(/\\/g,'/');
  const r=extraction.files.find(r=>r.location?.replace(/\\/g,'/')===normalized)||hostNative.files.find(r=>r.archive_entry===archive_entry)
    ||fullAsar.files.find(r=>r.archive_entry===normalized);
  return {archive_entry,static_extracted:!!r,path:r?.path||null,sha256:r?.sha256||null,
    implementation:'not_semantically_reversed',
    caveat:r?'Binary identity known; see per-function evidence separately':'Archive listing only; bytes and function bodies not extracted here'};
});
const result={schema_version:1,date:'2026-10-09',method:'Current manifest-owned native inventory, source SHA/UTF-16 AST ranges, byte-verified PE inventory and official host archive listing; no vendor execution',
  audit_inventory_path:auditInventoryPath,
  audit_inventory_scope:auditInventoryPath.startsWith('docs/')?'Full acquired current middleware ConfigureFFI/ffi-napi-rz scan plus five host wrappers; independent application window.FFILibrary/getProc/call channels are separately audited and excluded from this 48-library inventory; runtime asset unchanged':'Existing runtime inventory snapshot; full middleware acquisition/audit pending',
  runtime_inventory:{path:'assets/data/native-library-inventory.json',modified_by_this_audit:false,
    libraries:runtimeInventory.libraries.length,sources:runtimeInventory.sources.length,
    unambiguous_binding_signatures:runtimeInventory.libraries.reduce((s,l)=>s+l.declared_functions.length,0),
    caveat:'Runtime inclusion uses this independent older asset; new audit declarations are not automatically runtime integration'},
  classification_policy:'Function navigation classes are name heuristics, never a read-only permission gate or verified internal semantics',
  summary:{library_ids:libraries.length,binding_sources:inventory.sources.length,unambiguous_binding_signatures:libraries.reduce((s,l)=>s+l.functions.length,0),
    signature_conflicts:libraries.reduce((s,l)=>s+l.declaration_conflicts.length,0),
    unique_product_dll_binaries:native.resources.length,host_common_dll_files:hostNative.files.length,
    catalog_and_middleware_product_ids:products.length,products_with_manifest_owned_dll:products.filter(p=>p.library_owners.length).length,
    unresolved_attribution_occurrences:inventory.unresolved_attribution.length,unresolved_signature_groups:unresolved_groups.length,
    archive_native_entries:host_archive_native_entries.length,archive_native_entries_extracted:host_archive_native_entries.filter(r=>r.static_extracted).length,
    packaged_native_addons:serviceCode.packaged_native_addons.length,
    semantically_reversed_entire_dlls:0,known_hid_disassembled_functions:hid.functions.length,
    host_service_disassembled_exports:serviceCode.files.reduce((s,f)=>s+f.roots.length,0),
    host_service_code_ranges:serviceCode.files.reduce((s,f)=>s+f.code_ranges.length,0)},
  archive_native_entries_scope:'The 29 outer-package native entries in the dedicated native evidence list, not the complete ASAR entry count. static_extracted matches normalized paths against the full current host extraction receipt; 16 means 12 unpacked Windows addons and 4 CommonDLL bodies. Other outer-package graphics DLLs and helper EXEs remain archive-listed only in this audit.',
  inputs,libraries,binaries,products,host_chains,host_archive_native_entries,unresolved_groups,
  packaged_native_addons:serviceCode.packaged_native_addons,
  internal_machine_code_evidence:[{path:hid.path,sha256:hid.sha256,evidence:'docs/re/receiver-native-hid-current-evidence.json',
    functions:hid.functions.map(f=>({name:f.name,rva:f.rva,end_rva:f.end_rva,machine_code_sha256:f.machine_code_sha256})),
    caveat:'Seven selected HID function ranges only. Does not reconstruct all HID.node or any product DLL implementation'},
    ...serviceCode.files.map(f=>({path:f.path,sha256:f.sha256,evidence:'docs/re/host-service-machine-code-current-evidence.json',
      roots:f.roots,virtual_targets:f.virtual_targets,code_range_count:f.code_ranges.length,caveat:f.caveat}))]};
const output=path.join(root,'docs/re/native-chains-current-evidence.json'),encoded=JSON.stringify(result,null,2)+'\n';
if(process.argv.includes('--check'))assert.equal(fs.readFileSync(output,'utf8'),encoded,'Stale native chain evidence');
else fs.writeFileSync(output,encoded);
console.log(JSON.stringify(result.summary));
