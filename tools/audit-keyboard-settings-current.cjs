// Static current middleware inspection only; no evaluation of vendor sources.
const fs=require('fs'),path=require('path'),assert=require('assert');
const {inspect,canonical}=require('./device-query-source.cjs');
const {CurrentMiddlewareSource}=require('./current-middleware-source.cjs');
const {walk,key,hash,Source}=require('./webpack-source.cjs');
const acorn=require('acorn');
const {sourceClaimInterfaces}=require('./expand-device-query-capabilities.cjs');
const root=path.resolve(__dirname,'..');
const read=p=>JSON.parse(fs.readFileSync(path.join(root,p),'utf8'));
const plans=read('.ref/middleware/receiver-protocol-requests.json');
const boots=read('.ref/middleware/bootstrap-requests.json');
const caps=read('assets/data/device-read-capabilities.json');
const timings=read('assets/data/receiver-query-capabilities.json');
const readProof=read('docs/re/mouse-read-capabilities-current-evidence.json');
const products=[],gaps=[];
function helper(audit,method,arity){
 const delegates=[];walk(method.node,n=>{if(n.type==='CallExpression'&&n.arguments.length===arity&&n.arguments[0]?.type==='ThisExpression'&&n.arguments.slice(1).every(a=>a.type==='Identifier'))delegates.push(n);});
 assert.equal(delegates.length,1);const resolved=audit.resolve(method.id,delegates[0].callee);audit.record('brightness delegate arity '+arity,resolved);return resolved;
}
function command(audit,target){
 const sends=[];walk(target.node,n=>{if(n.type==='CallExpression'&&key(n.callee.property)==='sendCommand')sends.push(n);});assert.equal(sends.length,1);
 const h=audit.resolve(target.id,sends[0].arguments[0]);audit.record('brightness command header',h);
 assert.equal(h.node.type,'NewExpression');assert.equal(h.node.callee.name,'Uint8Array');return audit.source.literal(h.id,h.node.arguments[0]);
}
let reference;
for(const plan of plans.products.filter(p=>p.device_info.values.category==='KEYBOARD')){
 if(plan.selected_class!=='rzDevice25DualLinkKeyboard'){gaps.push({product_id:plan.product_id,reason:'Current selected factory is not the verified DualLinkKeyboard Feature route'});continue;}
 const audit=inspect(plan,boots.products.find(p=>p.product_id===plan.product_id));
 if(process.argv.includes('--inspect')){
  console.log(plan.product_id,JSON.stringify(audit.features().map(f=>({name:f.name,value:f.value}))));
  for(const name of ['getBrightness','setBrightness']){
   try{const m=audit.method(name);console.log('METHOD',name,audit.source.snippet(m.id,m.node));if(name==='getBrightness'){for(const sym of ['$t','Bt']){const h=audit.resolve(m.id,audit.source.binding(m.id,sym));console.log('HELPER',audit.source.snippet(h.id,h.node));}}else{const q=audit.query(name);console.log(name,q.command,audit.source.snippet(q.helper.id,q.helper.node),q.parsers.map(p=>audit.source.snippet(p.id,p.node)));}}catch(e){console.log(e.message);}
  }
  for(const c of audit.definitionsContaining('.setBrightness(')){console.log('CALLER',c.id,audit.source.snippet(c.id,c.node));for(const [name,node]of audit.source.module(c.id).definitions)if(name==='Te')console.log('TWO_MODE',audit.source.snippet(c.id,node));}
  break;
 }
 try{
  const cap=caps.products.find(p=>p.product_id===plan.product_id);assert(cap&&cap.source_class===plan.selected_class,'Unproved transport capability');
  const timing=timings.capabilities.find(t=>t.source_product_id===plan.product_id),prior=readProof.products.find(p=>p.product_id===plan.product_id);
  assert(timing&&prior&&prior.source_class===plan.selected_class,'Missing individually source-gated transport evidence');
  for(const field of ['max_retry_in','max_retry_out','sleep_between_out_ms','sleep_between_out_in_ms','sleep_between_in_ms','transaction_prefix','transaction_modulus'])assert.equal(cap[field],timing[field],'Transport timing changed '+field);
  for(const field of ['direct_pids','vendor_id','report_id','report_bytes'])assert.deepEqual(cap[field],prior[field],'Source transport identity/layout changed '+field);
  assert.deepEqual(prior.claim_interface,plan.device_info.values.claimInterface,'Original PID interface map changed');
  const sourceInterfaces=sourceClaimInterfaces(plan.device_info.values);
  assert.equal(cap.claim_interface,sourceInterfaces.claim_interface,'Source physical interface changed');
  assert.deepEqual(cap.claim_interfaces_by_pid,sourceInterfaces.claim_interfaces_by_pid,'Source PID interface mapping changed');
  for(const receipt of prior.receipts){const body=audit.source.text(receipt.path);assert.equal(hash(body),receipt.sha256);assert.equal(body.slice(receipt.offset,receipt.end),receipt.source,'Inherited transport proof changed');}
  const get=audit.method('getBrightness'),set=audit.query('setBrightness');
  const region=helper(audit,get,1),getter=helper(audit,get,3);
  const regionCommand=command(audit,region),getCommand=command(audit,getter);
  if(process.argv.includes('--debug'))console.log(plan.product_id,{set:set.command,get:getCommand,regions:regionCommand});
  assert.deepEqual(set.command,[3,15,4]);assert.deepEqual(getCommand,[3,15,132]);assert.deepEqual(regionCommand,[80,15,128]);
  const texts={method:audit.source.snippet(get.id,get.node),setter:audit.source.snippet(set.helper.id,set.helper.node),getter:audit.source.snippet(getter.id,getter.node),regions:audit.source.snippet(region.id,region.node),parser:audit.source.snippet(set.parsers[0].id,set.parsers[0].node)};
  for(const token of ['parseInt(','.jsonData.regionIdList.length>0','catch(','=1}return'])assert(texts.method.includes(token),'Region fallback changed '+token);
  assert(texts.setter.includes('Math.floor(')&&texts.setter.includes('/100*255)'),'Brightness byte encoder changed');
  assert(texts.parser.includes('Math.ceil(')&&texts.parser.includes('/255*100)'),'Brightness percentage decoder changed');
  assert(texts.regions.includes('t+=5')&&texts.regions.includes('.regionIdList.push('),'Region list stride changed');
  assert.equal(set.parsers.length,1);
  const getterClone=structuredClone(getter.node);
  walk(getterClone,n=>{if(n.type!=='MemberExpression')return;let target;try{target=audit.resolve(getter.id,n);}catch{return;}
   if(target.node.type==='NewExpression'&&target.node.callee.name==='Uint8Array'){
    const values=audit.source.literal(target.id,target.node.arguments[0]);assert.deepEqual(values,getCommand,'Unexpected getter header reference');
    const replacement={type:'ArrayExpression',elements:values.map(value=>({type:'Literal',value}))};for(const k of Object.keys(n))delete n[k];Object.assign(n,replacement);
   }
  });
  const shapes=[get,set.methodTarget,set.helper,{id:getter.id,node:getterClone},region,set.parsers[0]].map(t=>canonical(audit.source,t.id,t.node));
  if(!reference)reference=shapes;else {if(process.argv.includes('--debug'))for(let i=0;i<shapes.length;i++)if(shapes[i]!==reference[i]){const a=shapes[i],b=reference[i];let x=0;while(a[x]===b[x])x++;console.log('shape differs',plan.product_id,i,a.slice(x-150,x+160),b.slice(x-150,x+160));}assert.deepEqual(shapes,reference,'Brightness semantics differ from current reference product');}
  const callers=audit.definitionsContaining('.setBrightness(');
  if(process.argv.includes('--init-inspect')){
   console.log(plan.product_id,'getBrightness callers',audit.definitionsContaining('.getBrightness(').map(t=>({module:t.id,source:audit.source.snippet(t.id,t.node)})));
   for(const t of audit.definitionsContaining('taskMakerInitBrightness'))console.log(t.id,audit.source.snippet(t.id,t.node));
   const importer=audit.source.binding(10745,'Gt');const init=audit.resolve(importer.arguments[0].value,audit.source.exported(importer.arguments[0].value,'nJ'));console.log('INITMAKER',audit.source.snippet(init.id,init.node));
   for(const t of audit.definitionsContaining('ON_INIT_BRIGHTNESS'))console.log('INIT',t.id,audit.source.snippet(t.id,t.node).slice(0,8000));
   break;
  }
  const writeCaller=callers.find(t=>{const text=audit.source.snippet(t.id,t.node);return text.includes('taskRunnerSetBrightnessToDevice')&&text.includes('signal.aborted')&&text.includes('profileId:1,regionId:')&&text.includes('taskRunnerBrightnessMemoryStorageCache');});
  assert(writeCaller,'No actual brightness setting task');audit.record('brightness setting cancellation, failure, cache and device caller',writeCaller);
  const callerText=audit.source.snippet(writeCaller.id,writeCaller.node);assert(callerText.includes('.regionIdEnum')&&callerText.includes('.AllRegion'),'Caller no longer selects AllRegion');
  // Resolve the enum on the actual caller, without assuming zero from protocol folklore.
  let enumNode;walk(writeCaller.node,n=>{if(n.type==='MemberExpression'&&key(n.property)==='AllRegion')enumNode=n;});
  assert(enumNode);const en=audit.resolve(writeCaller.id,enumNode.object);audit.record('actual brightness region enum',en);
  if(process.argv.includes('--debug'))console.log('enum',audit.source.snippet(en.id,en.node));
  assert.equal(en.node.type,'ObjectExpression');const allRegion=en.node.properties.find(p=>key(p.key)==='AllRegion');assert(allRegion);assert.equal(allRegion.value.value,0);
  assert(!plan.feature.receipt.source.includes('regionIdEnum'),'Product config selects a specific region');
  const featureCalls=[];for(const [id,scope]of audit.source.modules)if(scope.file===audit.source.mainFile)walk(scope.fn,n=>{if(n.type==='CallExpression'&&key(n.callee.property)==='useFeature'&&n.arguments[0]?.value==='deviceUI'&&n.arguments[1]?.value==='config')featureCalls.push({id,node:n});});
  assert.equal(featureCalls.length,1,'Ambiguous actual deviceUI config');const config=audit.source.literal(featureCalls[0].id,featureCalls[0].node.arguments[2]);assert.equal(config.brightness.hasTwoMode,false,'Two-mode brightness requires another setter chain');audit.record('actual product brightness UI registration hasTwoMode=false',featureCalls[0]);
  const uiManifest=read(`.ref/devices/${plan.product_id}/asset-manifest.json`),uiFiles=[...new Set(Object.values(uiManifest.files))].filter(f=>/\.js$/.test(f));
  const brightnessSources=uiFiles.map(file=>`.ref/devices/${plan.product_id}/${file.replace(/^\.\//,'')}`).filter(file=>fs.existsSync(path.join(root,file))&&fs.readFileSync(path.join(root,file),'utf8').includes('this.props.setBrightness(this.state.brightness)'));
  assert.equal(brightnessSources.length,1,'Ambiguous manifest-declared current brightness source');
  const uiFile=brightnessSources[0],uiText=fs.readFileSync(path.join(root,uiFile),'utf8'),http=read(uiFile+'.http.json');assert.equal(hash(uiText),http.sha256);assert.equal(http.http_status,200);
  const classes=[];walk(acorn.parse(uiText,{ecmaVersion:'latest'}),n=>{if(['ClassDeclaration','ClassExpression'].includes(n.type)){const text=uiText.slice(n.start,n.end);if(text.includes('this.toggleSwitch=')&&text.includes('this.changeValue=')&&text.includes('this.props.setBrightness(this.state.brightness)'))classes.push(n);}});assert.equal(classes.length,1,'Unresolved actual UI brightness component');
  const uiNode=classes[0],uiSnippet=uiText.slice(uiNode.start,uiNode.end);for(const token of ['min:0,max:100','0===','isEnabled:!0','isEnabled:!1','brightness.value','changeValue:this.changeValue'])assert(uiSnippet.includes(token),'UI brightness contract changed '+token);
  const ui=Object.create(Source.prototype);ui.directory=`.ref/devices/${plan.product_id}`;ui.files=uiFiles.map(f=>`${ui.directory}/${f.replace(/^\.\//,'')}`);ui.modules=new Map();ui.texts=new Map();ui.parsed=new Set();
  const baseText=ui.text.bind(ui);ui.text=file=>{const text=baseText(file),receipt=read(file+'.http.json');assert.equal(hash(text),receipt.sha256);assert.equal(receipt.http_status,200);return text;};
  ui.parse=function(file){if(this.parsed.has(file))return;const text=this.text(file);walk(acorn.parse(text,{ecmaVersion:'latest'}),node=>{
   if(node.type!=='Property'||!Number.isInteger(key(node.key))||!['FunctionExpression','ArrowFunctionExpression'].includes(node.value.type))return;
   const fn=node.value;if(fn.body.type!=='BlockStatement'||fn.params.length<3)return;const id=key(node.key),scope={id,file,fn,definitions:new Map(),exports:new Map()};
   for(const statement of fn.body.body){
    if(statement.type==='VariableDeclaration')for(const d of statement.declarations)if(d.id.type==='Identifier')scope.definitions.set(d.id.name,d.init);
    if(['FunctionDeclaration','ClassDeclaration'].includes(statement.type))scope.definitions.set(statement.id.name,statement);
    walk(statement,n=>{if(/Function|Class/.test(n.type))return false;
     if(n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&key(n.callee.property)==='d'&&n.callee.object.name===fn.params[2].name&&n.arguments[0]?.name===fn.params[1]?.name&&n.arguments[1]?.type==='ObjectExpression')for(const p of n.arguments[1].properties){const body=p.value.body;scope.exports.set(key(p.key),body?.type==='BlockStatement'?body.body.find(s=>s.type==='ReturnStatement')?.argument:body);}
    });
   }
   this.modules.set(id,scope);return false;
  });this.parsed.add(file);};
  ui.parse(uiFile);
  let owner=[...ui.modules.values()].find(scope=>scope.file===uiFile&&scope.fn.start<=uiNode.start&&scope.fn.end>=uiNode.end);
  if(!owner){const enclosing=[];walk(acorn.parse(uiText,{ecmaVersion:'latest'}),n=>{if(/Function/.test(n.type)&&n.body.type==='BlockStatement'&&n.start<uiNode.start&&n.end>uiNode.end)enclosing.push(n);});enclosing.sort((a,b)=>(a.end-a.start)-(b.end-b.start));assert(enclosing.length,'Brightness entry closure unresolved');
   owner={id:-1,file:uiFile,fn:enclosing[0],definitions:new Map()};for(const statement of owner.fn.body.body){if(statement.type==='VariableDeclaration')for(const d of statement.declarations)if(d.id.type==='Identifier')owner.definitions.set(d.id.name,d.init);}ui.modules.set(-1,owner);
  }
  const rangeMounts=[];walk(uiNode,n=>{if(n.type==='CallExpression'&&n.arguments[1]?.type==='ObjectExpression'&&n.arguments[1].properties.some(p=>key(p.key)==='extraClass'&&p.value.value==='brightness'))rangeMounts.push(n);});assert.equal(rangeMounts.length,1,'Brightness Range mounting ambiguous');
  const mount=rangeMounts[0],props=mount.arguments[1].properties;assert(!props.some(p=>['callOnChangeOnEveryStep','debounceTime'].includes(key(p.key))),'Brightness Range overrides pointer commit policy');
  const component=mount.arguments[0];assert.equal(component.type,'MemberExpression');assert.equal(component.object.type,'Identifier');
  const imported=ui.binding(owner.id,component.object.name);assert.equal(imported.type,'CallExpression');assert(Number.isInteger(imported.arguments[0]?.value));
  const rangeId=imported.arguments[0].value;let range=ui.exported(rangeId,key(component.property));const seen=new Set();
  for(;;){if(range.type==='Identifier'){assert(!seen.has(range.name));seen.add(range.name);range=ui.binding(rangeId,range.name);continue;}
   if(range.type==='CallExpression'&&range.arguments.length===1&&range.arguments[0].type==='Identifier'){range=range.arguments[0];continue;}break;}
  if(process.argv.includes('--range-inspect')){console.log(plan.product_id,rangeId,ui.snippet(rangeId,range));continue;}
  assert(['ClassDeclaration','ClassExpression'].includes(range.type),'Range export is not its resolved class');
  const rangeText=ui.snippet(rangeId,range);
  for(const token of ['this.mouseIsDown?','this.props.callOnChangeOnEveryStep&&this.props.changeValue(','this.props.debounceTime','this.onMouseUp=','this.props.changeValue(this.state.value','window.addEventListener("mouseup",this.onMouseUp)','window.removeEventListener("mouseup",this.onMouseUp','this.mouseIsDown=!1','this.handleKeyDown=','35!==','36!==','37!==','38!==','39!==','40!==','.preventDefault()'])assert(rangeText.includes(token),'Range pointer release policy changed '+token);
  const rangeFile=ui.module(rangeId).file,rangeFileText=ui.text(rangeFile),rangeHttp=read(rangeFile+'.http.json');
  audit.receipts.push({label:'actual brightness Range component pointer preview then release, lifecycle cleanup',module:rangeId,path:rangeFile,sha256:hash(rangeFileText),offset:range.start,end:range.end,source:rangeText});
  audit.receipts.push({label:'actual Range brightness mount without step/debounce overrides',path:uiFile,sha256:hash(uiText),offset:mount.start,end:mount.end,source:uiText.slice(mount.start,mount.end)});
  if(!audit.source.acquisition.some(a=>a.path===rangeFile))audit.source.acquisition.push({path:rangeFile,sha256:hash(rangeFileText),bytes:Buffer.byteLength(rangeFileText),source_url:rangeHttp.source_url,fetched_at_utc:rangeHttp.fetched_at_utc});
  audit.receipts.push({label:'actual current UI brightness change/toggle zero-disable and range',path:uiFile,sha256:hash(uiText),offset:uiNode.start,end:uiNode.end,source:uiSnippet});
  audit.source.acquisition.push({path:uiFile,sha256:hash(uiText),bytes:Buffer.byteLength(uiText),source_url:http.source_url,fetched_at_utc:http.fetched_at_utc});
  const getterParsers=[];walk(getter.node,n=>{if(n.type==='CallExpression'&&n.callee.type==='Identifier'&&n.arguments.length===1){const binding=audit.source.module(getter.id).definitions.get(n.callee.name);if(binding&&/Function/.test(binding.type))getterParsers.push(audit.resolve(getter.id,n.callee));}});
  assert.equal(getterParsers.length,1);assert.equal(canonical(audit.source,getterParsers[0].id,getterParsers[0].node),canonical(audit.source,set.parsers[0].id,set.parsers[0].node),'Getter parser differs');audit.record('brightness getter response parser',getterParsers[0]);
  const syncCaller=callers.find(t=>{const text=audit.source.snippet(t.id,t.node);return text.includes('.getBrightness(')&&text.includes('result:"SUCCESS"')&&text.includes('profileId:0');});
  assert(syncCaller,'Missing hardware profile brightness read/set caller');audit.record('original optional hardware profile brightness synchronization',syncCaller);
  const initDispatchers=audit.definitionsContaining('case"ON_INIT_BRIGHTNESS"');assert.equal(initDispatchers.length,1,'Unresolved original brightness init dispatcher');
  const cases=[];walk(initDispatchers[0].node,n=>{if(n.type==='SwitchCase'&&n.test?.value==='ON_INIT_BRIGHTNESS')cases.push(n);});assert.equal(cases.length,1);audit.record('original ON_INIT_BRIGHTNESS dispatch parameters',{id:initDispatchers[0].id,node:cases[0]});
  const initDelegates=[];walk(cases[0],n=>{if(n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&n.arguments.length===10)initDelegates.push(n);});assert.equal(initDelegates.length,1,'Ambiguous brightness initialization task maker');
  const initMaker=audit.resolve(initDispatchers[0].id,initDelegates[0].callee),initText=audit.source.snippet(initMaker.id,initMaker.node);for(const token of ['.isEnabled','.value','NORMAL_SKIPPABLE','enqueueTask(','"setting"','"version"','.activeProfile'])assert(initText.includes(token),'Brightness init setter-chain changed '+token);assert(!initText.includes('.getBrightness('),'Brightness init now reads device first');audit.record('original brightness initialization consumes saved profile and enqueues setter, not a getter',initMaker);
  const loadProfiles=audit.definitionsContaining('ON_INIT_BRIGHTNESS').filter(t=>{const text=audit.source.snippet(t.id,t.node);return text.includes('loadActiveProfile')&&text.includes('MW_ACTION_FROM_LOCALSTORAGE')&&text.includes('selectedProfileGuid:');});assert.equal(loadProfiles.length,1,'Saved profile initialization producer unresolved');audit.record('original saved active profile producer for ON_INIT_BRIGHTNESS',loadProfiles[0]);
  for(const name of ['_createDataSend','_calculateChecksum','_getUSBTransferInResult','connectDevice','sendCommand','_getTransactionId'])audit.method(name);
  const alternate=audit.alternateCommands();for(const c of [regionCommand,getCommand,set.command])assert(!alternate.some(a=>a.command[2]===c[2]),'Alternate transaction not implemented');
  products.push({product_id:plan.product_id,source_class:plan.selected_class,get_command:getCommand,set_command:set.command,regions_command:regionCommand,active_profile:1,all_region:0,fallback_region:1,region_record_bytes:5,transport:cap,factory:plan.factory,feature:plan.feature.receipt,bootstrap:boots.products.find(p=>p.product_id===plan.product_id).bootstrap,device_info:plan.device_info,receipts:audit.receipts,acquisition:audit.source.acquisition});
 }catch(error){gaps.push({product_id:plan.product_id,reason:error.message.split('\n')[0]});}
}
if(process.argv.includes('--inspect')||process.argv.includes('--range-inspect')||process.argv.includes('--init-inspect'))process.exit(0);
assert(products.length>0,'Refusing to overwrite a valid catalog with an empty result after failed source gates');
const evidence={schema_version:1,method:'Current manifest-verified factories, individual AST-resolved brightness methods/commands/parsers, actual cancellation/cache/device tasks; static only',products,gaps,implementation:{rust:'crates/razer-device/src/keyboard_settings.rs',consumers:['crates/razer-pages/src/features/keyboard_brightness.rs','crates/razer-pages/src/features/keyboard_products.rs','crates/razer-pages/src/features/source_workspace.rs','crates/razer-pages/src/features/product_workspace.rs','crates/razer-shell/src/shell/keyboard_brightness_write.rs','crates/razer-shell/src/shell/keyboard_brightness_read.rs','crates/razer-discovery/src/direct/mod.rs','crates/razer-discovery/src/direct/platform/windows.rs','crates/razer-ipc/src/lib.rs','crates/razer-service/src/runtime/portable.rs','crates/razer-service/src/runtime/windows/native.rs'],source_read_behavior:'AllRegion=0 getter queries regions; first returned region, fallback=1 only if region query rejects',source_initialization:'Original saved active-profile producer emits ON_INIT_BRIGHTNESS; nJ task maker uses isEnabled/value and enqueues the active setter. This is not a getter-based initialization',local_observation:'First active-lighting device read is application observation policy; separate loading/percentage/error UI consumes real getter and writeback values, preserves local profile brightness/value/isEnabled and slider draft' ,source_write_behavior:'profileId=1 AllRegion=0 setter encodes floor(percent/100*255); response decoder ceil(byte/255*100)',ui_behavior:'Actual mounted Range has no every-step/debounce overrides; pointer Change previews, Release submits parent changeValue; brightness toggle submits immediately; zero disables brightness, toggle keeps value; draft update is distinct from device acknowledgement',readback:'Application confirmation policy, original active-setting task does not read back',hardware_profiles:'Optional original sync caller acquired; not implemented by active brightness API',task_cache:'Original task memory/version/cancellation acquired; Rust read/write requests share a serialized in-flight slot across restore/connection invalidation; user edits advance read revision and have priority; current scope epoch and latest explicit queued intent are retained, stale completion releases its slot without accepting old observation. Original shared task cache and global lock/state machine are not fully replaced',transport_scope:'Shared OS-independent route dispatcher resolves a source-interface-matched live wired HID collection or an isolated Windows ContainerId adapter; real IPC replies validate collection/target and product, readback range and write intent. BLE and receiver relay remain absent',gaps:['Original ON_INIT_BRIGHTNESS saved-profile setter producer is acquired but not yet connected to real profile provenance, init policy and global lock; initial getter observation is a separate application policy','Actual adjustmentModeRunning and nanoLeafEnabled producers and disabled policy are not yet connected'],runtime:'Not executed, development prohibition'}};
const asset={schema_version:1,products:products.map(p=>{const {factory,feature,bootstrap,device_info,receipts,acquisition,...row}=p;return row;})};
for(const [file,data]of [['docs/re/keyboard-settings-current-evidence.json',evidence],['assets/data/keyboard-settings-capabilities.json',asset]]){
 const body=JSON.stringify(data,null,2)+'\n';if(process.argv.includes('--check'))assert.equal(fs.readFileSync(path.join(root,file),'utf8'),body,'Stale '+file);else fs.writeFileSync(path.join(root,file),body);
}
console.log(JSON.stringify({products:products.length,gaps}));
