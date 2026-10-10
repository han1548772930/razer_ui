// Parse current source as data. Never import/evaluate vendor JavaScript.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {CurrentMiddlewareSource}=require('./current-middleware-source.cjs');
const {walk,key,hash}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..'),read=p=>fs.readFileSync(path.join(root,p),'utf8');
const receipts=[],products=[];
function record(s,id,name,node){const r={product_id:s.product,module:id,name,...s.receipt(id,node)};receipts.push(r);return r;}
function exported(s,id,name){let n=s.exported(id,name);while(n.type==='Identifier')n=s.binding(id,n.name);return n;}
for(const product of [164,241]){
 const s=new CurrentMiddlewareSource(product),refresh=product===164?31468:6107;
 s.parse(s.chunkFile(6120));
 const profile=s.literal(21503,s.exported(21503,'DEFAULTPROFILE'));
 const deviceName=s.literal(21503,s.exported(21503,'DEVICE_NAME'));
 const defaults=[...s.module(21503).definitions.values()].find(n=>n?.type==='ObjectExpression'&&s.snippet(21503,n).startsWith('{version:'));
 if(!defaults)throw Error('Missing global defaults');
 record(s,21503,'local-global-defaults',defaults);
 for(const name of ['DeviceInfo','getDefaultProfile','getDeviceDefaultLocalData','EDITION_DEFAULT_PROFILE'])record(s,21503,name,exported(s,21503,name));
 // DeviceResetFeature is acquired but not enabled in these products. The
 // legacy event branch is the actual caller, not this neighboring class.
 record(s,63030,'unregistered-DeviceResetFeature',exported(s,63030,'DeviceResetFeature'));
 const branch=[];walk(s.module(refresh).fn,n=>{if(n.type==='SwitchCase'&&n.test?.value==='resetDevice')branch.push(n);});
 if(branch.length!==1)throw Error('Ambiguous actual reset branch');
 record(s,refresh,'actual-resetDevice-dispatch',branch[0]);
 const task=product===164?exported(s,25947,'Y7C'):s.binding(6107,'Ut');
 const taskText=s.snippet(product===164?25947:6107,task);
 if(!taskText.includes('taskMakerResetOBM')||!taskText.includes('getDefaultProfile')||!taskText.includes('profiles'))throw Error('Reset task changed');
 record(s,product===164?25947:6107,'actual-taskMakerResetOBM',task);
 if(product===241)record(s,6107,'first-OBM-mappings-guard',s.binding(6107,'Et'));
 for(const name of ['Tb','b4','vi','mZ'])record(s,60763,name,exported(s,60763,name));
 for(const [name,binding]of [
  ['cache-missing-serial-default-profile',product===164?'b':'M'],
  ['cache-has-serial-active-profile',product===164?'C':'O'],
  ['cache-get-serial-active-profile',product===164?'M':'C']
 ])record(s,60763,name,s.binding(60763,binding));
 record(s,89788,'cache-Loupdeck-bypass',exported(s,89788,'nw'));
 record(s,88443,'current-injected-feature-list',exported(s,88443,'N$'));
 for(const name of ['WJ','lY'])record(s,20236,name,exported(s,20236,name));
 record(s,60763,'serial-metadata',s.binding(60763,product===164?'T':'S'));
 record(s,refresh,'first-document-initialization',s.binding(refresh,product===164?'fn':'so'));
 record(s,58898,'current-schema-version',s.binding(58898,'En'));
 if(s.literal(58898,s.binding(58898,'En'))!==13)throw Error('Schema version changed');
 record(s,84816,'serial-query',exported(s,84816,'I_'));
 record(s,84816,'serial-parser',s.binding(84816,'u'));
 for(const name of ['kD','Qq'])record(s,30580,name,exported(s,30580,name));
 // Resolve actual primary Linker in a fresh manifest-scoped reader because
 // lazy chunks also package sibling classes under the same module IDs.
 for(const routeName of ['rzDevice25Linker.',...(product===241?['rzDevice25LinkerMultiDevices.']:[])]){
  const nativeSource=new CurrentMiddlewareSource(product),file=nativeSource.files.find(file=>file.includes(routeName));
  nativeSource.parse(file);
  for(const[id,module]of nativeSource.modules)walk(module.fn,node=>{
   if(node.type==='MethodDefinition'&&['_getTransactionId','sendCommand'].includes(key(node.key)))record(nativeSource,id,'primary-'+routeName+key(node.key),node);
   if(node.type==='ClassDeclaration'&&routeName.includes('MultiDevices')&&nativeSource.snippet(id,node).includes('this.name="rzDevice25LinkerMultiDevices"'))record(nativeSource,id,'primary-MultiDevices-inheritance',node);
  });
  for(const receipt of nativeSource.acquisition)if(!s.acquisition.some(r=>r.path===receipt.path))s.acquisition.push(receipt);
 }
 record(s,49243,'53-bit-version',exported(s,49243,'RzVersion53bits'));
 record(s,76665,'unique-profile-name',exported(s,76665,'W$'));
 record(s,97564,'profile-guid',exported(s,97564,'k$'));
 const fields=[];walk(exported(s,refresh,'Fk'),n=>{if(n.type==='SwitchCase'&&['brightness','quickEffects','mappings'].includes(n.test?.value)&&s.snippet(refresh,n).includes('case"'+n.test.value+'":{')&&!s.snippet(refresh,n).includes('singleProfileDevice'))fields.push(n);});
 for(const field of fields)record(s,refresh,'refresh-'+field.test.value+'-'+field.start,field);
 walk(s.module(41374).fn,n=>{if(n.type==='Property'&&key(n.key)==='ON_RESET_DEVICE'&&n.value.type==='ArrayExpression')record(s,41374,'state-machine-reset-actions',n);});
 const main=s.text(s.mainFile);
 if(main.includes('featureKey:"deviceReset"'))throw Error('DeviceResetFeature registration changed; re-audit reachable branch');
 if(main.includes('isSupportedByLoupdeck'))throw Error('Loupdeck cache bypass registered; re-audit serial cache preparation');
 const deviceInfo=s.snippet(21503,exported(s,21503,'DeviceInfo'));
 if(/isOBMDevice:|singleProfileDevice:/.test(deviceInfo))throw Error('Reset OBM/single-profile capability changed');
 const directory='.ref/devices/'+product,manifest=JSON.parse(read(directory+'/asset-manifest.json'));
 for(const relative of [...new Set(Object.values(manifest.files))].filter(x=>/^\.\/static\/js\/[^/]+\.js$/.test(x))){
  const file=directory+'/'+relative.slice(2),text=read(file);
  if(!text.includes('type:"ON_RESET_DEVICE"'))continue;
  walk(acorn.parse(text,{ecmaVersion:'latest'}),n=>{
   if(n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&n.left.object.type==='ThisExpression'&&['resetDevice','confirmDel'].includes(key(n.left.property))){
    const source=text.slice(n.start,n.end);
    if(source.includes('type:"ON_RESET_DEVICE"')||source.includes('this.resetDevice()'))receipts.push({product_id:product,name:'Help-'+key(n.left.property),path:file,sha256:hash(text),offset:n.start,end:n.end,source});
   }
  });
 }
 products.push({product_id:product,edition_id:0,device_name:deviceName,schema_version:13,default_profile:profile,default_global:s.literal(21503,defaults),
  actual_task:'taskMakerResetOBM',factory_reset_command_reachable:false,preserves_existing_profiles:true,
  refresh_events:['ON_INIT_BRIGHTNESS','ON_SET_EFFECTS','ON_SET_KEYMAPPING'],acquisition:s.acquisition});
}
const asset={schema_version:1,products:products.map(({acquisition,...p})=>p)};
const implementation_paths=[
 'crates/razer-device/src/receiver_identity.rs',
 'crates/razer-service/src/runtime/windows/receiver_identity.rs',
 'crates/razer-storage/src/receiver_reset.rs',
 'crates/razer-pages/src/features/source_help.rs',
 'crates/razer-shell/src/shell/receiver_reset.rs',
];
const implementation=implementation_paths.map(file=>({path:file,sha256:hash(fs.readFileSync(path.join(root,file)))}));
for(const [file,required]of [
 ['crates/razer-storage/src/receiver_reset.rs',['pub fn prepare_serial_cache','pub fn persist','pub fn record_refresh','device_refresh_complete']],
 ['crates/razer-shell/src/shell/receiver_reset.rs',['ReceiverIdentityRead','prepare_serial_cache','ResetPlan::prepare','ReceiverBrightnessWrite','record_refresh','HelpResetOutcome','install_receiver_reset_cleanup']],
 ['crates/razer-pages/src/features/source_help.rs',['HelpResetOutcome','accept_source_serial','HelpResetEvent::Requested','HelpResetEvent::Canceled']],
 ['crates/razer-pages/src/features/source_workspace.rs',['HelpResetOutcome','outcome.serial_number','_receiver_source_document','finish_reset']],
])for(const needle of required)if(!read(file).includes(needle))throw Error('Missing connected implementation '+file+': '+needle);
const evidence={schema_version:1,generator_sha256:hash(read('tools/audit-receiver-reset-current.cjs')),
 offset_unit:'UTF-16 code units; end exclusive; SHA-256 covers actual UTF-8 source bytes',products,receipts,implementation,
 semantics:['Normal Help emits ON_RESET_DEVICE with empty payload. No key/globalProfileKeys means taskMakerResetOBM, not DeviceResetFeature.',
  'Current 164/241 do not enable deviceReset generic feature or isOBMDevice/singleProfileDevice. No reset opcode is used by this reachable Help task.',
  'Task preserves existing profiles, appends a uniquely named UUID default profile, changes activeProfile and serial deviceMetadatas, increments the 53-bit version, saves local storage, then publishes/refills profile state.',
  'Before reset, b4 creates a separately named default profile when the actual serial lacks a truthy activeProfileGuid, then selects that serial metadata. This does not increment the source document version.',
  'Profile refresh queues brightness, effects and empty mappings. The initial completed UI notification precedes these operations and is not device-write acceptance.',
  'Help re-enables after a two-second source timer. That timer is not a hardware acknowledgment.'],
 gaps:['Receiver brightness uses the original pre-read/conditional setter executor with no post-setter getter or value comparison; current source evidence is recorded separately in receiver-brightness-current-evidence.json. Effect switching and mapping engine submission remain required before full reset completion can be reported.',
  'Original host publication, memory cache/device storage synchronization, storage overrides, migration of older schema versions and linked sub-device metadata scopes are not implemented by this local document adaptation.',
  'The original two-second button cooldown is implemented separately from internal real operation pending/error/cancellation. No fabricated progress/discard panel is inserted under the original reset widget; that timer is never treated as device acceptance.',
  'Local JSON storage is an application adaptation; it is separate from the original Chromium device storage and must not be claimed as its format.',
  'No application, builds, tests, target JavaScript or DLL executed; runtime acceptance remains unverified.']};
for(const[target,value]of [['assets/data/receiver-reset-defaults.json',asset],['docs/re/receiver-reset-current-evidence.json',evidence]]){
 const text=JSON.stringify(value,null,2)+'\n';if(process.argv.includes('--check')){if(read(target)!==text)throw Error('Stale '+target);}else fs.writeFileSync(path.join(root,target),text);
}
console.log(JSON.stringify({products:products.length,receipts:receipts.length}));
