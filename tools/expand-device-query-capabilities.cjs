// Product-by-product source gates for the portable shared query codec.
const assert=require('assert'),acorn=require('acorn');
const {inspect,canonical}=require('./device-query-source.cjs');
const {walk,key}=require('./webpack-source.cjs');
// Preserve raw DeviceInfo.claimInterface in evidence. Runtime readers require
// one u8 for the primary source PID and an explicit complete per-PID map when
// the original field is a map; never choose the first entry as a fallback.
function sourceClaimInterfaces(info){
 assert(Number.isInteger(info.productId)&&info.productId>=0&&info.productId<=0xffffffff,'DeviceInfo.productId is not a PID integer');
 const raw=info.claimInterface;
 const u8=value=>Number.isInteger(value)&&value>=0&&value<=255;
 if(typeof raw==='number'){
  assert(u8(raw),'Scalar claimInterface is not u8');
  return{claim_interface:raw};
 }
 assert(raw&&typeof raw==='object'&&!Array.isArray(raw),'claimInterface is neither a scalar u8 nor a PID map');
 const entries=Object.entries(raw);
 for(const[pid,value]of entries){
  assert(/^(0|[1-9]\d*)$/.test(pid)&&Number.isInteger(Number(pid))&&Number(pid)<=0xffffffff,'claimInterface map key is not a canonical PID integer: '+pid);
  assert(u8(value),'claimInterface map value is not u8 for PID '+pid);
 }
 const primary=String(info.productId);
 assert(Object.hasOwn(raw,primary),'claimInterface map has no exact DeviceInfo.productId entry: '+primary);
 return{claim_interface:raw[primary],claim_interfaces_by_pid:Object.fromEntries(entries)};
}
function recordClaimInterfaceSelector(audit,plan){
 const factory=acorn.parseExpressionAt(audit.source.text(plan.factory.path),plan.factory.offset,{ecmaVersion:'latest'});
 assert.equal(audit.source.snippet(plan.factory.module,factory),plan.factory.source,'Current interface factory receipt changed');
 const selectors=[];
 walk(factory,node=>{
  if(node.type==='SwitchStatement'&&audit.source.snippet(plan.factory.module,node).includes('DeviceInfo.claimInterface['))selectors.push(node);
 });
 assert.equal(selectors.length,1,'Ambiguous current number/map interface selector');
 const selector=selectors[0],cases=selector.cases;
 assert.equal(cases.length,6,'Current interface factory override branches changed');
 const bladeTypes=['rzDevice25LedMatrixBlade','rzDevice25LedMatrixBlade2','rzDevice25LedMatrixBladeWattage'];
 for(let index=0;index<bladeTypes.length;index++){
  const test=cases[index].test;assert.equal(test?.callee?.name,'Boolean');
  assert.equal(key(test.arguments[0]?.property),bladeTypes[index]);
 }
 assert.equal(cases[2].consequent[0]?.argument?.value,2,'Blade interface override changed');
 const typed=type=>cases.find(branch=>branch.test?.type==='BinaryExpression'
  &&['==','==='].includes(branch.test.operator)&&branch.test.left.value===type
  &&branch.test.right.type==='UnaryExpression'&&branch.test.right.operator==='typeof');
 const numeric=typed('number'),mapped=typed('object');assert(numeric&&mapped,'Interface selector type branches changed');
 const returned=branch=>branch.consequent.find(node=>node.type==='ReturnStatement')?.argument;
 const numericValue=returned(numeric),mapValue=returned(mapped);
 assert.equal(key(numericValue?.property),'claimInterface');
 assert.equal(audit.source.snippet(plan.factory.module,numericValue),audit.source.snippet(plan.factory.module,numeric.test.right.argument));
 assert.equal(mapValue?.type,'MemberExpression');assert(mapValue.computed,'Interface map is no longer indexed');
 assert.equal(audit.source.snippet(plan.factory.module,mapValue.object),audit.source.snippet(plan.factory.module,numericValue));
 assert.equal(mapValue.property?.type,'MemberExpression');assert.equal(key(mapValue.property.property),'productId');
 const actualPid=mapValue.property.object.name;assert.equal(actualPid,factory.params[0]?.name,'Interface lookup does not use connection input PID');
 let selectedVariable;
 walk(factory,node=>{
  if(node.type!=='VariableDeclarator'||node.id.type!=='Identifier')return;
  if(node.init?.start<=selector.start&&node.init?.end>=selector.end)selectedVariable=node.id.name;
 });
 assert(selectedVariable,'Missing selected interface local');
 const constructors=[];
 walk(factory,node=>{
  if(node.type==='NewExpression'&&node.arguments[0]?.name===actualPid&&node.arguments[1]?.name===selectedVariable)constructors.push(node);
 });
 assert(constructors.length,'Selected interface is not forwarded with actual connection object');
 audit.record('claimInterface number or map selector by actual connection PID',{id:plan.factory.module,node:selector});
 for(const node of constructors)audit.record('selected claimInterface forwarded to actual device constructor',{id:plan.factory.module,node});
 const assignments=[],undefinedDefaults=[];
 for(const owner of audit.chain){
  const constructor=owner.node.body.body.find(node=>key(node.key)==='constructor');if(!constructor)continue;
  walk(constructor,node=>{
   if(node.type==='AssignmentExpression'&&node.left.type==='MemberExpression'
    &&node.left.object.type==='ThisExpression'&&key(node.left.property)==='claimInterface'
    &&node.right.type==='Identifier'&&node.right.name===constructor.value.params[1]?.name)assignments.push({id:owner.id,node});
   if(node.type==='LogicalExpression'&&node.operator==='&&'
    &&audit.source.snippet(owner.id,node).replace(/\s+/g,'')==='void0===this.claimInterface&&(this.claimInterface=0)')undefinedDefaults.push({id:owner.id,node});
  });
 }
 assert.equal(assignments.length,1,'Claim interface constructor assignment changed');
 audit.record('constructor retains selected claimInterface',assignments[0]);
 assert.equal(undefinedDefaults.length,1,'Original undefined claimInterface fallback changed');
 audit.record('original constructor defaults undefined interface to zero',undefinedDefaults[0]);
 const connection=audit.method('connectDevice'),source=audit.source.snippet(connection.id,connection.node);
 assert(source.includes('productId:this.device.productId||this.productId')&&source.includes('claimInterface:this.claimInterface')&&source.includes('.connectHidDevice('),'Selected actual interface is not sent to current host');
 audit.record('selected actual PID/interface forwarded to host connectHidDevice',connection);
 return{default_direct_wired:'number uses the original scalar; object indexes the original complete map by the actual connection object productId, then forwards it to the constructor and host',
  blade_override:'Three original rzDeviceType Blade branches select interface2 before the number/map cases; the current device-type model and route do not fully implement those factory branches',
  source_missing_map_entry:'original map lookup returns undefined, then base constructor defaults an undefined claimInterface to0',
  application_route_policy:'Rust rejects missing PID entries explicitly as a conservative application routing policy; this is not the original constructor fallback and does not complete all factory conditions'};
}
function expand(plans,boots,reference,timings){
 const products=[],gaps=[];
 const ref=inspect(plans.products.find(p=>p.product_id===182),boots.products.find(p=>p.product_id===182));
 const same=(audit,target,expected,label)=>assert.equal(canonical(audit.source,target.id,target.node),canonical(ref.source,expected.id,expected.node),'Different source semantics: '+label);
 const specs=[['firmware','getExtendedFirmwareVersion',[],4],['battery','getBatteryLevel',[0],2],['charging','getChargingStatus',[0],2],['polling','getUSBHighSpeedPollingRate',[1],2],['dpi','getDpiLevel',[0],7],['idle','getTimeToSleep',[],2]];
 const refQueries=new Map(specs.map(([,name])=>[name,ref.query(name)]));
 const transportNames=['_createDataSend','_calculateChecksum','_getUSBTransferInResult','connectDevice','sendCommand'];
 const refTransport=new Map(transportNames.map(name=>[name,ref.method(name)]));
 const sendShape=(audit,target,keyboard)=>{
  const clone=structuredClone(target.node);let masks=0;
  walk(clone,n=>{
   if(keyboard&&n.type==='AssignmentExpression'&&n.operator==='&='&&n.right.value===31&&n.left.type==='MemberExpression'&&n.left.computed&&n.left.property.value===1){n.operator='|=';n.right.value=128;masks++;}
   // The observed keyboard variant logs final failure at error level; mouse
   // logs warn. Both retain identical thrown error and transport state.
   if(n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&['error','warn'].includes(key(n.callee.property))&&n.callee.object.type==='MemberExpression')n.callee.property.name='sourceDiagnostic';
  });
  if(keyboard)assert.equal(masks,1,'Keyboard special-command mask changed');
  return canonical(audit.source,target.id,clone);
 };
 for(const plan of plans.products){
  if(plan.product_id===182)continue;
  if(!['rzDevice25DualLinkMouse','rzDevice25DualLinkKeyboard'].includes(plan.selected_class)){gaps.push({product_id:plan.product_id,scope:'product',reason:'Factory is not a proved direct mouse/keyboard binding'});continue;}
  const boot=boots.products.find(p=>p.product_id===plan.product_id);
  let audit,timing,alternate;
  let claimInterfaceSelection;
  try{
   audit=inspect(plan,boot);timing=timings.capabilities.find(c=>c.source_product_id===plan.product_id);
   assert(timing&&timing.source_class===plan.selected_class,'Missing source-matched transport timing');
   for(const name of transportNames){
    const target=audit.method(name);
    if(name==='sendCommand')assert.equal(sendShape(audit,target,plan.selected_class==='rzDevice25DualLinkKeyboard'),sendShape(ref,refTransport.get(name),false),'Different source semantics: sendCommand');
    else same(audit,target,refTransport.get(name),name);
   }
   alternate=audit.alternateCommands();
   const txn=audit.method('_getTransactionId'),expected=acorn.parse('class T{_getTransactionId(){return 31===this.transactionId&&(this.transactionId=0),'+(timing.transaction_prefix?timing.transaction_prefix+'|':'')+'this.transactionId++}}',{ecmaVersion:'latest'}).body[0].body.body[0];
   assert.equal(canonical(audit.source,txn.id,txn.node),canonical(audit.source,txn.id,expected),'Transaction recipe differs');
   const initial=[];for(const owner of audit.chain)walk(owner.node.body.body.find(m=>key(m.key)==='constructor'),n=>{if(n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&n.left.object.type==='ThisExpression'&&key(n.left.property)==='transactionId')initial.push({id:owner.id,node:n});});
   assert.equal(initial.length,1,'Ambiguous transaction initialization');assert.equal(initial[0].node.right.value,0);audit.record('transaction initial value',initial[0]);
  }catch(error){gaps.push({product_id:plan.product_id,scope:'transport',reason:error.message.split('\n')[0]});continue;}
  // Reject an unresolved source interface before emitting a runtime product.
  // Original scalar/map evidence is retained unchanged in the product record.
  try{
   assert.equal(plan.device_info.values.productId,audit.infoField('productId'),'DeviceInfo primary PID differs from current source literal');
   assert.deepEqual(plan.device_info.values.claimInterface,audit.infoField('claimInterface'),'DeviceInfo claimInterface differs from current source literal');
   sourceClaimInterfaces(plan.device_info.values);
   if(typeof plan.device_info.values.claimInterface==='object')claimInterfaceSelection=recordClaimInterfaceSelector(audit,plan);
  }
  catch(error){gaps.push({product_id:plan.product_id,scope:'claim_interface',reason:error.message.split('\n')[0]});continue;}
  const queries=[];
  // The actual caller modules are product-scoped, including keyboard callers.
  const batteryCallers=audit.definitionsContaining('.getBatteryList(1)');
  const batteryId=batteryCallers.find(t=>{let matched=false;walk(t.node,n=>{if(n.type==='IfStatement'&&n.alternate){let zero=false;walk(n.alternate,p=>{if(p.type==='AssignmentExpression'&&p.right.value===0)zero=true;});if(zero)matched=true;}});return matched;});
  if(batteryId)audit.record('battery id caller',batteryId);
  for(const [name,method,payload,minBytes]of specs){
   if(name==='dpi'&&plan.selected_class!=='rzDevice25DualLinkMouse')continue;
   if(name==='idle'&&plan.selected_class!=='rzDevice25DualLinkMouse')continue;
   try{
    if(['battery','charging'].includes(name))assert(batteryId,'No source caller proving direct battery ID 0');
    const candidate=audit.query(method),original=refQueries.get(method);
    same(audit,candidate.helper,original.helper,name+' delegate');
    assert.deepEqual(candidate.command,original.command,'Command differs');assert.equal(candidate.parsers.length,original.parsers.length,'Parser count differs');
    assert(!alternate.some(h=>h.command[2]===candidate.command[2]),'Query needs alternate transaction policy');
    for(let ix=0;ix<candidate.parsers.length;ix++)same(audit,candidate.parsers[ix],original.parsers[ix],name+' parser');
    if(name==='polling'){
     // Mouse feature declarations decide which of the caller's actual branches
     // uses the high-speed command. A shared base method alone is insufficient.
     const highSpeed=audit.features().some(f=>f.name==='highSpeedPollingRate'&&f.value==='yes');
     assert(highSpeed,'Current product does not select highSpeedPollingRate=yes');
     const callers=audit.definitionsContaining('.getUSBHighSpeedPollingRate(');
     const caller=callers.find(t=>t.node.params[0]?.type==='AssignmentPattern'&&t.node.params[0].right.value===1);
     assert(caller,'No caller proving default polling profile 1');audit.record('polling caller/default profile',caller);
    }
    if(name==='dpi'){
     const callers=audit.definitionsContaining('.getDpiLevel(0)'),caller=callers.find(t=>audit.source.snippet(t.id,t.node).includes('.setDpiLevel(0,'));assert(caller,'No current DPI selector 0 read/write caller');audit.record('DPI read-before-write caller',caller);
    }
    if(name==='idle'){
     assert(audit.features().some(f=>f.name==='isBattery'&&f.value==='yes'),'No actual battery feature registration for power controls');
     const caller=audit.definitionsContaining('.setTimeToSleep(').find(t=>{
      const text=audit.source.snippet(t.id,t.node);
      return text.includes('.getTimeToSleep()')&&text.includes('new Error("data not match")')&&text.includes('60*')&&text.includes('singleProfileDevice');
     });
     assert(caller,'No current sleep setter/readback task caller');audit.record('idle write/readback caller and units',caller);
    }
    queries.push({name,method,command:candidate.command,payload,min_response_bytes:minBytes});
   }catch(error){gaps.push({product_id:plan.product_id,scope:name,reason:error.message.split('\n')[0]});}
  }
  if(!queries.length)continue;
  const info=plan.device_info.values;
  products.push({product_id:plan.product_id,source_class:plan.selected_class,device_info:plan.device_info,factory:plan.factory,bootstrap:boot.bootstrap,
   ...(claimInterfaceSelection?{claim_interface_selection:claimInterfaceSelection}:{}),
   direct_pids:[info.productId,info.dongleId].filter(Number.isInteger),vendor_id:info.vendorId,claim_interface:info.claimInterface,report_id:0,report_bytes:91,transaction_prefix:timing.transaction_prefix,transaction_modulus:timing.transaction_modulus,
   min_dpi:audit.infoField('minDPI'),max_dpi:audit.infoField('maxDPI'),queries,polling_codes:reference.polling_codes,charging_codes:reference.charging_codes,alternate_commands:alternate,receipts:audit.receipts,acquisition:audit.source.acquisition});
 }
 return{products,gaps};
}
module.exports={expand,sourceClaimInterfaces};
