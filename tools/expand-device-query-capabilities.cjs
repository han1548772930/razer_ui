// Product-by-product source gates for the portable shared query codec.
const assert=require('assert'),acorn=require('acorn');
const {inspect,canonical}=require('./device-query-source.cjs');
const {walk,key}=require('./webpack-source.cjs');
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
   direct_pids:[info.productId,info.dongleId].filter(Number.isInteger),vendor_id:info.vendorId,claim_interface:info.claimInterface,report_id:0,report_bytes:91,transaction_prefix:timing.transaction_prefix,transaction_modulus:timing.transaction_modulus,
   min_dpi:audit.infoField('minDPI'),max_dpi:audit.infoField('maxDPI'),queries,polling_codes:reference.polling_codes,charging_codes:reference.charging_codes,alternate_commands:alternate,receipts:audit.receipts,acquisition:audit.source.acquisition});
 }
 return{products,gaps};
}
module.exports={expand};
