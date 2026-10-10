// Current 182 page's DPI stages chain. Static parsing only; no vendor execution.
const fs=require('fs'),path=require('path'),assert=require('assert'),acorn=require('acorn');
const {walk,key,hash,Source}=require('./webpack-source.cjs');
const {inspect}=require('./device-query-source.cjs');
const root=path.resolve(__dirname,'..'),read=file=>fs.readFileSync(path.join(root,file),'utf8');
const manifestPath='.ref/devices/182/asset-manifest.json',manifest=JSON.parse(read(manifestPath));
const pagePath='.ref/devices/182/'+manifest.files['main.js'].slice(2),text=read(pagePath);
const model=JSON.parse(read('docs/re/mouse-polling-model-current-evidence.json')).evidence.find(p=>p.product_id===182);
assert.equal(hash(read(manifestPath)),model.manifest.sha256);
assert.equal(hash(text),model.receipts[0].sha256);
const receipts=[],nodes=new Map();
const recordUi=(label,node)=>receipts.push({label,path:pagePath,sha256:hash(text),offset:node.start,end:node.end,source:text.slice(node.start,node.end)});
walk(acorn.parse(text,{ecmaVersion:'latest'}),node=>{
  const source=text.slice(node.start,node.end);
  if(node.type==='ClassDeclaration'&&['IM','jm','Vm','OM'].includes(node.id?.name)) {
    if(node.id.name!=='IM'||source.includes('this.changeDpiValueX='))nodes.set(node.id.name,node);
  }
  if(node.type==='VariableDeclarator'&&['AM','lM','Zm','Wm','vm','Lm','Ym','gm','aE'].includes(node.id?.name)
      && !['Literal','TemplateLiteral'].includes(node.init?.type))nodes.set('binding:'+node.id.name,node);
  if(node.type==='VariableDeclarator'&&node.init?.type==='ArrowFunctionExpression'&&['Jm','qm','Qm','eM','EM','$m','jE'].includes(node.id.name)) {
    if(['Jm','qm','Qm','eM','EM','$m'].includes(node.id.name)&&source.includes('type:Ne.'))nodes.set(node.id.name,node.init);
    if(node.id.name==='jE'&&source.includes('stages'))nodes.set('jE',node.init);
  }
  if(node.type==='FunctionExpression'&&node.end-node.start<15000){
    if(source.includes('case Ne.Q7r:')&&source.includes('ON_STAGES_CHANGE'))nodes.set('stageReducer',node);
    if(source.includes('case Ne.F8S:')&&source.includes('case Ne.Mmk:'))nodes.set('activeDisplayReducer',node);
  }
});
for(const symbol of ['IM','jm','Vm','OM','binding:AM','binding:lM','binding:Zm','binding:Wm','binding:vm','binding:Lm','binding:Ym','binding:gm','binding:aE','Jm','qm','Qm','eM','EM','$m','stageReducer','activeDisplayReducer']) {
  assert(nodes.has(symbol),'Current DPI caller missing '+symbol);recordUi('current DPI UI '+symbol,nodes.get(symbol));
}
const sourceUi=symbol=>text.slice(nodes.get(symbol).start,nodes.get(symbol).end);
assert(sourceUi('IM').includes('a||_(s,n)')&&sourceUi('IM').includes('a||this.props.setDotsPerInchStageValueY(o,i)'));
assert(sourceUi('IM').includes('this.props.setPerformanceSelectedSensitivityStage(this.props.dpiStages,e),this.props.setActiveDPI(a,t,_)'));
assert(!sourceUi('activeDisplayReducer').includes('postMessage'));
assert(sourceUi('OM').includes('(AM,{})')&&sourceUi('binding:AM').endsWith('(IM)'));
assert(sourceUi('binding:Zm')==='Zm=jm'&&sourceUi('binding:Wm')==='Wm=Vm');
assert(sourceUi('jm').includes('a||this.props.changeDpiValueX(e,E,!1)')&&sourceUi('jm').includes('a||this.props.changeDpiValueY(e,E,!1)'));
assert(sourceUi('Vm').includes('setParentState:this.changeDpiValueX')&&sourceUi('Vm').includes('changeValue:this.changeDpiValueX')&&sourceUi('Vm').includes('maxLength:6'));
assert(sourceUi('IM').includes('if(!s&&i!==o)')&&sourceUi('IM').includes('independent:!_'));
const ui=Object.assign(Object.create(Source.prototype),{directory:'.ref/devices/182',files:[...new Set(Object.values(manifest.files))].filter(f=>/\.js$/.test(f)).map(f=>'.ref/devices/182/'+f.slice(2)),modules:new Map(),texts:new Map(),parsed:new Set()});
ui.parse(pagePath);
assert.equal(nodes.get('binding:vm').init.arguments[0].value,4230);
assert.equal(nodes.get('binding:Lm').init.arguments[0].value,801);
for(const module of [4230,801]) {
  const scope=ui.module(module);assert.equal(scope.file,pagePath,'Current 182 mounted editor chunk changed');
  receipts.push({label:'actual mounted '+(module===4230?'DPI number editor':'DPI linear Range'),...ui.receipt(module,scope.fn)});
}
const number=ui.snippet(4230,ui.module(4230).fn),range=ui.snippet(801,ui.module(801).fn);
for(const token of ['this.props.setParentState(e,this.isRegisterEvent)','this.isRegisterEvent=!1','this.sendToParent(a)','13!==e.keyCode&&27!==e.keyCode||this.inputDom.current.blur()','setInterval(()=>{e()},300)','Math.ceil(e/this.props.stepValue)*this.props.stepValue'])assert(number.includes(token),'Current number commit policy changed '+token);
assert(range.includes('this.mouseIsDown')&&range.includes('this.onMouseUp=')&&range.includes('this.props.changeValue'),'Current Range preview/release chain changed');
const enumModule=nodes.get('binding:aE').init.arguments[0].value;
const navs=[];walk(acorn.parse(text,{ecmaVersion:'latest'}),node=>{
  if(node.type!=='ObjectExpression')return;
  const properties=new Map(node.properties.filter(p=>p.type==='Property').map(p=>[key(p.key),p.value]));
  const component=properties.get('component'),name=properties.get('name');
  if(component?.type==='CallExpression'&&component.arguments[0]?.name==='lM'){
    assert(name?.type==='MemberExpression'&&name.object.name==='aE');
    assert.equal(ui.literal(enumModule,ui.exported(enumModule,key(name.property))),'TAB_PERFORMANCE');
    navs.push(node);recordUi('actual current TAB_PERFORMANCE root mounts lM/connect(OM)',node);
    receipts.push({label:'actual Performance name export',...ui.receipt(enumModule,ui.exported(enumModule,key(name.property)))});
  }
});assert.equal(navs.length,1,'Ambiguous actual current Performance mount');
const plans=JSON.parse(read('.ref/middleware/receiver-protocol-requests.json'));
const boots=JSON.parse(read('.ref/middleware/bootstrap-requests.json'));
const audit=inspect(plans.products.find(p=>p.product_id===182),boots.products.find(p=>p.product_id===182));
let stageMaker,xyMaker;
for(const[id,module]of audit.source.modules)walk(module.fn,node=>{
  if(node.type==='SwitchCase'&&['ON_STAGES_CHANGE','ON_STAGE_XY_CHANGE'].includes(node.test?.value)){
    receipts.push(audit.record('DPI dispatcher '+node.test.value,{id,node}));
    walk(node,part=>{
      if(part.type==='CallExpression'&&part.callee.type==='SequenceExpression'){
        const property=key(part.callee.expressions.at(-1).property);
        if(property==='bSr')stageMaker=audit.resolve(id,part.callee);
        if(property==='v0I')xyMaker=audit.resolve(id,part.callee);
      }
    });
  }
});
assert(stageMaker&&xyMaker);receipts.push(audit.record('DPI stages profile/version save and hardware task maker',stageMaker));
receipts.push(audit.record('XY independent flag profile save and cleanup',xyMaker));
let task,callback;
walk(stageMaker.node,node=>{
  if(node.type==='MemberExpression'&&node.property.name==='p6N')task=audit.resolve(stageMaker.id,node);
  if(node.type==='MemberExpression'&&node.property.name==='EDu')callback=audit.resolve(stageMaker.id,node);
});
assert(task&&callback);receipts.push(audit.record('DPI stages current-setting task and readback',task));
receipts.push(audit.record('DPI stages completion and UI action cleanup',callback));
const taskSource=audit.source.snippet(task.id,task.node);
for(const part of ['getFeatureParam("taskRunnerSetDPI","setDpiLevel")', '.setDpiLevel(1,e,t)',
  '.setDPIStages(1,l,d,v)', '.getDPIStages(1)', 'v[e]+1!==d[e]', 'new Error("data not match")', 'yield Mt(s,l,d,v)'])
  assert(taskSource.includes(part),'Current DPI task changed '+part);
for(const symbol of ['Ke','Mt']) {
  const targets=[];walk(audit.source.modules.get(task.id).fn,node=>{if(node.type==='FunctionDeclaration'&&node.id?.name===symbol)targets.push({id:task.id,node});});
  assert.equal(targets.length,1);receipts.push(audit.record(symbol==='Ke'?'visible-stage packing and active index remapping':'OBM slots comparison and stage write',targets[0]));
}
const commands={};
for(const method of ['setDPIStages','getDPIStages']) {
  const target=audit.method(method);receipts.push(audit.record('inline DPI stages wrapper '+method,target));
  let header,parser;walk(target.node,node=>{
    if(node.type==='CallExpression'&&node.callee.property?.name==='sendCommand')header=audit.resolve(target.id,node.arguments[0]);
    if(node.type==='CallExpression'&&node.callee.name==='m')parser=audit.resolve(target.id,node.callee);
  });
  assert(header&&parser);assert.equal(header.node.callee.name,'Uint8Array');
  const command=audit.source.literal(header.id,header.node.arguments[0]);
  assert.deepEqual(command,method==='setDPIStages'?[80,4,6]:[80,4,134]);commands[method]=command;
  receipts.push(audit.record(method+' command',header));receipts.push(audit.record(method+' stage response parser',parser));
}
const features=audit.features();assert(!features.some(f=>f.name==='taskRunnerSetDPI'));
const initial=[];
for(const[id,module]of audit.source.modules)for(const node of module.definitions.values())if(node&&audit.source.snippet(id,node).startsWith('Object.assign(Object.assign({taskRunnerSetDPI:'))initial.push({id,node});
assert.equal(initial.length,1);const defaults=initial[0];
assert(audit.source.snippet(defaults.id,defaults.node).includes('taskRunnerSetDPI:{setDpiStages:{}}'));
receipts.push(audit.record('default DPI feature is setDpiStages',defaults));
walk(defaults.node,node=>{
  if(node.type==='CallExpression'&&node.callee.type==='SequenceExpression'){
    const target=audit.resolve(defaults.id,node.callee);assert(!audit.source.snippet(target.id,target.node).includes('taskRunnerSetDPI'));
    receipts.push(audit.record('derived defaults do not override DPI feature',target));
  }
  if(node.type==='MemberExpression'&&key(node.property)==='injectorDefaults'){
    const binding=audit.source.module(defaults.id).definitions.get(node.object.name);
    assert.equal(binding?.type,'CallExpression');const module=binding.arguments[0].value;
    assert.throws(()=>audit.source.exported(module,'injectorDefaults'));
    receipts.push(audit.record('current cache has no injectorDefaults override',{id:module,node:audit.source.module(module).fn}));
  }
});
const report={schema_version:1,product_id:182,source_manifest:{path:manifestPath,sha256:hash(read(manifestPath))},source_receipts:receipts,
  source_semantics:{page:'IM stage selection and Range release update stages; Jm/setActiveDPI alone updates display only',
    range:'changeDpiValueX/Y suppress submission while drag flag is set; release sends full stage set and active stage',
    action:'eM/EM/$m -> stage reducer ON_STAGES_CHANGE -> bSr/ft -> p6N/Xe; independent XY flag has separate profile-only chain',
    persistence:'stage maker clones stages, mutates active profile stages/active/count/enable, increments version and saves/broadcasts before enqueueing',
    feature:'182 current default selects setDpiStages; no bootstrap registration, derived override or cache injectorDefaults selects setDpiLevel',
    packing:'Ke filters visible stages, remaps selected index to visible-stage ordinal, emits [index0,x,y,0] tuples; disabled stages sends active stage only',
    protocol:{commands,class_id:1,get_payload:[1],set_payload:'[classId,activeStage,count, repeated(index0,x_u16be,y_u16be,z_u16be)]',set_packet_size:'7*count+3',response:'[classId,activeStage,count, repeated(index1,x_u16be,y_u16be,z_u16be)]'},
    verification:'Xe compares each returned nonzero stage index with sent index+1 and exact XYZ; Mt compares all OBM slot data plus active/count before conditional setter',
    cleanup:'device task abort check, status/error callback and UI post-process cleanup; task memory cache updated for stages path'},
  implementation:{status:'source_semantics_acquired_with_stage_protocol_and_page_consumer',stage_protocol:'crates/razer-device/src/mouse_dpi_stages.rs',stage_backend_evidence:'docs/re/mouse-dpi-stages-current-evidence.json',page_consumer:'crates/razer-pages/src/features/mouse_dpi_profile.rs',source_page:'crates/razer-pages/src/features/sensitivity.rs',shell_consumer:'crates/razer-shell/src/shell/mouse_dpi_stages.rs',existing_level_write:'DeviceWriteSetting::Dpi selector0 setDpiLevel read/write/readback exists but is not used for this UI stage submission chain'},
  gaps:['Page/IPC/direct current-table submission is implemented; original profile/version/cache/task-queue/OBM persistence remains separately incomplete.',
    'Original active-profile save/version/cache/OBM slot transport is not connected by the existing selector0 level writer.',
    'The current stage task does not check class1 active/count in its first readback loop; preserve this original behavior separately from OBM slot comparison.',
    'No runtime acceptance was executed.'],runtime_acceptance:'not_run'};
const output='docs/re/mouse-dpi-ui-current-evidence.json',serialized=JSON.stringify(report,null,2)+'\n';
if(process.argv.includes('--write'))fs.writeFileSync(path.join(root,output),serialized);
else assert.equal(read(output).replace(/\r\n/g,'\n'),serialized,'Current DPI UI evidence changed');
console.log('182 DPI stages: current page, profile task, feature branch and inline protocol statically checked.');
