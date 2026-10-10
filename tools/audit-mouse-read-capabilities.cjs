// Resolve source-owned query methods, parameters and decoders as static AST data.
// No vendor module is imported/evaluated, and no hardware query is performed.
const fs=require('fs'),path=require('path');
const acorn=require('acorn');
const {CurrentMiddlewareSource}=require('./current-middleware-source.cjs');
const {walk,key,hash}=require('./webpack-source.cjs');
const {sourceClaimInterfaces}=require('./expand-device-query-capabilities.cjs');
const root=path.resolve(__dirname,'..'),check=process.argv.includes('--check');
const bootstrap=JSON.parse(fs.readFileSync(path.join(root,'.ref/middleware/bootstrap-requests.json'),'utf8'));
const plans=JSON.parse(fs.readFileSync(path.join(root,'.ref/middleware/receiver-protocol-requests.json'),'utf8'));
const readProducts=[182],products=[];
for(const productId of readProducts){
 const source=new CurrentMiddlewareSource(productId),plan=plans.products.find(p=>p.product_id===productId),boot=bootstrap.products.find(p=>p.product_id===productId);
 for(const file of [source.mainFile,...boot.files,...plan.files])source.parse(file);
 for(const receipt of [plan.factory,plan.device_info.receipt,boot.bootstrap]){
  const text=source.text(receipt.path);
  if(hash(text)!==receipt.sha256||text.slice(receipt.offset,receipt.end)!==receipt.source)throw Error('Stale source input receipt');
 }
 const receipts=[];
 const record=(id,label,node)=>{receipts.push({label,module:id,...source.receipt(id,node)});return node};
 const resolve=(id,node)=>{
  const seen=new Set();
  for(;;){const token=id+':'+node.start;if(seen.has(token))throw Error('Circular source alias');seen.add(token);
   if(node.type==='Identifier'){node=source.binding(id,node.name);continue;}
   if(node.type==='SequenceExpression'){node=node.expressions.at(-1);continue;}
   if(node.type==='MemberExpression'&&node.object.type==='Identifier'){
    const imported=source.binding(id,node.object.name);
    if(imported.type==='CallExpression'&&Number.isInteger(imported.arguments[0]?.value)){id=imported.arguments[0].value;node=source.exported(id,key(node.property));continue;}
   }
   return{id,node};
  }
 };
 const chain=[];let target=resolve(plan.class_module,source.exported(plan.class_module,'default'));
 for(let depth=0;depth<8;depth++){
  if(!['ClassExpression','ClassDeclaration'].includes(target.node.type))throw Error('Class binding changed');
  chain.push(target);record(target.id,'class inheritance',target.node.superClass||target.node);
  if(!target.node.superClass)break;target=resolve(target.id,target.node.superClass);
 }
 const method=name=>{
  const owner=chain.find(c=>c.node.body.body.some(m=>key(m.key)===name));
  if(!owner)throw Error('Missing method '+name);
  return{id:owner.id,node:record(owner.id,name,owner.node.body.body.find(m=>key(m.key)===name))};
 };
 const queries=[];
 const specs=[
  ['firmware','getExtendedFirmwareVersion',[],4],
  ['battery','getBatteryLevel',[0],2],
  ['charging','getChargingStatus',[0],2],
 ['polling','getUSBHighSpeedPollingRate',[1],2],
  ['dpi','getDpiLevel',[0],7],
  ['idle','getTimeToSleep',[],2],
 ];
 for(const [name,methodName,payload,minBytes]of specs){
  const m=method(methodName);let helper;
  walk(m.node,n=>{if(n.type==='CallExpression'&&n.arguments[0]?.type==='ThisExpression'&&n.callee.type==='MemberExpression')helper=resolve(m.id,n.callee);});
  // getDpiLevel forwards to the module-local h helper.
  if(!helper)walk(m.node,n=>{if(n.type==='CallExpression'&&n.arguments[0]?.type==='ThisExpression'&&n.callee.type==='Identifier'&&n.arguments.length===2)helper=resolve(m.id,n.callee);});
  if(!helper)throw Error('No exact query helper '+name);
  record(helper.id,name+' parser/delegate',helper.node);
  const sends=[];walk(helper.node,n=>{if(n.type==='CallExpression'&&key(n.callee.property)==='sendCommand')sends.push(n);});
  if(sends.length!==1)throw Error('Unexpected query chain '+name);
  const header=resolve(helper.id,sends[0].arguments[0]);record(header.id,name+' command',header.node);
  if(header.node.type!=='NewExpression'||header.node.callee.name!=='Uint8Array')throw Error('Nonliteral header');
  const command=source.literal(header.id,header.node.arguments[0]);
  // Parsers invoked through module-local function references are also retained.
  walk(helper.node,n=>{if(n.type==='CallExpression'&&n.callee.type==='Identifier'&&n.arguments.length===1){try{const binding=resolve(helper.id,n.callee);if(['ArrowFunctionExpression','FunctionExpression','FunctionDeclaration'].includes(binding.node.type))record(binding.id,name+' result parser',binding.node);}catch{}}});
  queries.push({name,method:methodName,command,payload,min_response_bytes:minBytes});
 }
 for(const name of ['_getTransactionId','_createDataSend','_calculateChecksum','_getUSBTransferInResult','connectDevice','sendCommand'])method(name);
 const wired=chain.find(c=>c.id===87969);
 if(!wired)throw Error('Wired class inheritance changed');
 for(const name of ['constructor','_getTransactionId']){
  const node=wired.node.body.body.find(m=>key(m.key)===name);
  if(!node)throw Error('Missing wired method '+name);record(wired.id,'wired '+name,node);
 }
 const base=chain.find(c=>c.id===41863),initializers=[];
 if(!base)throw Error('Missing transaction owner class');
 walk(base.node.body.body.find(m=>key(m.key)==='constructor'),n=>{
  if(n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&n.left.object.type==='ThisExpression'&&key(n.left.property)==='transactionId')initializers.push(n);
 });
 if(initializers.length!==1)throw Error('Transaction initializer changed');
 record(base.id,'transaction initial value',initializers[0]);
 // DualLinkMouse reserves bit 7 for two unrelated command IDs. None of the
 // allowed read commands may silently inherit that alternate transaction mode.
 const dualHeaderModule=source.binding(48957,'l').arguments[0].value;
 for(const name of ['Z','z$']){
  const header=resolve(dualHeaderModule,source.exported(dualHeaderModule,name));
  record(header.id,'alternate transaction command '+name,header.node);
  const command=source.literal(header.id,header.node.arguments[0]);
  if(queries.some(query=>query.command[2]===command[2]))throw Error('Read needs alternate transaction flag');
 }
 const byteModule=source.binding(87969,'a').arguments[0].value;
 const combine=resolve(byteModule,source.exported(byteModule,'Jz'));
 record(combine.id,'DPI big endian decoder',combine.node);
 for(const [id,name,label]of [[87887,'F','battery id caller'],[87887,'Ne','polling caller/default profile'],[87887,'qe','firmware caller'],[85191,'et','DPI read-before-write caller (only query is in scope)']])record(id,label,source.binding(id,name));
 const polling=resolve(82929,source.exported(82929,'Mt'));record(polling.id,'high-speed polling enum',polling.node);
 const charging=resolve(48328,source.exported(48328,'eD'));record(charging.id,'charging enum',charging.node);
 const info=plan.device_info.values;
 const infoReceipt=plan.device_info.receipt;
 const infoExpression=acorn.parseExpressionAt(source.text(infoReceipt.path),infoReceipt.offset,{ecmaVersion:'latest'});
 const infoNode=infoExpression.type==='SequenceExpression'?infoExpression.expressions[0]:infoExpression;
 const infoField=name=>source.literal(plan.device_info.module,infoNode.properties.find(p=>key(p.key)===name).value);
 // Keep the original map in evidence; validate against the current source
 // literal and select its exact primary PID before emitting runtime data.
 if(info.productId!==infoField('productId')||JSON.stringify(info.claimInterface)!==JSON.stringify(infoField('claimInterface')))throw Error('Primary PID/interface differs from current source literal');
 sourceClaimInterfaces(info);
 const batteryCaller=receipts.find(r=>r.label==='battery id caller').source;
 const pollingCaller=receipts.find(r=>r.label==='polling caller/default profile').source;
 if(!batteryCaller.includes('else V=0')||!pollingCaller.startsWith('(e=1)=>')||!boot.bootstrap.source.includes('useFeature("highSpeedPollingRate","yes")'))throw Error('Caller parameters/features changed; re-audit');
 const exact=(label,expected)=>{if(receipts.find(r=>r.label===label)?.source!==expected)throw Error('Source invariant changed: '+label);};
 const includes=(label,...parts)=>{const text=receipts.find(r=>r.label===label)?.source;if(!text||parts.some(p=>!text.includes(p)))throw Error('Source invariant changed: '+label);};
 exact('_getTransactionId','_getTransactionId(){return 31===this.transactionId&&(this.transactionId=0),this.transactionId++}');
 exact('transaction initial value','this.transactionId=0');
 exact('wired _getTransactionId',receipts.find(r=>r.label==='_getTransactionId').source);
 exact('_calculateChecksum','_calculateChecksum(e){let t=0;for(let n=2;n<88;n++)t=e[n]^t;return t}');
 exact('DPI big endian decoder','(e,t)=>(255&e)<<8|255&t');
 includes('_createDataSend','new Uint8Array(90)','o[1]=i','o[5]=e[0],o[6]=e[1],o[7]=e[2]','o[e+8]=t[e]','o[88]=r');
 includes('_getUSBTransferInResult','l||(l=new Array(90).fill(0))','n.resendOutCommand=!0');
 includes('wired constructor','this.reportLength=91','this.reportId=null!==(u=null==d?void 0:d.reportId)&&void 0!==u?u:0');
 includes('firmware parser/delegate','major:e.data[0],minor:e.data[1],internal:e.data[2],reserved:e.data[3]');
 includes('battery parser/delegate','batteryId:r.data[0],batteryLevel:Math.floor(r.data[1]/255*100)');
 includes('charging parser/delegate','batteryId:c.data[0],chargingStatus:c.data[1]');
 includes('polling result parser','switch(e.data[1])','profileId:e.data[0],pollingRate:t');
 exact('dpi result parser','e=>{e.jsonData={classId:e.data[0],dpiX:a.Jz(e.data[1],e.data[2]),dpiY:a.Jz(e.data[3],e.data[4]),dpiZ:a.Jz(e.data[5],e.data[6])}}');
 includes('DPI read-before-write caller (only query is in scope)','yield r.getDpiLevel(0)');
 exact('idle result parser','e=>{e.jsonData={timeToSleep:a.Jz(e.data[0],e.data[1])}}');
 const idleAudit=require('./device-query-source.cjs').inspect(plan,boot);
 const idleCaller=idleAudit.definitionsContaining('.setTimeToSleep(').find(t=>{
  const text=idleAudit.source.snippet(t.id,t.node);
  return text.includes('.getTimeToSleep()')&&text.includes('new Error("data not match")')&&text.includes('60*')&&text.includes('singleProfileDevice');
 });
 if(!idleCaller)throw Error('Missing sleep setter/readback caller');
 record(idleCaller.id,'idle write/readback caller and units',idleCaller.node);
 products.push({product_id:productId,source_class:plan.selected_class,device_info:plan.device_info,factory:plan.factory,bootstrap:boot.bootstrap,
  direct_pids:[info.productId,info.dongleId],vendor_id:info.vendorId,claim_interface:info.claimInterface,report_id:0,report_bytes:91,transaction_prefix:0,transaction_modulus:31,
  min_dpi:infoField('minDPI'),max_dpi:infoField('maxDPI'),queries,polling_codes:source.literal(polling.id,polling.node),charging_codes:source.literal(charging.id,charging.node),receipts,acquisition:source.acquisition});
}
const timings=JSON.parse(fs.readFileSync(path.join(root,'assets/data/receiver-query-capabilities.json'),'utf8'));
const expansion=require('./expand-device-query-capabilities.cjs').expand(plans,bootstrap,products[0],timings);
products.push(...expansion.products);
require('./expand-polling-capabilities.cjs').expandQueries(products,expansion.gaps,plans,bootstrap);
products.sort((a,b)=>a.product_id-b.product_id);
const hostPath='.ref/host-4.0.827/electron/UsbRzDeviceAction.js';
const hostText=fs.readFileSync(path.join(root,hostPath),'utf8');
const hostStart=hostText.indexOf('case"hid.getFeatureReport"');
const hostEnd=hostText.indexOf('case"hid.sendFeatureReportInBatch"',hostStart);
if(hostStart<0||hostEnd<0)throw Error('Current host HID feature-read handler changed');
const hostReceipt=hostText.slice(hostStart,hostEnd);
if(!hostReceipt.includes('catch(e)')||!hostReceipt.includes('return s'))throw Error('Host feature-read error behavior changed');
const hostEvidence={path:hostPath,sha256:hash(hostText),offset:Buffer.byteLength(hostText.slice(0,hostStart)),end:Buffer.byteLength(hostText.slice(0,hostEnd)),source:hostReceipt};
const report={schema_version:1,method:'Shared current rzDevice25 read methods and actual product binding/call parameters; static evidence only. Every additional product resolves its own factory, inheritance, methods and callers and passes conservative semantic gates. Failed gates remain explicit gaps. No whole task runner, device-mode setter or vendor bootstrap is executable from this artifact.',native_read_retry:{host_get_feature_error:hostEvidence,middleware:'Host catches native getFeatureReport errors and returns undefined; middleware substitutes an empty 90-byte response, detects the missing transaction and requests a new OUT command. The app mirrors this by ending the current IN retry loop and continuing the OUT retry loop.'},products,gaps:expansion.gaps};
const file=path.join(root,'docs/re/mouse-read-capabilities-current-evidence.json'),bytes=JSON.stringify(report,null,2)+'\n';
if(check){if(fs.readFileSync(file,'utf8')!==bytes)throw Error('Stale mouse read evidence');}else fs.writeFileSync(file,bytes);
const runtime=products.map(p=>{
 const timing=JSON.parse(fs.readFileSync(path.join(root,'assets/data/receiver-query-capabilities.json'),'utf8')).capabilities.find(c=>c.source_product_id===p.product_id);
 if(!timing||timing.source_class!==p.source_class||timing.transaction_prefix!==p.transaction_prefix||timing.transaction_modulus!==p.transaction_modulus)throw Error('Read transport class/timing changed');
 // Product UI DPI bounds remain evidence only; the source getter accepts the
 // full raw u16 range, so those edit limits must not become a read capability.
 const interfaces=sourceClaimInterfaces(p.device_info.values);
 return Object.fromEntries(Object.entries({...p,...interfaces,...Object.fromEntries(['max_retry_in','max_retry_out','sleep_between_out_ms','sleep_between_out_in_ms','sleep_between_in_ms'].map(k=>[k,timing[k]]))}).filter(([k])=>!['device_info','factory','bootstrap','receipts','acquisition','min_dpi','max_dpi','claim_interface_selection'].includes(k)));
});
 const runtimeFile=path.join(root,'assets/data/device-read-capabilities.json'),runtimeBytes=JSON.stringify({schema_version:1,products:runtime},null,2)+'\n';
if(check){if(fs.readFileSync(runtimeFile,'utf8')!==runtimeBytes)throw Error('Stale device read capabilities');}else fs.writeFileSync(runtimeFile,runtimeBytes);
console.log(JSON.stringify({products:products.length,queries:products.reduce((n,p)=>n+p.queries.length,0),receipts:products.reduce((n,p)=>n+p.receipts.length,0)}));
