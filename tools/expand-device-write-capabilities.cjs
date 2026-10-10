// Source-proved direct setters for individually gated product query bindings.
const assert=require('assert');
const {inspect,canonical}=require('./device-query-source.cjs');
const {walk,key,hash}=require('./webpack-source.cjs');
function expand(reads,plans,boots){
 const products=[],gaps=[],ref=inspect(plans.products.find(p=>p.product_id===182),boots.products.find(p=>p.product_id===182));
 const original=new Map();
 // The reference write methods have product-specific selector arguments and
 // are therefore resolved only when a product has an actual matching setter.
 for(const read of reads.products){
  if(read.product_id===182)continue;
  const plan=plans.products.find(p=>p.product_id===read.product_id),boot=boots.products.find(p=>p.product_id===read.product_id),audit=inspect(plan,boot);
  for(const receipt of [...read.receipts,read.factory,read.device_info.receipt,read.bootstrap]){const text=audit.source.text(receipt.path);assert.equal(hash(text),receipt.sha256);assert.equal(text.slice(receipt.offset,receipt.end),receipt.source);}
  const alternate=audit.alternateCommands(),writes=[];
  for(const [kind,name,selector]of [['dpi','setDpiLevel',0],['polling','setUSBHighSpeedPollingRate',1]]){
   if(!read.queries.some(q=>q.name===kind))continue;
   if(kind==='polling'&&!read.queries.some(q=>q.method==='getUSBHighSpeedPollingRate'))continue;
   try{
    const candidate=audit.query(name),reference=original.get(name)||ref.query(name);original.set(name,reference);
    assert.equal(canonical(audit.source,candidate.methodTarget.id,candidate.methodTarget.node),canonical(ref.source,reference.methodTarget.id,reference.methodTarget.node),'Setter interface differs');
    assert.equal(canonical(audit.source,candidate.helper.id,candidate.helper.node),canonical(ref.source,reference.helper.id,reference.helper.node),'Setter payload builder differs');
    assert.deepEqual(candidate.command,reference.command,'Setter command differs');assert.equal(candidate.parsers.length,reference.parsers.length);
    for(let i=0;i<candidate.parsers.length;i++)assert.equal(canonical(audit.source,candidate.parsers[i].id,candidate.parsers[i].node),canonical(ref.source,reference.parsers[i].id,reference.parsers[i].node),'Setter parser differs');
    assert(!alternate.some(h=>h.command[2]===candidate.command[2]),'Setter requires alternate transaction');
    if(kind==='dpi'){
     assert(Number.isInteger(read.min_dpi)&&Number.isInteger(read.max_dpi)&&read.min_dpi>0&&read.min_dpi<=read.max_dpi,'Unresolved current product DPI edit limits');
     const caller=audit.definitionsContaining('.getDpiLevel(0)').find(t=>audit.source.snippet(t.id,t.node).includes('.setDpiLevel(0,'));assert(caller,'Missing DPI action caller');
     const calls=[],comparisons=[];walk(caller.node,n=>{
      if(n.type==='CallExpression'&&['getDpiLevel','setDpiLevel'].includes(key(n.callee.property)))calls.push(n);
      if(n.type==='BinaryExpression'&&n.operator==='===')comparisons.push(n);
     });
     assert.equal(calls.filter(c=>key(c.callee.property)==='getDpiLevel'&&c.arguments[0]?.value===0).length,2,'Missing DPI read-before-write/readback');
     const setters=calls.filter(c=>key(c.callee.property)==='setDpiLevel');assert.equal(setters.length,1);
     assert.equal(setters[0].arguments[0].value,0);assert.equal(setters[0].arguments[1].name,caller.node.params[3].name);assert.equal(setters[0].arguments[2].name,caller.node.params[4].name);assert.equal(setters[0].arguments.length,3,'DPI Z default changed');
     for(const [property,param]of [['dpiX',3],['dpiY',4]])assert(comparisons.some(c=>[c.left,c.right].some(n=>n.type==='MemberExpression'&&key(n.property)===property)&&[c.left,c.right].some(n=>n.type==='Identifier'&&n.name===caller.node.params[param].name)),'Missing source DPI readback equality');
     assert(audit.source.snippet(caller.id,caller.node).includes('new Error("data not match")'));audit.record('dpi read/write/readback caller',caller);
    }else{
     const caller=audit.definitionsContaining('.setUSBHighSpeedPollingRate(').find(t=>t.node.params[1]?.type==='AssignmentPattern'&&t.node.params[1].right.value===1);assert(caller,'Missing source polling profile/default caller');
     const setters=[];walk(caller.node,n=>{if(n.type==='CallExpression'&&key(n.callee.property)==='setUSBHighSpeedPollingRate')setters.push(n);});assert.equal(setters.length,1);
     assert.equal(setters[0].arguments[0].name,caller.node.params[1].left.name);assert.equal(setters[0].arguments[1].name,caller.node.params[0].name);audit.record('polling profile/default caller',caller);
    }
    writes.push({kind,method:name,command:candidate.command,selector,read_kind:kind,min_response_bytes:kind==='dpi'?7:2});
   }catch(error){gaps.push({product_id:read.product_id,scope:kind,reason:error.message.split('\n')[0]});}
  }
  if(writes.length)products.push({product_id:read.product_id,source_class:read.source_class,min_dpi:read.min_dpi??0,max_dpi:read.max_dpi??0,polling_codes:read.polling_codes,writes,receipts:audit.receipts,acquisition:audit.source.acquisition});
 }
 return{products,gaps};
}
module.exports={expand};
