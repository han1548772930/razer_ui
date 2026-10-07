// Derive query bindings from every selected current dual-link factory, resolving
// each class, inherited query, command bytes, transaction namespace and timing.
// Static Acorn data only: downloaded code is never called.
const fs=require('fs'),path=require('path');
const {CurrentMiddlewareSource}=require('./current-middleware-source.cjs');
const {walk,key}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..');
const read=f=>JSON.parse(fs.readFileSync(path.join(root,f),'utf8'));
const plan=read('.ref/middleware/receiver-protocol-requests.json');
const boots=read('.ref/middleware/bootstrap-requests.json').products;
const evidencePath='docs/re/receiver-catalog-current-evidence.json';
const nodes=(n,p)=>{const a=[];walk(n,x=>{if(p(x))a.push(x)});return a};
const only=(a,n)=>{if(a.length!==1)throw Error(`${n}: ${a.length} candidates`);return a[0]};
const ensure=(v,s)=>{if(!v)throw Error(s)};
// Explicitly reviewed startup roots. Other products retain no startup policy
// until their caller (not just their shared protocol class) is audited.
const startupAudits=new Map([[179,{module:34340,entry:'Ne',counter:'be'}]]);
const capabilities=[],products=[];
for(const planned of plan.products){
  const row={product_id:planned.product_id,status:planned.status,receipts:[]};
  if(planned.status!=='selected_factory_branch'){products.push(row);continue}
  try {
    const source=new CurrentMiddlewareSource(planned.product_id);
    for(const f of [source.mainFile,...boots.find(p=>p.product_id===planned.product_id).files,...planned.files])source.parse(f);
    const record=(id,label,node)=>{row.receipts.push({label,module:id,...source.receipt(id,node)});return node};
    const resolve=(id,node)=>{
      const seen=new Set();
      for(;;){
        const token=`${id}:${node.start}`;ensure(!seen.has(token),'Circular source alias');seen.add(token);
        if(node.type==='Identifier'){node=source.binding(id,node.name);continue}
        if(node.type==='MemberExpression'&&node.object.type==='Identifier'){
          const imp=source.binding(id,node.object.name);
          if(imp.type==='CallExpression'&&imp.callee.name===source.module(id).fn.params[2]?.name&&Number.isInteger(imp.arguments[0]?.value)){
            id=imp.arguments[0].value;node=source.exported(id,key(node.property));continue;
          }
        }
        return {id,node};
      }
    };
    const chain=[];
    let target=resolve(planned.class_module,source.exported(planned.class_module,'default'));
    for(let depth=0;depth<8;depth++){
      ensure(['ClassDeclaration','ClassExpression'].includes(target.node.type),'Selected import is not a class');
      chain.push(target);
      record(target.id,'class inheritance',target.node.superClass||target.node);
      if(target.node.body.body.some(m=>key(m.key)==='getMultipleDeviceWirelessConnectionStatusV2'))break;
      ensure(target.node.superClass,'Missing inherited query');
      target=resolve(target.id,target.node.superClass);
    }
    const owner=name=>only(chain.filter(c=>c.node.body.body.some(m=>key(m.key)===name)).slice(0,1),name+' owner');
    const method=name=>{const c=owner(name);return {id:c.id,node:record(c.id,name,only(c.node.body.body.filter(m=>key(m.key)===name),name))}};
    const query=method('getMultipleDeviceWirelessConnectionStatusV2');
    const invocation=only(nodes(query.node,n=>n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&n.arguments.length===1&&n.arguments[0].type==='ThisExpression'),'V2 delegate');
    const helper=resolve(query.id,invocation.callee);record(helper.id,'V2 parser',helper.node);
    const send=only(nodes(helper.node,n=>n.type==='CallExpression'&&key(n.callee.property)==='sendCommand'),'V2 command send');
    ensure(send.arguments.length===2,'V2 gained a payload argument');
    const header=resolve(helper.id,send.arguments[0]);record(header.id,'V2 command bytes',header.node);
    ensure(header.node.type==='NewExpression'&&header.node.callee.name==='Uint8Array','Command is not bytes');
    const command=source.literal(header.id,header.node.arguments[0]);
    ensure(JSON.stringify(command)==='[80,0,191]','Unknown wireless status command');
    const parserText=source.snippet(helper.id,helper.node);
    ensure(nodes(helper.node,n=>n.type==='AssignmentExpression'&&n.operator==='+='&&n.right.value===3).length===1,'V2 record stride changed');
    ensure(parserText.includes('deviceCount:0,status:[]')&&parserText.includes('.jsonData.status.push({productId:'),'V2 output fields changed');
    const endianCall=only(nodes(helper.node,n=>n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&
      n.arguments.length===2&&n.arguments.every(a=>a.type==='MemberExpression'&&a.computed&&a.property.type==='BinaryExpression'&&a.property.operator==='+')),'V2 PID byte pair');
    ensure(endianCall.arguments[0].property.right.value===1&&endianCall.arguments[1].property.right.value===2,'V2 PID byte offsets changed');
    const endian=resolve(helper.id,endianCall.callee);record(endian.id,'PID endian',endian.node);
    const masked=(n,param)=>n?.type==='BinaryExpression'&&n.operator==='&'&&n.left.value===255&&n.right.name===param;
    const endianBody=endian.node.body,endianParams=endian.node.params;
    ensure(endianParams.length===2&&endianBody.type==='BinaryExpression'&&endianBody.operator==='|'&&
      endianBody.left.operator==='<<'&&endianBody.left.right.value===8&&
      masked(endianBody.left.left,endianParams[0].name)&&masked(endianBody.right,endianParams[1].name),'PID endian changed');
    const transaction=method('_getTransactionId');
    const wrap=only(nodes(transaction.node,n=>n.type==='BinaryExpression'&&n.operator==='==='&&n.left.type==='Literal'),'transaction wrap').left.value;
    const prefixNodes=nodes(transaction.node,n=>n.type==='BinaryExpression'&&n.operator==='|');
    const prefix=prefixNodes.length?source.literal(transaction.id,only(prefixNodes,'transaction namespace').left):0;
    ensure(wrap===31&&[0,128,224].includes(prefix),'Unknown transaction namespace');
    // The selected subclass may reserve primary-device commands. Prove the
    // actual V2 header is outside those bit override exceptions.
    const sender=method('sendCommand');
    const overrides=nodes(sender.node,n=>n.type==='AssignmentExpression'&&['|=','&='].includes(n.operator));
    for(const override of overrides){
      const guard=only(nodes(sender.node,n=>n.type==='LogicalExpression'&&n.operator==='||'&&n.right===override),'transaction exception guard').left;
      const comparisons=nodes(guard,n=>n.type==='BinaryExpression'&&n.operator==='!==');ensure(comparisons.length===2,'Unknown command exception');
      for(const c of comparisons){
        ensure(c.right.type==='MemberExpression'&&c.right.computed&&c.right.property.value===2,'Unknown command comparison');
        const exception=resolve(sender.id,c.right.object);record(exception.id,'transaction exception header',exception.node);
        ensure(exception.node.type==='NewExpression'&&source.literal(exception.id,exception.node.arguments[0])[2]!==command[2],'V2 needs an extra transaction override');
      }
    }
    const base=chain.at(-1),ctor=only(base.node.body.body.filter(m=>key(m.key)==='constructor'),'base constructor');
    record(base.id,'protocol constructor defaults',ctor);
    const report=only(nodes(ctor,n=>n.type==='AssignmentExpression'&&n.left.object?.type==='ThisExpression'&&key(n.left.property)==='reportLength'),'report length');
    const reportBytes=source.literal(base.id,report.right),parameters=ctor.value.params;
    ensure(parameters.length===9&&parameters[8].type==='AssignmentPattern','Protocol constructor signature changed');
    const reportId=source.literal(base.id,only(parameters[8].right.properties.filter(p=>key(p.key)==='reportId'),'report ID').value);
    ensure(reportBytes===91&&reportId===0,'Unknown report envelope');
    const maxIn=source.literal(base.id,parameters[6].right),maxOut=source.literal(base.id,parameters[7].right);
    const factory=source.binding(planned.factory.module,only([...source.module(planned.factory.module).definitions].filter(([,n])=>n?.start===planned.factory.offset),'factory binding')[0]);
    const blocks=nodes(factory,n=>n.type==='BlockStatement'&&source.snippet(planned.factory.module,n).includes('.'+planned.selected_class+'()'));
    const selected=blocks.sort((a,b)=>(a.end-a.start)-(b.end-b.start))[0];ensure(selected,'Constructor branch absent');
    const constructor=only(nodes(selected,n=>n.type==='NewExpression'&&n.arguments.length===6),'selected constructor');record(planned.factory.module,'selected constructor',constructor);
    const timing=index=>{
      const arg=constructor.arguments[index];ensure(arg.type==='BinaryExpression'&&arg.operator==='*','Factory timing scale changed');
      return source.literal(planned.factory.module,arg.left)*source.literal(planned.factory.module,arg.right);
    };
    const consumers=[];
    for(const[id,m]of source.modules)for(const[name,n]of m.definitions){
      if(!n||!['ArrowFunctionExpression','FunctionExpression'].includes(n.type))continue;
      const text=source.snippet(id,n);
      if(text.includes('.getMultipleDeviceWirelessConnectionStatusV2()')&&text.includes('.dongleId==='))consumers.push({id,node:n});
    }
    const consumer=only(consumers,'source wireless-list consumer');record(consumer.id,'source wireless-list consumer',consumer.node);
    const peerProductMatch=/\.dongleId===\w+\.productId\|\|\w+\.productId===\w+\.productId/.test(source.snippet(consumer.id,consumer.node));
    let startupRetry;
    const startupAudit=startupAudits.get(planned.product_id);
    if(startupAudit){
      const {module,entry,counter}=startupAudit;
      const startup=record(module,'startup discovery retry',source.binding(module,entry));
      const initial=record(module,'startup retry counter',source.binding(module,counter));
      ensure(source.literal(module,initial)===0,'Startup counter changed');
      const consumerName=only([...source.module(consumer.id).definitions].filter(([,n])=>n===consumer.node),'consumer name')[0];
      ensure(module===consumer.id&&nodes(startup,n=>n.type==='CallExpression'&&n.callee.name===consumerName).length===1,'Startup query changed');
      const limit=only(nodes(startup,n=>n.type==='BinaryExpression'&&n.operator==='<'&&n.left.name===counter),'startup retry limit');
      const count=source.literal(module,limit.right);
      ensure(count===5,'Startup retry count changed');
      ensure(nodes(startup,n=>n.type==='UpdateExpression'&&n.operator==='++'&&n.argument.name===counter).length===1,'Startup increment changed');
      const timers=nodes(startup,n=>n.type==='CallExpression'&&n.callee.name==='setTimeout');
      const delayed=only(timers.filter(n=>n.arguments[1]?.type==='BinaryExpression'),'startup status delay');
      const delay=delayed.arguments[1];
      ensure(delay.operator==='*'&&delay.left.value===200&&delay.right.operator==='+'&&delay.right.left.name===counter&&delay.right.right.value===1,'Startup delay changed');
      const errorTimer=only(timers.filter(n=>n.arguments[0]?.name===entry),'startup error timer');
      const status=only(nodes(startup,n=>n.type==='BinaryExpression'&&n.operator==='==='&&n.left.value===0&&source.snippet(module,n.right).includes('.status')),'startup disconnected status');
      ensure(source.snippet(module,status.right).includes('[0]'),'Startup no longer checks first peer');
      const text=source.snippet(module,startup);
      ensure(text.includes('serialNumber')&&text.includes('""==='),'Startup serial readiness condition changed');
      // Match he()'s self/sentinel filtering before selecting the first peer.
      const consumerText=source.snippet(consumer.id,consumer.node);
      ensure(/65535!==\w+\.productId&&\w+\.productId!==\w+\.DeviceInfo\.dongleId/.test(consumerText),'Startup peer filtering changed');
      startupRetry={first_peer_status:status.left.value,delays_ms:Array.from({length:count},(_,index)=>delay.left.value*(index+2))};
      row.startup_retry_scope={implemented:'Bounded first-peer status requery only',
        query_error_retry_ms:source.literal(module,errorTimer.arguments[1]),
        remaining:'Owner-cancelled error retries and runtime serial readiness are not implemented'};
      ensure(row.startup_retry_scope.query_error_retry_ms===1000,'Startup error interval changed');
    }
    // Keep the manufacturer-provided interface map keyed by real PID.
    const info=planned.device_info.values,claim=typeof info.claimInterface==='number'?info.claimInterface:info.claimInterface?.[info.dongleId];
    ensure(Number.isInteger(claim)&&claim>=0&&claim<=255,'Unresolved per-connection interface');
    for(const name of ['_createDataSend','_calculateChecksum','_getUSBTransferInResult'])method(name);
    const capability={source_product_id:planned.product_id,vendor_id:info.vendorId,product_id:info.dongleId,
      claim_interface:claim,report_bytes:reportBytes,report_id:reportId,protocol:'razer_device25_wireless_status_v2',command,
      transaction_prefix:prefix,transaction_modulus:wrap,max_retry_in:maxIn,max_retry_out:maxOut,
      sleep_between_out_ms:timing(3),sleep_between_out_in_ms:timing(4),sleep_between_in_ms:timing(5),
      peer_match_product_id:peerProductMatch,...(startupRetry?{startup_retry:startupRetry}:{}),
      source_class:planned.selected_class,evidence_path:evidencePath};
    capabilities.push(capability);row.status='query_binding_verified';row.capability=capability;row.acquisition=source.acquisition;
    row.feature=planned.feature;row.device_info=planned.device_info;row.factory=planned.factory;row.loader=planned.loader;
  } catch(error){row.status='unresolved_query_binding';row.reason=error.message;}
  products.push(row);
}
const duplicates=capabilities.filter((a,i)=>capabilities.slice(0,i).some(b=>a.vendor_id===b.vendor_id&&a.product_id===b.product_id));
ensure(!duplicates.length,'Ambiguous receiver transport bindings');
const counts={};for(const p of products)counts[p.status]=(counts[p.status]||0)+1;
const payload={schema_version:1,scope:'Every direct dual-link receiver selected from the full current middleware inventory; each binding independently resolves factory, protocol, query and transport parameters.',capabilities};
const evidence={method:'Static per-product AST and HTTP SHA-256 receipts; no vendor execution; runtime acceptance is not claimed',scope_products:plan.scope_products,counts,products};
for(const[file,value]of [[evidencePath,evidence],['assets/data/receiver-query-capabilities.json',payload]]){
  if(file.startsWith('assets/'))ensure(!products.some(p=>p.status==='unresolved_query_binding'),'Do not publish an incomplete replacement capability catalog');
  const text=JSON.stringify(value,null,2)+'\n',dest=path.join(root,file);
  if(process.argv.includes('--check'))ensure(fs.readFileSync(dest,'utf8')===text,'Stale '+file);
  else fs.writeFileSync(dest,text);
}
console.log(JSON.stringify({counts,unresolved:products.filter(p=>p.status==='unresolved_query_binding').map(p=>({product_id:p.product_id,reason:p.reason}))}));
