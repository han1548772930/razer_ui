// Current sleep timer setters, actual task readback and byte encoders.
// Acorn only; no vendor function or hardware execution.
const assert=require('assert');
const {inspect,canonical}=require('./device-query-source.cjs');
const {walk,key}=require('./webpack-source.cjs');

function expand(products,gaps,reads,plans,boots){
 const reference=inspect(plans.products.find(p=>p.product_id===182),boots.products.find(p=>p.product_id===182));
 const original=reference.query('setTimeToSleep');
 assert.deepEqual(original.command,[2,7,3]);
 const originalText=reference.source.snippet(original.helper.id,original.helper.node);
 for(const part of ['void 0===t||isNaN(t)','n[0]=a.E4(t),n[1]=a.l7(t)','return g(r),r'])assert(originalText.includes(part),'Sleep source encoder changed');
 // Minified byte-helper exports and rzError diagnostic template variable
 // spellings differ across current bundles. Normalize only independently
 // verified byte functions and the typed-input-only invalid-value error.
 const shape=(audit,query,field)=>{
  const clone=structuredClone(query[field].node);
  walk(clone,n=>{
   if(n.type==='CallExpression'&&n.callee.type==='MemberExpression'){
    let target;try{target=audit.resolve(query[field].id,n.callee);}catch{return;}
    if(field==='methodTarget'&&target.id===query.helper.id&&target.node.start===query.helper.node.start){
     assert.equal(n.arguments.length,2);assert.equal(n.arguments[0].type,'ThisExpression');
     assert.equal(n.arguments[1].name,query.methodTarget.node.value.params[0].name);
     n.callee={type:'Identifier',name:'sourceSleepDelegate'};
    }else if(field==='helper'){
     const text=audit.source.snippet(target.id,target.node);
     if(text==='e=>e>>8&255'||text==='e=>255&e')n.callee={type:'Identifier',name:text==='e=>e>>8&255'?'sourceHighByte':'sourceLowByte'};
    }
   }
   if(field==='helper'&&n.type==='NewExpression'&&n.callee.type==='MemberExpression'){
    const target=audit.resolve(query.helper.id,n.callee),text=audit.source.snippet(target.id,target.node);
    assert.equal(target.node.superClass?.name,'Error');
    for(const part of ['this.name="rzError"','this.commandClass=','this.commandId=','this.message='])assert(text.includes(part),'Changed native input error fields');
    audit.record('idle invalid input error class (diagnostic spellings differ)',target);
    n.callee={type:'Identifier',name:'sourceInputError'};
   }
  });
  return canonical(audit.source,query[field].id,clone);
 };
 for(const read of reads.products.filter(p=>p.queries.some(q=>q.name==='idle'))){
  try{
   const audit=inspect(plans.products.find(p=>p.product_id===read.product_id),boots.products.find(p=>p.product_id===read.product_id));
   const candidate=audit.query('setTimeToSleep');
   assert.deepEqual(candidate.command,original.command);
   for(const field of ['methodTarget','helper'])assert.equal(shape(audit,candidate,field),shape(reference,original,field),'Sleep setter '+field+' differs');
   assert.equal(candidate.parsers.length,1);
   assert.equal(canonical(audit.source,candidate.parsers[0].id,candidate.parsers[0].node),canonical(reference.source,original.parsers[0].id,original.parsers[0].node),'Sleep response parser differs');
   const encoders=[];walk(candidate.helper.node,n=>{
    if(n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&n.arguments.length===1){
     try{const target=audit.resolve(candidate.helper.id,n.callee),text=audit.source.snippet(target.id,target.node);
      if(['e=>e>>8&255','e=>255&e'].includes(text)){audit.record('idle big endian byte encoder',target);encoders.push(text);}
     }catch{}
    }
   });
   assert.deepEqual(encoders,['e=>e>>8&255','e=>255&e']);
   const callers=audit.definitionsContaining('.setTimeToSleep(').filter(t=>{
    const text=audit.source.snippet(t.id,t.node);
    return text.includes('.getTimeToSleep()')&&text.includes('new Error("data not match")')&&text.includes('60*')&&text.includes('singleProfileDevice');
   });
   assert.equal(callers.length,1,'Ambiguous sleep timer task');
   audit.record('idle actual write/readback and product units',callers[0]);
   assert(!audit.alternateCommands().some(h=>h.command[2]===candidate.command[2]),'Sleep setter alternate transaction unresolved');
   let product=products.find(p=>p.product_id===read.product_id);
   if(!product){product={product_id:read.product_id,source_class:read.source_class,min_dpi:read.min_dpi??0,max_dpi:read.max_dpi??0,polling_codes:read.polling_codes,writes:[],receipts:[],acquisition:[]};products.push(product);}
   product.writes.push({kind:'idle',method:'setTimeToSleep',command:candidate.command,selector:0,read_kind:'idle',min_response_bytes:2});
   product.receipts.push(...audit.receipts);
   for(const acquisition of audit.source.acquisition)if(!product.acquisition.some(a=>a.path===acquisition.path))product.acquisition.push(acquisition);
  }catch(error){gaps.push({product_id:read.product_id,scope:'idle',reason:error.message.split('\n')[0]});}
 }
}
module.exports={expand};
