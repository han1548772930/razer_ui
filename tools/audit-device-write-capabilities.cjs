// Current Razer setters and their callers, parsed as data. Never evaluates vendor JS.
const fs=require('fs'),path=require('path'),assert=require('assert');
const {CurrentMiddlewareSource}=require('./current-middleware-source.cjs');
const {walk,key,hash}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..'),check=process.argv.includes('--check');
const readPath='docs/re/mouse-read-capabilities-current-evidence.json';
const reads=JSON.parse(fs.readFileSync(path.join(root,readPath),'utf8'));
const plans=JSON.parse(fs.readFileSync(path.join(root,'.ref/middleware/receiver-protocol-requests.json'),'utf8'));
const products=[];
for(const read of reads.products.filter(p=>p.product_id===182)){
 const source=new CurrentMiddlewareSource(read.product_id);
 const plan=plans.products.find(p=>p.product_id===read.product_id);
 const receipts=[];
 const resolve=(id,node)=>{
  const seen=new Set();
  for(;;){assert(node,'Unresolved node');const token=id+':'+node.start;assert(!seen.has(token),'Circular binding');seen.add(token);
   if(node.type==='Identifier'){node=source.binding(id,node.name);continue;}
   if(node.type==='SequenceExpression'){node=node.expressions.at(-1);continue;}
   if(node.type==='MemberExpression'&&node.object.type==='Identifier'){
    const imported=source.binding(id,node.object.name);
    if(imported.type==='CallExpression'&&Number.isInteger(imported.arguments[0]?.value)){id=imported.arguments[0].value;node=source.exported(id,key(node.property));continue;}
   }
   return{id,node};
  }
 };
 const record=(label,target)=>{const receipt={label,module:target.id,...source.receipt(target.id,target.node)};receipts.push(receipt);return receipt;};
 // Revalidate every original read/factory receipt used to select the product.
 for(const receipt of [...read.receipts,read.factory,read.device_info.receipt,read.bootstrap]){
  const text=source.text(receipt.path);assert.equal(hash(text),receipt.sha256);assert.equal(text.slice(receipt.offset,receipt.end),receipt.source);
 }
 const chain=[];let target=resolve(plan.class_module,source.exported(plan.class_module,'default'));
 for(let depth=0;depth<8;depth++){assert(['ClassDeclaration','ClassExpression'].includes(target.node.type));chain.push(target);
  if(!target.node.superClass)break;target=resolve(target.id,target.node.superClass);}
 const writes=[];
 for(const [kind,name,selector] of [['dpi','setDpiLevel',0],['polling','setUSBHighSpeedPollingRate',1]]){
  const owner=chain.find(c=>c.node.body.body.some(m=>key(m.key)===name));assert(owner,'Setter not found');
  const method=owner.node.body.body.find(m=>key(m.key)===name);record(kind+' method',{id:owner.id,node:method});
  const delegates=[];walk(method,n=>{if(n.type==='CallExpression'&&n.arguments[0]?.type==='ThisExpression'&&n.arguments[1]?.type==='Identifier')delegates.push(n);});
  assert.equal(delegates.length,1,'Ambiguous setter delegate');
  const helper=resolve(owner.id,delegates[0].callee);const proof=record(kind+' payload builder',helper);
  const sends=[];walk(helper.node,n=>{if(n.type==='CallExpression'&&key(n.callee.property)==='sendCommand')sends.push(n);});
  assert.equal(sends.length,1,'Ambiguous setter sendCommand');
  const header=resolve(helper.id,sends[0].arguments[0]);record(kind+' command',header);
  assert.equal(header.node.type,'NewExpression');assert.equal(header.node.callee.name,'Uint8Array');
  const command=source.literal(header.id,header.node.arguments[0]);
  const alternate=read.receipts.filter(r=>r.label.startsWith('alternate transaction command '));
  assert.equal(alternate.length,2,'Missing alternate transaction proof');
  for(const receipt of alternate){
   const node=source.exported(receipt.module,receipt.label.endsWith(' z$')?'z$':'Z');
   const reserved=resolve(receipt.module,node);
   assert.equal(reserved.node.type,'NewExpression');
   const special=source.literal(reserved.id,reserved.node.arguments[0]);
   assert.notEqual(command[2],special[2],'Write needs alternate transaction flag');
  }
  if(kind==='dpi'){
   assert.deepEqual(command,[7,4,5]);
   for(const part of ['s=0','c[0]=t','c[1]=a.E4(n)','c[2]=a.l7(n)','c[3]=a.E4(o)','c[4]=a.l7(o)','c[5]=a.E4(s)','c[6]=a.l7(s)','return _(d),d'])assert(proof.source.includes(part),part);
   const parser=record('dpi setter parser',{id:helper.id,node:source.binding(helper.id,'_')});
   assert.equal(parser.source,'e=>{e.jsonData={classId:e.data[0],dpiX:a.Jz(e.data[1],e.data[2]),dpiY:a.Jz(e.data[3],e.data[4]),dpiZ:a.Jz(e.data[5],e.data[6])}}');
   for(const name of ['E4','l7']){
    const imported=source.binding(helper.id,'a');const byte=resolve(imported.arguments[0].value,source.exported(imported.arguments[0].value,name));
    const receipt=record('dpi '+name+' encoder',byte);
    assert.equal(receipt.source,name==='E4'?'e=>e>>8&255':'e=>255&e');
   }
   const caller=record('dpi read/write/readback caller',{id:85191,node:source.binding(85191,'et')});
   for(const part of ['yield r.getDpiLevel(0)','yield r.setDpiLevel(0,o,i)','u=o===e.jsonData.dpiX&&i===e.jsonData.dpiY','new Error("data not match")'])assert(caller.source.includes(part),part);
  }else{
   assert.deepEqual(command,[2,0,64]);
   for(const part of ['a[0]=t,a[1]=s','case 8e3:s=r.Mt.RATE_8000Hz','case 125:s=r.Mt.RATE_125Hz','default:throw','return $(l),l'])assert(proof.source.includes(part),part);
   record('polling setter parser',{id:helper.id,node:source.binding(helper.id,'$')});
   const caller=[];walk(source.module(87887).fn,n=>{
    if(n.type==='ArrowFunctionExpression'){const r=source.receipt(87887,n);if(r.source.includes('.setUSBHighSpeedPollingRate(')&&r.source.length<1000)caller.push(n);}
   });assert.equal(caller.length,1);const receipt=record('polling profile/default caller',{id:87887,node:caller[0]});
   assert(receipt.source.startsWith('(e,t=1)=>'));assert(receipt.source.includes('(0,m.g2)()?yield i.setUSBHighSpeedPollingRate(t,e)'));
  }
  // Existing getter decoders are also valid for the setter's explicitly proved parser.
  writes.push({kind,method:name,command,selector,read_kind:kind,min_response_bytes:kind==='dpi'?7:2});
 }
 products.push({product_id:read.product_id,source_class:read.source_class,min_dpi:read.min_dpi,max_dpi:read.max_dpi,
  polling_codes:read.polling_codes,writes,receipts,acquisition:source.acquisition});
}
const boots=JSON.parse(fs.readFileSync(path.join(root,'.ref/middleware/bootstrap-requests.json'),'utf8'));
const expansion=require('./expand-device-write-capabilities.cjs').expand(reads,plans,boots);
products.push(...expansion.products);
require('./expand-idle-write-capabilities.cjs').expand(products,expansion.gaps,reads,plans,boots);
products.sort((a,b)=>a.product_id-b.product_id);
const evidence={schema_version:1,method:'Manifest-verified current factories, individual setter payload/parser/transport and real caller gates; no vendor execution',
 read_evidence:{path:readPath,sha256:hash(fs.readFileSync(path.join(root,readPath)))},products,gaps:expansion.gaps,
 policy:{polling_readback:'Application confirmation policy; current setter caller does not itself prove a readback',
  idle_units:'Raw u16 timeToSleep; current task multiplies UI value by 60 unless DeviceInfo.singleProfileDevice. UI conversion and persistent task cache remain separate consumer work.',
  scope:'Only these product bindings. No mapping, persistent DPI stage table, mode switch, pairing, BLE or firmware write inferred.'}};
const runtime={schema_version:1,products:products.map(({receipts,acquisition,...p})=>p)};
for(const [relative,data] of [['docs/re/device-write-capabilities-current-evidence.json',evidence],['assets/data/device-write-capabilities.json',runtime]]){
 const encoded=JSON.stringify(data,null,2)+'\n',file=path.join(root,relative);
 if(check)assert.equal(fs.readFileSync(file,'utf8'),encoded,'Stale '+relative);else fs.writeFileSync(file,encoded);
}
console.log(JSON.stringify({products:products.length,writes:products.reduce((n,p)=>n+p.writes.length,0),receipts:products.reduce((n,p)=>n+p.receipts.length,0)}));
