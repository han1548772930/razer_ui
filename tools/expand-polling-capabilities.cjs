// Ordinary/profile polling codecs and actual feature/caller gates. Static AST only.
const assert=require('assert');
const {inspect,canonical}=require('./device-query-source.cjs');
const {walk,key}=require('./webpack-source.cjs');

function directHighSpeedShape(audit,caller){
 let target;walk(caller.node,n=>{if(n.type==='ConditionalExpression'&&n.consequent.type==='YieldExpression'&&key(n.consequent.argument.callee.property)==='getUSBHighSpeedPollingRate')target=audit.resolve(caller.id,n.test.callee);});
 assert(target,'High-speed branch selector unresolved');audit.record('polling physical connection branch selector',target);
 const clone=structuredClone(target.node);let calls=0,device=0,info=0;
 walk(clone,n=>{
  if(n.type==='ChainExpression'){const inner=n.expression;for(const k of Object.keys(n))delete n[k];Object.assign(n,inner);}
  if('optional' in n)n.optional=false;
  if(n.type==='ConditionalExpression'&&n.test.type==='BinaryExpression'&&n.test.operator==='=='&&n.test.left.value===null&&n.consequent.type==='UnaryExpression'&&n.consequent.operator==='void'&&n.consequent.argument.value===0&&n.alternate.type==='MemberExpression'&&key(n.alternate.property)==='productId'&&n.alternate.object.name===n.test.right.name){
   // The retained physical route has already proved this device object exists;
   // current bundles differ only in this optional-null spelling.
   const value=n.alternate;for(const k of Object.keys(n))delete n[k];Object.assign(n,value);
  }
  if(n.type==='CallExpression'&&n.callee.type==='SequenceExpression'&&!n.arguments.length){
   n.callee={type:'Identifier',name:['sourceFeatures','sourceDemo','sourceLinkState'][calls++]};
  }
  if(n.type==='MemberExpression'&&key(n.property)==='rzDevice'){
   assert(['jf','razerWindow'].includes(key(n.object.property)),'Current device binding changed');
   for(const k of Object.keys(n))delete n[k];Object.assign(n,{type:'Identifier',name:'sourceDevice'});device++;
  }
  if(n.type==='MemberExpression'&&['productId','dongleId'].includes(key(n.property))&&key(n.object.property)==='DeviceInfo'){
   n.object={type:'Identifier',name:'sourceDeviceInfo'};info++;
  }
 });
 assert.equal(calls,3);assert.equal(device,1);assert.equal(info,2);
 // For a physical productId (not dongleId), link-state branches are false.
 // Their actual source function remains recorded, without guessing live state.
 const plain={module:()=>({definitions:new Map()})};
 return canonical(plain,0,clone);
}

function branch(audit,reference){
 const callers=audit.definitionsContaining('.getPollingRate(');
 assert.equal(callers.length,1,'Ambiguous polling getter caller');
 const caller=callers[0],text=audit.source.snippet(caller.id,caller.node);
 assert.equal(caller.node.params[0]?.right?.value,1,'Polling default profile changed');
 for(const part of ['.getUSBHighSpeedPollingRate(','.getProfilePollingRate(','.getPollingRate()','return r.jsonData.pollingRate'])assert(text.includes(part),'Polling caller branch changed');
 audit.record('ordinary/profile polling actual getter and default profile',caller);
 const featureCalls=[];walk(caller.node,n=>{if(n.type==='CallExpression'&&n.callee.type==='SequenceExpression'&&!n.arguments.length){const target=audit.resolve(caller.id,n.callee);if(audit.source.snippet(target.id,target.node).includes('injector.featuresList'))featureCalls.push(target);}});
 assert.equal(featureCalls.length,1,'Feature selector unresolved');
 const hook=featureCalls[0];
 const body=hook.node.body;
 assert.equal(body?.type,'CallExpression');assert.equal(body.callee.object?.name,'Object');assert.equal(key(body.callee.property),'assign');
 assert.equal(body.arguments.length,2);assert.equal(body.arguments[0].type,'ObjectExpression');assert.equal(body.arguments[0].properties.length,0);
 assert.equal(key(body.arguments[1].property),'featuresList');assert.equal(key(body.arguments[1].object.property),'injector');
 audit.record('polling feature selector',hook);
 const registry=audit.resolve(hook.id,body.arguments[1].object.object);
 assert.equal(registry?.node.type,'NewExpression','Feature registry is not a static constructor');
 const initial=audit.resolve(registry.id,registry.node.arguments[0]);
 const initialText=audit.source.snippet(initial.id,initial.node);
 assert(initialText.startsWith('Object.assign(Object.assign({taskRunnerSetDPI:'));
 assert(!initialText.includes('pollingRate')&&!initialText.includes('highSpeedPollingRate'),'Initial feature override changed');
 audit.record('polling initial feature defaults',initial);
 // The derived default's isObm.yes contains macros/brightness/dpi only.
 // injectorDefaults is an absent export in the current cache module, rather
 // than a guessed empty feature object.
 const imported=[];walk(initial.node,n=>{if(n.type==='CallExpression'&&n.callee.type==='SequenceExpression'){const target=audit.resolve(initial.id,n.callee);imported.push(target);}if(n.type==='MemberExpression'&&key(n.property)==='injectorDefaults'){
  const binding=audit.source.module(initial.id).definitions.get(n.object.name);
  assert.equal(binding?.type,'CallExpression');
  const module=binding.arguments[0].value;
  assert.throws(()=>audit.source.exported(module,'injectorDefaults'));
  audit.record('polling cache exports (no injectorDefaults)',{id:module,node:audit.source.module(module).fn});
 }});
 assert.equal(imported.length,1);
 const derivedText=audit.source.snippet(imported[0].id,imported[0].node);
 assert(derivedText.includes('t.isObm={yes:{macros:!0,brightness:!0,dpi:!0}}'));
 assert(!derivedText.includes('pollingRate')&&!derivedText.includes('highSpeedPollingRate'));
 audit.record('polling derived feature defaults',imported[0]);
 const features=audit.features();
 const high=features.filter(f=>f.name==='highSpeedPollingRate');assert(high.length<=1);
 if(high.length){
  assert.equal(high[0].value,'yes');
  const original=reference.definitionsContaining('.getPollingRate(')[0];
  assert.equal(directHighSpeedShape(audit,caller),directHighSpeedShape(reference,original),'High-speed connection selector semantics differ');
  const options=high[0].options?audit.source.literal(0,high[0].options):{};
  assert(options&&typeof options==='object');
  if(options.isHyperpollingDevice===true)return 'high';
  assert(options.isHyperpollingDevice===undefined||options.isHyperpollingDevice===false);
 }
 const obm=features.filter(f=>f.name==='isObm');assert(obm.length<=1,'Multiple isObm registrations');
 if(!obm.length)return 'ordinary';
 assert.equal(obm[0].value,'yes');
 let polling=false;
 if(obm[0].options){
  walk(obm[0].options,n=>{
   if(n.type==='Property'&&key(n.key)==='pollingRate')polling=audit.source.literal(0,n.value);
   if(n.type==='MemberExpression'&&key(n.property)==='supportedFeature'){
    const info=audit.infoField('OBMSpecs');assert.equal(typeof info?.supportedFeature?.pollingRate,'boolean');polling=info.supportedFeature.pollingRate;
   }
  });
 }
 return polling?'profile':'ordinary';
}

function same(audit,query,reference,original,field){
 const shape=(a,q)=>{
  const clone=structuredClone(q[field].node);
  walk(clone,n=>{
   if(field==='helper'&&n.type==='MemberExpression'){
    let target;try{target=a.resolve(q.helper.id,n);}catch{}
    if(target?.id===q.header.id&&target?.node.start===q.header.node.start){
     for(const k of Object.keys(n))delete n[k];
     Object.assign(n,{type:'ArrayExpression',elements:q.command.map(value=>({type:'Literal',value}))});
    }
   }
   if(field==='methodTarget'&&n.type==='CallExpression'&&n.arguments[0]?.type==='ThisExpression'&&['Identifier','MemberExpression'].includes(n.callee.type)&&n.arguments.slice(1).every(v=>v.type==='Identifier')){
    const target=a.resolve(q[field].id,n.callee);assert.equal(target.id,q.helper.id);assert.equal(target.node.start,q.helper.node.start);
    assert.deepEqual(n.arguments.slice(1).map(v=>v.name),q.methodTarget.node.value.params.map(v=>v.name));
    n.callee={type:'Identifier',name:'sourcePollingDelegate'};
   }
   if(field==='helper'&&n.type==='NewExpression'&&n.callee.type==='MemberExpression'){
    const target=a.resolve(q.helper.id,n.callee),text=a.source.snippet(target.id,target.node);
    assert.equal(target.node.superClass?.name,'Error');
    for(const s of ['this.name="rzError"','this.commandClass=','this.commandId=','this.message='])assert(text.includes(s));
    a.record('polling typed input error',target);n.callee={type:'Identifier',name:'sourceInputError'};
   }
  });
  return canonical(a.source,q[field].id,clone);
 };
 assert.equal(shape(audit,query),shape(reference,original),'Different polling '+field+' semantics');
}

function enumCodes(audit,query){
 assert.equal(query.parsers.length,1);const parser=query.parsers[0];let target;
 walk(parser.node,n=>{if(n.type==='MemberExpression'&&key(n.property)==='RATE_1000Hz')target=audit.resolve(parser.id,n.object);});
 assert(target,'Polling enum unresolved');audit.record('ordinary/profile polling byte enum',target);
 const codes=audit.source.literal(target.id,target.node);
 assert.deepEqual(codes,{RATE_1000Hz:1,RATE_500Hz:2,RATE_333Hz:3,RATE_250Hz:4,RATE_200Hz:5,RATE_125Hz:8,RATE_100Hz:10});return codes;
}

function expandQueries(products,gaps,plans,boots){
 const reference=inspect(plans.products.find(p=>p.product_id===182),boots.products.find(p=>p.product_id===182));
 for(const product of products.filter(p=>p.source_class==='rzDevice25DualLinkMouse')){
  try{
   const audit=inspect(plans.products.find(p=>p.product_id===product.product_id),boots.products.find(p=>p.product_id===product.product_id));
   const mode=branch(audit,reference);
   product.polling_physical_product_id=product.device_info.values.productId;
   if(mode==='high'){product.polling_default_hz=8000;continue;}
   const method=mode==='profile'?'getProfilePollingRate':'getPollingRate';
   const query=audit.query(method),original=reference.query(method);
   assert.deepEqual(query.command,mode==='profile'?[2,0,142]:[1,0,133]);
   for(const field of ['methodTarget','helper'])same(audit,query,reference,original,field);
   assert.equal(query.parsers.length,1);assert.equal(canonical(audit.source,query.parsers[0].id,query.parsers[0].node),canonical(reference.source,original.parsers[0].id,original.parsers[0].node));
   assert(!audit.alternateCommands().some(h=>h.command[2]===query.command[2]));
   product.queries=product.queries.filter(q=>q.name!=='polling');
   product.queries.push({name:'polling',method,command:query.command,payload:mode==='profile'?[1]:[],min_response_bytes:mode==='profile'?2:1});
   product.polling_codes=enumCodes(audit,query);product.polling_value_offset=mode==='profile'?1:0;product.polling_default_hz=100;
   product.receipts.push(...audit.receipts);
   for(const row of audit.source.acquisition)if(!product.acquisition.some(r=>r.path===row.path))product.acquisition.push(row);
   for(let i=gaps.length-1;i>=0;i--)if(gaps[i].product_id===product.product_id&&gaps[i].scope==='polling')gaps.splice(i,1);
  }catch(error){gaps.push({product_id:product.product_id,scope:'polling_branch',reason:error.message.split('\n')[0]});}
 }
}

function expandWrites(products,gaps,reads,plans,boots){
 const reference=inspect(plans.products.find(p=>p.product_id===182),boots.products.find(p=>p.product_id===182));
 for(const read of reads.products.filter(p=>p.polling_value_offset!==undefined)){
  try{
   const audit=inspect(plans.products.find(p=>p.product_id===read.product_id),boots.products.find(p=>p.product_id===read.product_id));
   const mode=branch(audit,reference),method=mode==='profile'?'setProfilePollingRate':'setPollingRate';
   const query=audit.query(method),original=reference.query(method);
   assert.deepEqual(query.command,mode==='profile'?[2,0,14]:[1,0,5]);
   for(const field of ['methodTarget','helper'])same(audit,query,reference,original,field);
   assert.equal(query.parsers.length,1);assert.equal(canonical(audit.source,query.parsers[0].id,query.parsers[0].node),canonical(reference.source,original.parsers[0].id,original.parsers[0].node));
   assert.deepEqual(enumCodes(audit,query),read.polling_codes);
   const callers=audit.definitionsContaining('.setPollingRate(');assert.equal(callers.length,1);
   assert.equal(callers[0].node.params[1]?.right?.value,1);
   const text=audit.source.snippet(callers[0].id,callers[0].node);
   for(const s of ['.setUSBHighSpeedPollingRate(','.setProfilePollingRate(','.setPollingRate(e)'])assert(text.includes(s));
   audit.record('ordinary/profile polling actual setter and default profile',callers[0]);
   const tasks=audit.definitionsContaining('params:{pollingRate:').filter(t=>{
    const text=audit.source.snippet(t.id,t.node);
    return text.includes('new Error("data not match")')&&text.includes('.setMemoryStorageItem')&&text.includes('.signal.aborted');
   });
   assert.equal(tasks.length,1,'Polling read/write/readback task unresolved');
   audit.record('polling task read-before-write, readback, cancellation and memory cache',tasks[0]);
   assert(!audit.alternateCommands().some(h=>h.command[2]===query.command[2]));
   let product=products.find(p=>p.product_id===read.product_id);
   if(!product){product={product_id:read.product_id,source_class:read.source_class,min_dpi:read.min_dpi??0,max_dpi:read.max_dpi??0,polling_codes:read.polling_codes,writes:[],receipts:[],acquisition:[]};products.push(product);}
   assert(!product.writes.some(w=>w.kind==='polling'),'Duplicate polling setter');product.polling_codes=read.polling_codes;
   product.writes.push({kind:'polling',method,command:query.command,selector:mode==='profile'?1:0,read_kind:'polling',min_response_bytes:mode==='profile'?2:1});
   product.receipts.push(...audit.receipts);for(const row of audit.source.acquisition)if(!product.acquisition.some(r=>r.path===row.path))product.acquisition.push(row);
  }catch(error){gaps.push({product_id:read.product_id,scope:'polling_branch_write',reason:error.message.split('\n')[0]});}
 }
}
module.exports={expandQueries,expandWrites};
