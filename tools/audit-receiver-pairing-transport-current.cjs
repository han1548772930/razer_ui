// Current 164/241 command and asynchronous event evidence. Parse vendor bytes only.
const fs=require('fs'),path=require('path'),assert=require('assert');
const {CurrentMiddlewareSource}=require('./current-middleware-source.cjs');
const {Source,walk,key,hash}=require('./webpack-source.cjs');
const acorn=require('acorn'),root=path.resolve(__dirname,'..'),receipts=[],products=[];
const read=f=>fs.readFileSync(path.join(root,f),'utf8');
function record(s,id,name,node){const r={product_id:s.product,module:id,name,...s.receipt(id,node)};receipts.push(r);return r.source;}
function exported(s,id,name){let n=s.exported(id,name);if(n.type==='Identifier')n=s.binding(id,n.name);record(s,id,name,n);return n;}
for(const product of [164,241]){
 const s=new CurrentMiddlewareSource(product),main=s.text(s.mainFile),features=[],allFeatures=[];
 walk(acorn.parse(main,{ecmaVersion:'latest'}),n=>{
  if(n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&key(n.callee.property)==='useFeature')allFeatures.push(n.arguments[0]?.value);
  if(n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&key(n.callee.property)==='useFeature'&&['canPairMultipleDevices','rzDeviceType'].includes(n.arguments[0]?.value)){
   features.push(n);receipts.push({product_id:product,name:'current product feature',path:s.mainFile,sha256:hash(main),offset:n.start,end:n.end,source:main.slice(n.start,n.end)});
  }
 });
 assert(!allFeatures.includes('useHidHwEvents'));
 assert(!allFeatures.includes('isSidePanel'));
 receipts.push({product_id:product,name:'current main feature scope: original mapping_engine event producer',path:s.mainFile,sha256:hash(main),feature_names:allFeatures,useHidHwEvents:false});
 assert.equal(features.filter(n=>n.arguments[0].value==='canPairMultipleDevices').length,product===241?1:0);
 if(product===241)assert.equal(s.literal(34340,features.find(n=>n.arguments[0].value==='canPairMultipleDevices').arguments[2]),true);
 const commands={};for(const name of ['fr','jI','pJ']){const n=exported(s,30580,name);commands[name]=s.literal(30580,n.arguments[0]);}
 assert.deepEqual(commands,{fr:[3,0,65],jI:[2,0,66],pJ:[1,0,70]});
 for(const name of ['ox','Mk','O7','ki','q8'])exported(s,84816,name);
 record(s,84816,'edition response byte parser',s.binding(84816,'y'));
 for(const name of ['Se','cr'])exported(s,30580,name);
 exported(s,96204,'v5');
 record(s,34340,'postpair factory',s.binding(34340,'$e'));
 record(s,34340,'mouse layout fallback',s.binding(34340,product===164?'_e':'he'));
 let base=s.exported(7755,'default');let aliases=0;
 while(base.type==='Identifier'){assert(aliases++<8);base=s.binding(7755,base.name);}
 walk(base,n=>{if(n.type==='MethodDefinition'&&['getEdition','getHWModule'].includes(key(n.key)))record(s,7755,'postpair '+key(n.key),n);});
 for(const name of ['ZK','J7','yi','jQ'])exported(s,50652,name);
 for(const name of ['U1','ft','Zz'])exported(s,52975,name);
 for(const name of ['q','Se','Te','Ee','Ke','j','Y','Z','me','Ie','ye','X','ee','Q','Ne','Ce','De','ge','Me','N','Ye','Ze','Be'])record(s,34340,name,s.binding(34340,name));
 exported(s,21503,'DeviceInfo');
 const f2=exported(s,20236,'f2');assert(s.snippet(20236,f2).includes('5*B.SLEEP_TIME_BETWEEN_OUT'));
 const parser=exported(s,57368,'default');
 const parserText=s.snippet(57368,parser);
 assert(parserText.includes('device_1_editionId:e[7]')&&parserText.includes('device_1_keyboardLayout:e[6]'));
 assert(parserText.includes('PAIR_STATUS_UPDATE')&&parserText.includes('SCAN_STATUS_UPDATE'));
 const cap=JSON.parse(read('assets/data/receiver-query-capabilities.json')).capabilities.find(c=>c.product_id===product);assert(cap);
 const constants={};for(const name of ['SLEEP_TIME_BETWEEN_OUT','SLEEP_TIME_BETWEEN_OUT_IN','SLEEP_TIME_BETWEEN_IN','MAX_RETRY_IN','MAX_RETRY_OUT']){
  const node=exported(s,35284,name);constants[name]=s.literal(35284,node);
 }
 assert.deepEqual(constants,{SLEEP_TIME_BETWEEN_OUT:5,SLEEP_TIME_BETWEEN_OUT_IN:5,SLEEP_TIME_BETWEEN_IN:5,MAX_RETRY_IN:10,MAX_RETRY_OUT:20});
 assert.equal(cap.sleep_between_out_ms,5*constants.SLEEP_TIME_BETWEEN_OUT);
 assert.equal(cap.sleep_between_out_in_ms,5*constants.SLEEP_TIME_BETWEEN_OUT_IN);
 assert.equal(cap.sleep_between_in_ms,5*constants.SLEEP_TIME_BETWEEN_IN);
 assert.equal(cap.max_retry_in,constants.MAX_RETRY_IN);assert.equal(cap.max_retry_out,constants.MAX_RETRY_OUT);
 const cls=record(s,48320,'post metadata mouse class constructor timing',s.binding(48320,'f'));
 assert(cls.includes('c=i.MAX_RETRY_IN,d=i.MAX_RETRY_OUT'));
 if(product===164){
  // Actual default linker: unlike V2, Pair/Scan clear the transaction prefix.
  // Lazy chunks reuse IDs; scope the actual selected Linker file before lookup.
  const linker=new CurrentMiddlewareSource(product);
  linker.parse(linker.files.find(f=>/\/rzDevice25Linker\./.test(f)));
  const cls=linker.binding(91818,'h');
  const text=record(linker,91818,'linker class and command-specific transaction',cls);
  assert(text.includes('f[1]&=31')&&text.includes('224|this.transactionId++'));
  const postClass=record(s,48320,'postpair mouse-class ordinary prefix',s.binding(48320,'f'));
  assert(postClass.includes('this.transactionId++')&&!postClass.includes('128|this.transactionId++'));
  products.push({product_id:product,category:'MOUSE',scan_pair_prefix:0,unpair_prefix:224,cancel_prefix:0,metadata_prefix:0,transport:cap});
  products.push({product_id:product,category:'KEYBOARD',scan_pair_prefix:0,unpair_prefix:224,cancel_prefix:0,metadata_prefix:0,transport:cap});
 }else{
  // f2(category,true): target keyboard uses mouse class; target mouse uses keyboard.
  assert.equal(s.snippet(20236,s.binding(20236,'F')),'n(48320)');
  assert.equal(s.snippet(20236,s.binding(20236,'V')),'n(14770)');
  const txt=s.snippet(20236,f2);assert(txt.includes('true:{[ot.KEYBOARD]:F.default,[ot.MOUSE]:V.default}'));
  for(const [module,category,scanPair,unpair,guard]of [[48320,'KEYBOARD',128,0,'f[1]|=128'],[14770,'MOUSE',0,128,'f[1]&=31']]){
   const txt=record(s,module,'category transport class',s.binding(module,'f'));assert(txt.includes(guard));
   const ordinary=category==='KEYBOARD'?128:0;
   products.push({product_id:product,category,scan_pair_prefix:scanPair,unpair_prefix:unpair,cancel_prefix:0,metadata_prefix:ordinary,transport:cap});
  }
  const primary=new CurrentMiddlewareSource(product);primary.parse(primary.files.find(f=>/\/rzDevice25LinkerMultiDevices\./.test(f)));
  const parent=record(primary,91818,'primary linker cancel command prefix',primary.binding(91818,'h'));
  assert(parent.includes('v[1]&=31')&&parent.includes('e[2]!==c.pJ[2]'));
 }
}
const ui=Object.create(Source.prototype);ui.directory='.ref/devices/241';
const uiManifest=JSON.parse(read(ui.directory+'/asset-manifest.json'));
ui.files=[...new Set(Object.values(uiManifest.files))].filter(f=>/^\.\/static\/js\/[^/]+\.js$/.test(f)).map(f=>ui.directory+'/'+f.slice(2));
ui.modules=new Map();ui.texts=new Map();ui.parsed=new Set();
const uiMode=ui.binding(3746,'He');assert.equal(uiMode.value,1);
const uiModeReceipt=ui.receipt(3746,uiMode);
assert.equal(JSON.parse(read(uiModeReceipt.path+'.http.json')).sha256,uiModeReceipt.sha256);
receipts.push({product_id:241,module:3746,name:'Current UI start pairing mode',...uiModeReceipt});
for(const name of ['ds','as','is'])receipts.push({product_id:241,module:3746,name:'UI caller and dongle-first unpair argument '+name,...ui.receipt(3746,ui.binding(3746,name))});
const catalogData={};
for(const name of ['AvailableDevices','DualDongleCompatibleDevices']){
 const file='.ref/applications/synapse/dashboard/'+name+'.json',bytes=fs.readFileSync(path.join(root,file)),receipt=JSON.parse(read(file+'.http.json'));
 assert.equal(receipt.source_url,'https://apps.razer.com/synapse/dashboard/'+name+'.json');
 assert.equal(receipt.http_status,200);assert.equal(receipt.sha256,hash(bytes));assert.equal(receipt.bytes,bytes.length);
 catalogData[name]=JSON.parse(bytes);
 receipts.push({name:'Current '+name+' catalog',path:file,sha256:hash(bytes),bytes:bytes.length,acquisition:receipt});
}
const receiverRows=catalogData.DualDongleCompatibleDevices.Devices.filter(r=>[164,241].includes(r.DeviceDonglePid));assert.equal(receiverRows.length,2);
const accepted=new Set(receiverRows.flatMap(r=>r.DevicePairingBuddies));
const masters=[164,241].map(product=>{
 const s=new CurrentMiddlewareSource(product);let d=s.exported(21503,'DeviceInfo');if(d.type==='Identifier')d=s.binding(21503,d.name);
 const properties=new Map(d.properties.map(p=>[key(p.key),p.value]));
 const number=name=>s.literal(21503,properties.get(name));
 const master={productId:number('productId'),dongleId:number('dongleId'),bleId:properties.has('bleId')?number('bleId'):0,claimInterface:number('claimInterface')};
 if(properties.has('supports8KHzPollingRate'))master.supports8KHzPollingRate=number('supports8KHzPollingRate');
 return master;
});
const scanCatalog={schema_version:1,source_scope:[164,241],masters,devices:catalogData.DualDongleCompatibleDevices.Devices.filter(r=>accepted.has(r.DeviceDonglePid)||[164,241].includes(r.DeviceDonglePid)),available:catalogData.AvailableDevices.filter(r=>Number.isInteger(r.dongleId)&&accepted.has(r.dongleId))};
const host='.ref/host-4.0.827/electron/modules/mapping_engine/win/index.js';
const hostText=read(host);
walk(acorn.parse(hostText,{ecmaVersion:'latest'}),n=>{
 if(n.type==='PropertyDefinition'&&['createCbHardwareEvent','registerHardwareEvent','unregisterHardwareEvent','createAPIData','createDeviceKey'].includes(key(n.key)))receipts.push({name:key(n.key),path:host,sha256:hash(hostText),offset:n.start,end:n.end,source:hostText.slice(n.start,n.end)});
});
const nativeFollowup='docs/re/evidence/ida-native/receiver-pairing-mapping-engine-current.json';
const nativeDetail=JSON.parse(read(nativeFollowup));
assert.equal(nativeDetail.input_sha256,'6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a');
assert.equal(nativeDetail.data_receipts[0].guid,'8c7ed206-3f8a-4827-b3ab-ae9e1faefc6c');
assert.equal(nativeDetail.data_receipts[0].property_id,2);
for(const rva of [0x11a702,0x119de6,0x1232f8,0x2b770,0x2b8b0])assert(nativeDetail.functions.some(f=>f.rva===rva&&f.pseudocode));
const nativeCorpus=JSON.parse(read('docs/re/ida-native-corpus-current.json'));
const mapping=nativeCorpus.files.find(f=>f.path.endsWith('CommonDLL/mapping_engine.dll'));
assert(mapping && mapping.sha256==='6eabdfdedf797e042738b630d827c06f7a45dbe560ebaec96c66698f88f3320a');
const nativeTransport={
  path:mapping.path, sha256:mapping.sha256, analysis:mapping.evidence, followup:{path:nativeFollowup,sha256:hash(read(nativeFollowup))},
  selected_exports:{registerHardwareEvent_rva:'0x29f30',setHardwareEventCallback_rva:'0x2ba40',unregisterHardwareEvent_rva:'0x2af60'},
  endpoint:{constructor_rva:'0x122fd8',hardware_thread_rva:'0x123720',on_input_report_rva:'0x1244e0',open:'CreateFileA(GENERIC_READ, FILE_SHARE_READ|FILE_SHARE_WRITE, OPEN_EXISTING, FILE_FLAG_OVERLAPPED)'},
  callback_abi:'void callback(device_info_json:string, event_type:int, event_json:string, timestamp:uint64); callback is forwarded only for event_type==0 and event_json is parsed as a JSON byte array',
  identity:'native mapping_engine key is productId-containerId-usbInstanceId; registration and callback require the exact retained USB device record',
  selector:{rva:'0x119de6',forwarder_rva:'0x11a702',property_key_data_rva:'0x2e9a38',property_key_guid:'8c7ed206-3f8a-4827-b3ab-ae9e1faefc6c',property_key_pid:2,algorithm:'Enumerate present HID device interfaces; compare lowercase DEVPKEY_Device_ContainerId and PID parsed from each path; return all matching collections, without usage/page or USB-interface selection.'},
  input_read:{rva:'0x1232f8',capacity_bytes:100,algorithm:'Overlapped ReadFile into a 100-byte capacity; OnInputReport forwards only the actual completion byte count; callback serializer 0x2b770 emits each actual byte, without a synthetic report prefix.'},
  lifecycle:'registerHardwareEvent creates the endpoint/thread; unregisterHardwareEvent shuts down after IO-thread cleanup; failed endpoint reconnects before callback delivery',
};
const gaps=[
 'Runtime acceptance is unverified: actual collections are re-observed and retained by direct interrupt provider; unsupported descriptor/identity or offline endpoints fail instead of returning fabricated events.',
 'Post-pair edition/[1,0,185] sidepad queries and source-explicit fallback are implemented; source runtime sidepad precedence needs actual scoped DEVICE_RUNTIME_DATA and serial/profile merge inputs.',
 'Local duallink-devices add/remove persistence is implemented and connected after hardware evidence; binding readback/connection publication is distinct from full original runtime/profile merge, routing reconnect and window-storage serial listener behavior, which remain gaps.',
 'Failed-unpair 2000ms V2 query recovery is implemented; category-specific mapping/profile recombination and native reconnect semantics remain incomplete.',
 'Protocol25 scan parser exposes three records; unsupported counts fail rather than fabricate device identity.',
];
const implementation=['crates/razer-device/src/receiver_pairing.rs','crates/razer-service/src/runtime/windows/receiver_events.rs','crates/razer-service/src/runtime/windows/receiver_pairing.rs','crates/razer-discovery/src/receiver_pairing.rs','crates/razer-storage/src/receiver_pairing.rs','crates/razer-shell/src/shell/receiver_pairing.rs','crates/razer-shell/src/shell/receiver_pairing/route.rs','crates/razer-shell/src/shell/receiver_pairing/route/windows.rs','crates/razer-pages/src/features/dock_pairing/observation.rs','crates/razer-pages/src/features/dock_pairing/state.rs'];
const implementationReceipts=implementation.map(file=>{const bytes=fs.readFileSync(path.join(root,file));return{path:file,sha256:hash(bytes),bytes:bytes.length,runtime_acceptance:false};});
const evidence={schema_version:1,method:'Current acquisition-verified 164/241 AST, current official host source and retained IDA mapping_engine analysis; vendor JavaScript and native targets never executed',generator_sha256:hash(fs.readFileSync(__filename)),receipts,products,native_transport:nativeTransport,gaps,implementation,implementation_receipts:implementationReceipts,metadata_transport:{factory_sleep_multiplier:5,base_sleep_ms:5,actual_sleep_ms:25,max_retry_in:10,max_retry_out:20,edition_command:[3,0,134],edition_parser:'data[0]=keyboardLayout, data[1]=edition, data[2]=firmwareId',edition_helper_retries:3,edition_helper_retry_delay_ms:100,mouse_layout_command:[1,0,185],mouse_layout_parser:'data[0]=status; actual scoped runtime sidepadLayout has precedence in the original caller'},scan_catalog:{asset:'assets/data/receiver-pairing-catalog.json',devices:scanCatalog.devices.length,available:scanCatalog.available.length,raw_dongle_is_not_product_id:true},worker_actions:['ReceiverPairingStart','ReceiverPairingPoll','ReceiverPairingCancel']};
const assets={schema_version:1,products};
for(const [file,value]of [['docs/re/receiver-pairing-transport-current-evidence.json',evidence],['assets/data/receiver-pairing-capabilities.json',assets],['assets/data/receiver-pairing-catalog.json',scanCatalog]]){
 const text=JSON.stringify(value,null,2)+'\n';if(process.argv.includes('--check'))assert.equal(read(file),text,'Stale '+file);else fs.writeFileSync(path.join(root,file),text);
}
console.log(`Receiver pairing transport: ${products.length} source-gated category routes, ${receipts.length} current receipts.`);
