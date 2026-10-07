// Derive exact receiver bindings from current product middleware as Acorn data.
// No downloaded module is imported, evaluated, or executed.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {MiddlewareSource} = require('./middleware-source.cjs');
const {walk, key, hash} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..'), receipts = [], acquisitions = [];
const evidencePath = 'docs/re/receiver-capabilities-current-evidence.json';
function requireThat(value, message) { if (!value) throw Error(message); return value; }
function nodes(root, predicate) { const result=[]; walk(root,n=>{if(predicate(n))result.push(n)}); return result; }
function only(values, label) { requireThat(values.length===1,`${label}: expected one, got ${values.length}`); return values[0]; }
const configurations = [
  {product:179,boot:6120,bootTail:4551,info:21503,factory:96204,factoryName:'j',factoryLoader:'W',feature:34340,featureExport:'Kj',
    chain:[{module:11502,name:'o'},{module:91818,name:'h'},{module:7755,name:'dc'}],loader:'rzDevice25LinkerUma',
    className:'rzDevice25LinkerUma',kind:'LINKER',helper:84816,helperExport:'VO',headerModule:30580,headerExport:'IZ',
    endianModule:52975,endianExport:'U1',timings:35284},
  {product:226,boot:7846,bootTail:1364,info:3298,factory:87887,factoryName:'F',factoryLoader:'G',feature:66825,featureExport:'R4',
    chain:[{module:48957,name:'f'},{module:87969,name:'il'}],loader:'rzDevice25DualLinkMouse',
    className:'rzDevice25DualLinkMouse',kind:'MOUSE',helper:12844,helperExport:'PE',headerModule:14397,headerExport:'KY',
    endianModule:76912,endianExport:'Jz',timings:68560},
];
const capabilities = configurations.map(c=>{
  const mw=new MiddlewareSource(c.product); acquisitions.push(...mw.acquisition);
  const mainFile=only(mw.files.filter(f=>/\/main\./.test(f)),'main'), main=mw.text(mainFile);
  const mainAst=acorn.parse(main,{ecmaVersion:'latest'});
  const boot=only(nodes(mainAst,n=>n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&
    n.callee.object.name==='Promise'&&key(n.callee.property)==='all'&&
    n.arguments[0]?.elements?.some(e=>e.type==='CallExpression'&&e.arguments[0]?.value===c.boot)&&
    n.arguments[0]?.elements?.some(e=>e.type==='CallExpression'&&e.arguments[0]?.value===c.bootTail)),'boot chunks');
  receipts.push({source_product_id:c.product,label:'main boot chunks',path:mainFile,sha256:hash(main),offset:boot.start,end:boot.end,source:main.slice(boot.start,boot.end)});
  mw.parse(only(mw.files.filter(f=>new RegExp('/'+c.boot+'\\.').test(f)),'boot file')); mw.parse(mainFile);
  function record(module,label,node) { receipts.push({source_product_id:c.product,label,module,...mw.receipt(module,node)}); return node; }
  function exported(module,name) {let n=mw.exported(module,name);n=n.type==='Identifier'?mw.binding(module,n.name):n;return record(module,name,n);}
  const info=exported(c.info,'DeviceInfo');
  const property=(object,name)=>only(object.properties.filter(p=>key(p.key)===name),name).value;
  const infoValue=name=>mw.literal(c.info,property(info,name));
  const feature=only(nodes(mainAst,n=>n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&
    key(n.callee.property)==='useFeature'&&n.arguments[0]?.value==='rzDeviceType'&&n.arguments[1]?.value==='rzDevice25'),'product feature');
  receipts.push({source_product_id:c.product,label:'product rzDeviceType feature',path:mainFile,sha256:hash(main),offset:feature.start,end:feature.end,source:main.slice(feature.start,feature.end)});
  const featureObject=feature.arguments[2];
  requireThat(mw.literal(c.info,property(featureObject,'isDualLinkDevice'))===true,'Not a dual-link product');
  const declaredKind=featureObject.properties.find(p=>key(p.key)==='duallinkDeviceType');
  requireThat((declaredKind?mw.literal(c.info,declaredKind.value):'MOUSE')===c.kind,'Unexpected dual-link kind');
  const featurePredicate=exported(c.feature,c.featureExport);
  requireThat(mw.snippet(c.feature,featurePredicate).includes('.isDualLinkDevice'),'Missing dual-link predicate');
  const factory=record(c.factory,'receiver class selection',mw.binding(c.factory,c.factoryName));
  const factoryText=mw.snippet(c.factory,factory);
  requireThat(factoryText.includes('DeviceInfo.dongleId')&&factoryText.includes('DeviceInfo.claimInterface'),'Missing DeviceInfo routing');
  requireThat(factoryText.includes('"MOUSE"')&&factoryText.includes('duallinkDeviceType'),'Missing default kind routing');
  const blocks=nodes(factory,n=>n.type==='BlockStatement'&&mw.snippet(c.factory,n).includes('.'+c.loader+'()'));
  const selected=blocks.sort((a,b)=>(a.end-a.start)-(b.end-b.start))[0];
  requireThat(selected,'Missing selected class block'); record(c.factory,'selected receiver constructor branch',selected);
  const constructor=only(nodes(selected,n=>n.type==='NewExpression'&&n.arguments.length===6),'selected receiver new');
  const loader=record(c.factory,'lazy protocol class loaders',mw.binding(c.factory,c.factoryLoader));
  const classLoader=only(nodes(loader,n=>n.type==='Property'&&key(n.key)===c.loader),'class loader');
  requireThat(nodes(classLoader,n=>n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&key(n.callee.property)==='bind'&&
    n.arguments[1]?.value===c.chain[0].module).length===1,'Class loader/module mismatch');
  // Each class must extend the next module's default export; a shared method's
  // presence elsewhere in a bundle does not grant a product this capability.
  const classes=c.chain.map(({module,name},ix)=>{
    const node=mw.binding(module,name);requireThat(node.type==='ClassDeclaration','Expected class declaration');
    record(module,name+' inheritance',node.superClass);
    if(ix<c.chain.length-1){
      const parent=node.superClass;requireThat(parent.type==='MemberExpression','Expected imported parent');
      const imp=mw.binding(module,parent.object.name);
      requireThat(imp.type==='CallExpression'&&imp.arguments[0]?.value===c.chain[ix+1].module,'Broken class inheritance');
    }
    return {module,node};
  });
  const method=(cls,name)=>only(cls.node.body.body.filter(n=>n.type==='MethodDefinition'&&key(n.key)===name),name);
  const selectedConstructor=record(classes[0].module,'selected class constructor',method(classes[0],'constructor'));
  requireThat(nodes(selectedConstructor,n=>n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&
    n.left.object.type==='ThisExpression'&&key(n.left.property)==='name'&&n.right.value===c.className).length===1,'Class name changed');
  const base=classes.at(-1), baseConstructor=record(base.module,'shared protocol constructor',method(base,'constructor'));
  const reportAssignment=only(nodes(baseConstructor,n=>n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&
    n.left.object.type==='ThisExpression'&&key(n.left.property)==='reportLength'),'report length');
  const reportBytes=mw.literal(base.module,reportAssignment.right);
  const extraParam=baseConstructor.value.params.at(-1);
  const reportId=mw.literal(base.module,property(extraParam.right,'reportId'));
  const transactionOwner=classes.find(cls=>cls.node.body.body.some(n=>n.type==='MethodDefinition'&&key(n.key)==='_getTransactionId'));
  const transaction=record(transactionOwner.module,'transaction namespace',method(transactionOwner,'_getTransactionId'));
  const modulo=only(nodes(transaction,n=>n.type==='BinaryExpression'&&n.operator==='==='&&n.left.type==='Literal'),'transaction wrap').left.value;
  const prefixes=nodes(transaction,n=>n.type==='BinaryExpression'&&n.operator==='|');
  const prefix=prefixes.length?mw.literal(transactionOwner.module,only(prefixes,'transaction prefix').left):0;
  const senderOwner=classes.find(cls=>cls.node.body.body.some(n=>n.type==='MethodDefinition'&&key(n.key)==='sendCommand'));
  const sender=record(senderOwner.module,'receiver sendCommand override',method(senderOwner,'sendCommand'));
  // These subclasses change transaction bits for primary-device commands.
  // Prove V2 is not among those exceptions before using the class prefix.
  const override=only(nodes(sender,n=>n.type==='AssignmentExpression'&&['|=','&='].includes(n.operator)),'transaction bit override');
  const condition=only(nodes(sender,n=>n.type==='LogicalExpression'&&n.operator==='||'&&n.right===override),'transaction override guard').left;
  const comparisons=nodes(condition,n=>n.type==='BinaryExpression'&&n.operator==='!==');
  requireThat(comparisons.length===2,'Unexpected transaction exception condition');
  for(const comparison of comparisons){
    const access=comparison.right;
    requireThat(access.type==='MemberExpression'&&access.object.type==='MemberExpression','Expected command exception array');
    const reference=access.object, imported=mw.binding(senderOwner.module,reference.object.name);
    const target=imported.arguments[0].value, exception=exported(target,key(reference.property));
    requireThat(exception.type==='NewExpression'&&exception.callee.name==='Uint8Array','Expected exception byte array');
    requireThat(mw.literal(target,exception.arguments[0])[2]!==191,'V2 needs an additional transaction override');
  }
  for(const name of ['_createDataSend','_calculateChecksum','_getUSBTransferInResult','getMultipleDeviceWirelessConnectionStatusV2'])record(base.module,name,method(base,name));
  const helper=exported(c.helper,c.helperExport), header=exported(c.headerModule,c.headerExport);
  requireThat(header.type==='NewExpression'&&header.callee.name==='Uint8Array','Header is not a byte array');
  const command=mw.literal(c.headerModule,header.arguments[0]);
  requireThat(JSON.stringify(command)==='[80,0,191]','Unsupported V2 command change');
  const send=only(nodes(helper,n=>n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&key(n.callee.property)==='sendCommand'),'V2 send');
  requireThat(send.arguments[0].type==='MemberExpression'&&key(send.arguments[0].property)===c.headerExport,'V2 header binding changed');
  const headerImport=mw.binding(c.helper,send.arguments[0].object.name);
  requireThat(headerImport.arguments[0].value===c.headerModule,'V2 command module changed');
  exported(c.endianModule,c.endianExport);
  const times={};
  for(const name of ['MAX_RETRY_IN','MAX_RETRY_OUT','SLEEP_TIME_BETWEEN_IN','SLEEP_TIME_BETWEEN_OUT','SLEEP_TIME_BETWEEN_OUT_IN']){
    exported(c.timings,name);times[name]=mw.literal(c.timings,mw.exported(c.timings,name));
  }
  const interval=(index,name)=>{
    const arg=constructor.arguments[index];requireThat(arg.type==='BinaryExpression'&&arg.operator==='*','Expected constructor interval scaling');
    requireThat(key(arg.right.property)===name,'Unexpected interval order');
    return mw.literal(c.factory,arg.left)*mw.literal(c.factory,arg.right);
  };
  requireThat(reportBytes===91&&reportId===0&&modulo===31,'Unsupported protocol envelope');
  return {source_product_id:c.product,vendor_id:infoValue('vendorId'),product_id:infoValue('dongleId'),
    claim_interface:infoValue('claimInterface'),report_bytes:reportBytes,report_id:reportId,
    protocol:'razer_device25_wireless_status_v2',command,transaction_prefix:prefix,transaction_modulus:modulo,
    max_retry_in:times.MAX_RETRY_IN,max_retry_out:times.MAX_RETRY_OUT,
    sleep_between_out_ms:interval(3,'SLEEP_TIME_BETWEEN_OUT'),sleep_between_out_in_ms:interval(4,'SLEEP_TIME_BETWEEN_OUT_IN'),
    sleep_between_in_ms:interval(5,'SLEEP_TIME_BETWEEN_IN'),source_class:c.className,evidence_path:evidencePath};
});
// The current official host selects a concrete path by VID/PID/container and
// interface, then opens the original node-rz-hid handle. Preserve this routing
// evidence separately from product capability facts.
const hostFile='.ref/host-4.0.827/source-evidence/background-current-source.js';
const host=fs.readFileSync(path.join(root,hostFile),'utf8');
const hostAst=acorn.parse(host,{ecmaVersion:'latest',sourceType:'module'});
const hostClass=only(nodes(hostAst,n=>n.type==='ClassDeclaration'&&n.id?.name==='Ft'),'host HID router');
for(const name of ['getHidDevices','connectHidDevice']){
  const node=only(hostClass.body.body.filter(n=>key(n.key)===name),'host '+name);
  receipts.push({label:'host Ft.'+name,path:hostFile,sha256:hash(host),offset:node.start,end:node.end,source:host.slice(node.start,node.end)});
}
const transportFile='.ref/host-4.0.827/electron/UsbRzDeviceAction.js',transport=fs.readFileSync(path.join(root,transportFile),'utf8');
const transportAst=acorn.parse(transport,{ecmaVersion:'latest'});
for(const name of ['hid.getDevices','hid.openDevice']){
  const node=only(nodes(transportAst,n=>n.type==='SwitchCase'&&n.test?.value===name),'host '+name);
  receipts.push({label:name,path:transportFile,sha256:hash(transport),offset:node.start,end:node.end,source:transport.slice(node.start,node.end)});
}
const payload={schema_version:1,scope:'Exact direct receiver bindings audited from current product middleware. No inference from catalog membership or shared class presence.',capabilities};
const evidence={method:'Static Acorn parsing and acquisition SHA-256 validation; no vendor execution',...payload,acquisition:acquisitions,receipts};
// The full inventory generator now owns the runtime asset. Retain the two
// original detailed seed audits without shrinking the catalog on a refresh.
for(const [file,value]of [[evidencePath,evidence]]){
  const absolute=path.join(root,file),text=JSON.stringify(value,null,2)+'\n';
  if(process.argv.includes('--check'))requireThat(fs.readFileSync(absolute,'utf8')===text,'Stale '+file);
  else{fs.mkdirSync(path.dirname(absolute),{recursive:true});fs.writeFileSync(absolute,text);}
}
console.log(`Receiver seed audits: ${capabilities.length} exact product bindings, ${receipts.length} AST receipts; runtime catalog owned by audit-receiver-catalog.cjs`);
