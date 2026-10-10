// Inspect mounted source components as syntax/data; never execute vendor JavaScript.
const fs=require('fs'), path=require('path'), acorn=require('acorn');
const {Source,walk,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'), check=process.argv.includes('--check');
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
const requireValue=(v,message)=>{if(!v)throw Error(message);return v;};
const catalog=JSON.parse(read('crates/razer-pages/src/features/mouse_products_data.json'));
// The independently maintained three-mode audit owns this existing capability.
const specs=JSON.parse(read('crates/razer-pages/src/features/mouse_scroll_wheel_data.json')).filter(spec=>Array.isArray(spec.defaults.disabledModes));
const reports=[];
function definitions(block){
 const result=new Map();
 for(const n of block.body){
  if(n.type==='VariableDeclaration')for(const d of n.declarations)if(d.id.type==='Identifier'&&d.init)result.set(d.id.name,d.init);
  if(['ClassDeclaration','FunctionDeclaration'].includes(n.type))result.set(n.id.name,n);
 }
 return result;
}
for(const product of catalog.filter(product=>product.profile.scrollWheel&&!Array.isArray(product.profile.scrollWheel.disabledModes))){
 const pid=product.product_id,directory=`.ref/devices/${pid}`,manifestPath=directory+'/asset-manifest.json';
 const manifest=JSON.parse(read(manifestPath));
 requireValue(hash(read(product.source))===product.source_sha256,'Stale CONFIG '+pid);
 const source=Object.assign(Object.create(Source.prototype),{directory,
  files:[...new Set(Object.values(manifest.files))].filter(file=>/^\.\/static\/js\/.*\.js$/.test(file)).map(file=>directory+'/'+file.slice(2)),
  modules:new Map(),texts:new Map(),parsed:new Set()});
 const candidates=[],scanned=[],reducerReads=[],mappingTips=[],asts=new Map();
 for(const file of source.files){
  const text=source.text(file);scanned.push({path:file,sha256:hash(text)});
  if(/\.scrollWheelReducer\b|\["scrollWheelReducer"\]/.test(text))reducerReads.push(file);
  const tip=text.indexOf('className:"tip scrollmode-switch"');
  if(tip>=0)mappingTips.push({path:file,sha256:hash(text),offset:tip-120,end:tip+730,source:text.slice(tip-120,tip+730)});
  if(!text.includes('this.state.scrollModeUI'))continue;
  const ast=acorn.parse(text,{ecmaVersion:'latest'});
  asts.set(file,ast);
  walk(ast,node=>{
   if(node.type!=='BlockStatement')return;
   for(const child of node.body){
    if(child.type==='ClassDeclaration'&&text.slice(child.start,child.end).includes('this.state.scrollModeUI')){
     const context={file,definitions:definitions(node)};
     candidates.push({context,node:child,name:child.id.name});
    }
   }
  });
 }
 if(!candidates.length){
  requireValue(reducerReads.length===0,'Unclassified scroll reducer consumer requires audit '+pid);
  reports.push({product_id:pid,status:'no_scroll_editor_in_current_declared_js',
   profile_fields:Object.keys(product.profile.scrollWheel),supportScrollModeToggle:product.info.supportScrollModeToggle??null,scanned,reducer_reads:reducerReads,mapping_tooltips:mappingTips,
   note:'Shared mapping-button scrollmode-switch tooltip is conditional on assignmentValue; it is not a standalone scroll panel.'});
  continue;
 }
 requireValue(candidates.length===1,'Ambiguous scroll editor '+pid);
 const candidate=candidates[0],context=candidate.context,widget=source.text(context.file).slice(candidate.node.start,candidate.node.end);
 const hasLevels=widget.includes('this.changeSmartReelLevel=');
 requireValue(widget.includes('"FreeSpin"===this.state.scrollModeUI?"Tactile":"FreeSpin"'),'Unknown mode transition '+pid);
 requireValue(!widget.includes('disabledModes')&&!widget.includes('MicroTactile'),'Needs another mode adapter '+pid);
 requireValue(hasLevels || widget.includes('"FreeSpin"===this.props.scrollMode?"disabled":""'),'Unknown basic Smart Reel lock '+pid);
 const receipts=[];
 const receipt=(name,node,ctx)=>({symbol:name,path:ctx.file,sha256:hash(source.text(ctx.file)),offset:node.start,end:node.end,source:source.text(ctx.file).slice(node.start,node.end)});
 const moduleContext=id=>{const module=source.module(id);return{file:module.file,definitions:module.definitions,id};};
 function resolve(expr,ctx){
  if(expr.type==='Identifier'&&ctx.definitions.has(expr.name))return{name:expr.name,node:ctx.definitions.get(expr.name),context:ctx};
  if(expr.type==='MemberExpression'&&expr.object.type==='Identifier'){
   let binding=ctx.definitions.get(expr.object.name);
   if(binding?.type==='SequenceExpression')binding=binding.expressions.at(-1);
   if(binding?.type==='CallExpression'&&typeof binding.arguments[0]?.value==='number'){
    const id=binding.arguments[0].value;
    const exported=source.module(id).exports.get(expr.property.name);
    if(exported)return{name:`module:${id}:${expr.property.name}`,node:exported,context:moduleContext(id)};
   }
  }
 }
 function label(expr,ctx=context){
  const found=requireValue(resolve(expr,ctx),'Unresolved label '+pid);
  const key=source.literal(found.context.id,found.node);
  requireValue(typeof key==='string'&&/^[A-Z][A-Z0-9_]*$/.test(key),'Expected i18n key, not translated object/value in '+pid);
  receipts.push(receipt(found.name,found.node,found.context));
  if(found.node.type==='Identifier'&&found.context.definitions.has(found.node.name))
   receipts.push(receipt(found.name+' literal i18n key',found.context.definitions.get(found.node.name),found.context));
  return key;
 }
 const objects=[];
 walk(candidate.node,node=>{if(node.type==='ObjectExpression')objects.push(new Map(node.properties.filter(p=>p.type==='Property').map(p=>[p.key.name??p.key.value,p.value])));});
 const header=requireValue(objects.find(props=>props.has('title')&&props.has('tips')),'Missing wheel header');
 const modes=requireValue(objects.find(props=>props.has('modeA')&&props.has('modeB')),'Missing two-way modes');
 const levels=objects.filter(props=>props.has('min')&&props.has('max')&&props.has('step')&&props.has('noTip'));
 requireValue(hasLevels?levels.length===2:levels.length===0,'Unexpected level controls '+pid);
 for(const props of levels)requireValue(props.get('min').value===0&&props.get('max').value===4&&props.get('step').value===1,'Changed level range '+pid);
 walk(candidate.node,node=>{
  if(node.type!=='CallExpression'||node.arguments[1]?.type!=='ObjectExpression')return;
  const keys=node.arguments[1].properties.map(p=>p.key?.name);
  if(!keys.includes('modeA')&&!keys.includes('noTip'))return;
  let child=resolve(node.arguments[0],context),seen=new Set();
  while(child&&!seen.has(child.context.file+':'+child.node.start)){
   seen.add(child.context.file+':'+child.node.start);
   receipts.push(receipt(child.name,child.node,child.context));
   if(child.node.type==='Identifier')child=resolve(child.node,child.context);
   else if(child.node.type==='CallExpression'&&child.node.arguments.length===1)child=resolve(child.node.arguments[0],child.context);
   else break;
  }
 });
 const textKeys=[];
 walk(candidate.node,node=>{
  if(node.type!=='CallExpression'||node.arguments[1]?.type!=='ObjectExpression')return;
  const prop=node.arguments[1].properties.find(p=>p.type==='Property'&&(p.key.name??p.key.value)==='text');
  if(prop?.value.type!=='MemberExpression')return;
  const key=label(prop.value);textKeys.push(key);
  receipts.push(receipt('translation text caller '+key,node,context));
 });
 const getKey=key=>{requireValue(textKeys.includes(key),`Missing ${key} in ${pid}`);return key;};
 // Resolve the actual Customize root, including imported exported components.
 // Read the live navigation object directly. The removed historical page
 // inventory is not an implementation input or a fallback.
 const mounts=[];
 for(const file of source.files){
  const text=source.text(file);if(!text.includes('component:')||!text.includes('navs'))continue;
  const ast=asts.get(file)||acorn.parse(text,{ecmaVersion:'latest'}),blocks=[];
  asts.set(file,ast);
  walk(ast,node=>{if(node.type==='BlockStatement'){
   const defs=definitions(node);if(defs.size)blocks.push({file,definitions:defs,start:node.start,end:node.end});
  }});
  walk(ast,node=>{
   if(node.type!=='ObjectExpression')return;
   const props=new Map(node.properties.filter(p=>p.type==='Property').map(p=>[p.key.name??p.key.value,p.value]));
   if(props.get('id')?.type!=='Literal'||!props.has('name')||props.get('component')?.type!=='CallExpression')return;
   for(const ctx of blocks.filter(block=>block.start<=node.start&&block.end>=node.end).sort((a,b)=>(a.end-a.start)-(b.end-b.start))){
    const name=resolve(props.get('name'),ctx);if(!name)continue;
    let key;try{key=source.literal(name.context.id,name.node);}catch{continue;}
    if(key!=='TAB_CUSTOMIZE')continue;
    const component=props.get('component'),entry=component.arguments[0],start=resolve(entry,ctx);
    requireValue(start,'Cannot resolve current Customize root '+pid);
    mounts.push({start,page:{path:file,sha256:hash(text),nav_offset:node.start,component:text.slice(component.start,component.end)},
     navigation:receipt('current TAB_CUSTOMIZE navigation object',node,ctx)});break;
   }
  });
 }
 requireValue(mounts.length===1,'Ambiguous or missing current Customize mount '+pid);
 const {page,start,navigation}=mounts[0];
 receipts.push(navigation);
 requireValue(start,'Cannot resolve Customize root '+pid);
 const queue=[{...start,trail:[]}],seen=new Set(),reached=[];let trail;
 while(queue.length){
  const current=queue.shift(),key=current.context.file+':'+current.node.start;
  if(seen.has(key))continue;seen.add(key);reached.push(current.name);
  const chain=[...current.trail,receipt(current.name,current.node,current.context)];
  if(current.context.file===context.file&&current.node.start===candidate.node.start){trail=chain;break;}
  if(current.node.type==='Identifier'){const next=resolve(current.node,current.context);if(next)queue.push({...next,trail:chain});continue;}
  walk(current.node,node=>{
   if(node.type!=='CallExpression')return;
   const callee=source.text(current.context.file).slice(node.callee.start,node.callee.end);
   if(/\.(?:jsx|jsxs)\b/.test(callee)){
    const next=resolve(node.arguments[0],current.context);if(next)queue.push({...next,trail:chain});
   }else if(!/Class/.test(current.node.type)){
    for(const argument of node.arguments){const next=resolve(argument,current.context);if(next)queue.push({...next,trail:chain});}
   }
  });
 }
 requireValue(trail,'Scroll component not reachable from Customize '+pid+'; reached '+reached.join(', '));
 const css=[];
 for(const relative of [...new Set(Object.values(manifest.files))].filter(file=>file.endsWith('.css'))){
  const file=directory+'/'+relative.slice(2),text=read(file),rules=parseCSS(text).filter(rule=>/^\.scroll(?:[ .:]|$)|twoway-lighting|^\.modes-tab|^\.disabled|^\.slider|^\.switch/.test(rule.selector));
  if(rules.length)css.push({path:file,sha256:hash(text),rules});
 }
 requireValue(css.some(sheet=>sheet.rules.some(rule=>rule.selector==='.twoway-lighting')),'Missing source mode CSS '+pid);
 const cssValue=(selector,property)=>css.flatMap(sheet=>sheet.rules)
  .filter(rule=>!rule.conditions.length&&rule.selector.split(',').includes(selector))
  .flatMap(rule=>rule.properties).filter(prop=>prop.property===property).at(-1)?.value;
 const pixels=(selector,property,optional=false)=>{
  const value=cssValue(selector,property);if(optional&&value===undefined)return 0;
  requireValue(/^\d+(?:\.\d+)?px$/.test(value),'Invalid CSS pixel property '+selector+' '+property);
  return Number(value.slice(0,-2));
 };
 const color=(selector,property)=>{
  const value=cssValue(selector,property);requireValue(/^#[\da-f]{3}(?:[\da-f]{3})?$/i.test(value),'Invalid CSS color '+selector);
  return parseInt(value.length===4?value.slice(1).split('').map(c=>c+c).join(''):value.slice(1),16);
 };
 const pillPadding=requireValue(cssValue('.twoway-lighting .lighting-effect','padding'),'Missing pill padding').split(' ');
 requireValue(pillPadding.length===2&&pillPadding.every(v=>/^\d+px$/.test(v)),'Unknown pill padding');
 const twoModeStyle={background:color('.twoway-lighting','background-color'),text:color('.modes-tab','color'),
  selected_text:color('.modes-tab.active','color'),height:pixels('.twoway-lighting','height'),
  radius:pixels('.twoway-lighting','border-radius'),padding:pixels('.twoway-lighting','padding'),
  pill_height:pixels('.twoway-lighting .lighting-effect','height'),pill_radius:pixels('.twoway-lighting .lighting-effect','border-radius'),
  pill_padding_x:Number(pillPadding[1].slice(0,-2)),pill_padding_y:Number(pillPadding[0].slice(0,-2)),
  message_margin:pixels('.scroll .smartreel .msg','margin-top'),switch_margin:pixels('.scroll .smartreel .switch','margin-left'),
  mode_margin:pixels('.scroll .mode','margin-top',true),twoway_margin:pixels('.scroll .twoway','margin-top'),
  uppercase:modes.get('extraClass').value.split(' ').includes('uppercase')};
 specs.push({product_id:pid,profile_key:'scrollWheel',defaults:product.profile.scrollWheel,
  modes:[{id:'Tactile',label:label(modes.get('modeA'))},{id:'FreeSpin',label:label(modes.get('modeB'))}],
  level_min:0,level_max:hasLevels?4:0,level_step:hasLevels?1:0,max_disabled_modes:0,locking_mode:'',
  presentation:hasLevels?'two_mode_levels':'two_mode_switches',two_mode_style:twoModeStyle,help_key:label(header.get('tips')),
  mode_description:getKey('SCROLL_MODE_DESC'),
  acceleration_descriptions:[getKey(hasLevels?'SCROLL_ACCELERATION_LEVEL_DESC':'SCROLL_ACCELERATION_DESC')],
  smart_reel_descriptions:hasLevels?[getKey('SMART_REEL_DESC'),getKey('SMART_REEL_LEVEL_DESC')]:[getKey('SMART_REEL_DESC')]});
 reports.push({product_id:pid,status:hasLevels?'two_mode_levels':'two_mode_switches',manifest:{path:manifestPath,sha256:hash(read(manifestPath))},
  mounted_page:page.component,mount_chain:trail,receipts,css,
  description_contract:{source:'actual mounted translation component text props resolve to i18n key strings; translated dictionaries are not page keys',
   mode:getKey('SCROLL_MODE_DESC'),acceleration:[getKey(hasLevels?'SCROLL_ACCELERATION_LEVEL_DESC':'SCROLL_ACCELERATION_DESC')],
   smart_reel:hasLevels?[getKey('SMART_REEL_DESC'),getKey('SMART_REEL_LEVEL_DESC')]:[getKey('SMART_REEL_DESC')]},
  behavior:{mode:'Either TwoWay half toggles, including the selected half; vendor dispatch is debounced 300ms',
   smart_reel_lock:hasLevels?'none':'selected source scrollMode FreeSpin; preserve stored enabled bit',
   levels:hasLevels?[0,4,1]:null,disabled_modes:false}});
}
specs.sort((a,b)=>a.product_id-b.product_id);
for(const spec of specs){
 if(Object.hasOwn(spec,'mode_description'))requireValue(typeof spec.mode_description==='string','Scroll mode description must be an i18n string');
 for(const field of ['acceleration_descriptions','smart_reel_descriptions'])if(Object.hasOwn(spec,field))
  requireValue(Array.isArray(spec[field])&&spec[field].every(key=>typeof key==='string'),'Scroll descriptions must be i18n strings: '+field);
}
for(const[file,value]of[['crates/razer-pages/src/features/mouse_scroll_wheel_data.json',specs],['docs/re/mouse-scroll-wheel-current-evidence.json',{method:'Current mounted AST/CSS and CONFIG only; no vendor execution',products:reports}]]){
 const output=JSON.stringify(value,null,2)+'\n';if(check)requireValue(read(file)===output,'Stale '+file);else fs.writeFileSync(path.join(root,file),output);
}
console.log(`Scroll wheel: ${specs.length} source capabilities; ${reports.filter(report=>report.status==='no_scroll_editor_in_current_declared_js').length} profile-only false positives excluded`);
