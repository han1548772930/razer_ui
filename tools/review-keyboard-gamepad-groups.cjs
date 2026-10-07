// Parse current source as data; this tool never loads vendor modules or DLLs.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {subtree,componentAst,walk,hash}=require('./current-page-subtree.cjs');
const root=path.resolve(__dirname,'..'),check=process.argv.includes('--check');
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
const keyboards=JSON.parse(read('docs/re/keyboard-product-pages.json')).products;
const products=JSON.parse(read('docs/re/source-product-pages.json')).products;
const gamepads=JSON.parse(read('src/features/gamepad_products_data.json'));
const key=n=>n?.name??n?.value;
function expression(source,offset){const a=acorn.parseExpressionAt(source,offset,{ecmaVersion:'latest'});return a.type==='SequenceExpression'?a.expressions[0]:a;}
function navigation(page){
 const file=page.nav_path??page.path,source=read(file);
 if(hash(source)!==(page.nav_sha256??page.sha256))throw Error('Changed navigation '+file);
 const node=expression(source,page.nav_offset);
 const mounted=node.properties?.find(p=>['renderComponent','component'].includes(key(p.key)))?.value;
 if(!mounted||source.slice(mounted.start,mounted.end)!==page.component)throw Error('Changed page navigation '+file+':'+page.key);
 return {path:file,sha256:hash(source),offset:node.start,end:node.end,source:source.slice(node.start,node.end)};
}
function resolve(page){
 const nav=navigation(page);
 if(!page.lazy_module)return {page,nav,root:/\.jsx(?:s)?\)\(([^,]+)/.exec(page.component)?.[1]};
 // Re-resolve the current lazy module's default export, independently of the
 // old extraction's empty JSX arrays and component ordering.
 const source=read(page.path);
 if(hash(source)!==page.sha256)throw Error('Changed lazy file '+page.path);
 const head=new RegExp('(?:^|[,{])'+page.lazy_module+':').exec(source);
 if(!head)throw Error('Missing current lazy module');
 const module=expression(source,head.index+head[0].length),exports=[];
 walk(module.body,n=>{
  if(n.type==='CallExpression'&&key(n.callee.property)==='d'&&n.arguments[1]?.type==='ObjectExpression')
   for(const p of n.arguments[1].properties)if(key(p.key)==='default'&&p.value.body?.type==='Identifier')exports.push(p.value.body.name);
 });
 if(exports.length!==1)throw Error('Ambiguous current default export');
 const components=page.components.map(c=>{
  if(source.slice(c.offset,c.end)!==c.source)throw Error('Stale lazy component '+c.symbol);
  const jsx=[];walk(componentAst(c),n=>{
   if(n.type==='CallExpression'&&n.callee.type==='SequenceExpression'&&['jsx','jsxs'].includes(key(n.callee.expressions.at(-1)?.property))&&n.arguments[0]?.type==='Identifier')jsx.push({component:n.arguments[0].name});
  });
  return {...c,jsx};
 });
 return {page:{...page,components},nav,root:exports[0],lazy_module:{id:page.lazy_module,path:page.path,sha256:page.sha256,offset:module.start,end:module.end,default_export:exports[0]}};
}
function mounted(resolved,target){
 const result=subtree(resolved.page,target,resolved.root);
 if(!result)throw Error('No current mount '+resolved.page.path+':'+target.symbol);
 return result;
}
const keyboardRows=[];
for(const product of keyboards)for(const page of product.pages.filter(p=>['ACTUATION','OLED','TAB_CALIBRATION'].includes(p.key))){
 const r=resolve(page),rootComponent=r.page.components.find(c=>c.symbol===r.root);
 if(!rootComponent)throw Error('Missing special-page root');
 const row={product_id:product.product_id,page_key:page.key,navigation:r.nav,root:mounted(r,rootComponent),subtrees:[],review:'root-only; controls pending individual review',completed:false};
 if(r.lazy_module)row.lazy_module=r.lazy_module;
 if(page.key==='ACTUATION'){
  const targets=r.page.components.filter(c=>c.source.includes('syncRapidTriggerKeys'));
  if(targets.length!==1)throw Error('Ambiguous rapid trigger editor '+product.product_id);
  row.subtrees.push({kind:'rapid-trigger-editor',...mounted(r,targets[0])});
  row.review='mounted rapid-trigger editor reviewed as a confirmed missing native UI; remaining controls pending';
  row.native='src/features/keyboard_actuation.rs provides initial actuation-point editing; it is not the full source page.';
  row.remaining=['Rapid Trigger editor and source-specific Snap Tap controls','Vertical actuation presentation, mixed-selection rules and whole-page reset confirmation','Real sensor/adjustment observation and full UI/input acceptance'];
 }else if(page.key==='OLED'){
  row.native='src/features/source_controls/oled_page.rs and OLED editor modules implement local controls, previews and Apply drafts; root receipt does not revalidate their behavior.';
  row.remaining=['Review each current home card and preset/editor flow against the already implemented modules','Language download and live device observation branches; native tooltip/layout parity','End-to-end input, focus, animation and original visual acceptance'];
 }else{
  row.native='src/features/keyboard_calibration.rs contains the existing 740/746 page and modal; this batch verifies only the current page root.';
  row.remaining=['Re-audit all modal states, key status rendering, cancellation/profile-change lifecycle and factory-profile conditions','Real calibration service state observation; no simulated live success','Full layout/input/animation acceptance'];
 }
 keyboardRows.push(row);
}
const gamepadRows=[];
for(const spec of gamepads){
 const product=products.find(p=>p.product_id===spec.product_id);
 for(const pageKey of spec.pages){
  const page=product?.pages.find(p=>p.key===pageKey);
  if(!page)throw Error('Missing native gamepad source page '+spec.product_id+':'+pageKey);
  const r=resolve(page),rootComponent=r.page.components.find(c=>c.symbol===r.root);
  if(!rootComponent)throw Error('Missing gamepad root '+spec.product_id+':'+pageKey);
  const row={product_id:spec.product_id,product_name:spec.name,page_key:pageKey,navigation:r.nav,root:mounted(r,rootComponent),subtrees:[],review:'root-only; controls pending individual review',completed:false};
  if(pageKey==='TRIGGERS'){
   const targets=page.components.filter(c=>c.source.includes('rangeSlider')&&c.source.includes('onMouseUp'));
   if(targets.length!==1)throw Error('Ambiguous trigger slider '+spec.product_id);
   row.subtrees.push({kind:'dual-thumb-trigger-range',...mounted(r,targets[0])});
   row.review='current mounted range subtree reverified; prior pointer lifecycle correction remains partial';
   row.remaining=['Re-audit remaining per-product trigger controls, mode gating and reset layout','Native range visual/input acceptance and real trigger observation'];
   row.prior_evidence='docs/re/gamepad-trigger-lifecycle-current-evidence.json';
  }else if(pageKey==='TAB_CALIBRATION'){
   const split=[2676,2684].includes(spec.product_id);
   const targets=page.components.filter(c=>c.source.includes('calibration-wrapper-v2')||c.source.includes('trigger-calibration-popup__'));
   if(targets.length!==(split?2:1))throw Error('Unexpected calibration family '+spec.product_id);
   for(const target of targets)row.subtrees.push({kind:target.source.includes('trigger-calibration-popup__')?'trigger-calibration-popup':split?'per-side-thumbstick-calibration-popup':'thumbstick-calibration-wizard',...mounted(r,target)});
   row.review='current calibration subtree reviewed as a confirmed native UI gap';
   row.native='src/features/gamepad_products.rs::calibration currently has a description, two disabled buttons and an unavailable-service note.';
   row.remaining=split?
    ['Per-side thumbstick popup: selection, step progress, source artwork/simulator, error and cancellation states','Separate trigger calibration popup: meter, timer, step/error states and source artwork','Real state/query integration without fabricated samples or success; DLL writeback remains deferred']:
    ['Source warning/confirmation and multistep thumbstick wizard, original artwork, simulator and step indicator','Product-specific directions, progress, cancellation and error presentation','Real state/query integration without fabricated samples or success; DLL writeback remains deferred'];
   if(spec.product_id===2636){
    row.review='current five-step thumbstick UI implemented with local start/stop/retry intentions and a scoped genuine-observation seam; static verification only';
    row.native_state='partial';
    row.native='src/features/gamepad_calibration.rs and gamepad_calibration_state.rs: original edition artwork, SVG stepper, input visualization, direction/LB overlays, rotation gating, source error modal, cancellation and retry; no hardware writes.';
    row.prior_evidence='docs/re/gamepad-2636-calibration-current-evidence.json';
    row.remaining=['Connect current-source read/query observations; no real tester/calibration data has been supplied or executed','Viewport scaling, product-contained error placement and native visual/input acceptance remain unverified','DLL calibration mutation/writeback remains deferred; local intentions never acknowledge device calibration'];
   }
  }else{
   row.remaining=pageKey==='THUMBSTICKS'?['Individually compare existing deadzone, sensitivity/curve, mode and product-specific controls','Source gating, local draft actions and real stick observation remain unaccepted']:
    pageKey==='TAB_CUSTOMIZE'?['Individually compare current mappings, product artwork/hotspots and all product-specific controls','Profile/local edit flows and real state observation remain unaccepted']:
    pageKey==='TAB_LIGHTING'?['Individually compare current effect controls, color/preset editing, selection and reset behavior','Source state gating and local editing/Apply semantics remain unaccepted']:
    ['Individually compare power saving controls, source ranges/units and charging/battery state conditions','Local actions and real state observation remain unaccepted'];
  }
  if(spec.product_id===4115)row.product_note='Kitsune arcade controller: keep its Customize/Lighting review separate; current native inventory has no trigger or calibration page.';
  if(spec.product_id===2650)row.product_note='Current native inventory has no trigger page; do not synthesize one from the other controllers.';
  gamepadRows.push(row);
 }
}
const summary={keyboard_special_pages:keyboardRows.length,actuation_rapid_editor_gaps:keyboardRows.filter(r=>r.page_key==='ACTUATION').length,
 oled_root_only:keyboardRows.filter(r=>r.page_key==='OLED').length,keyboard_calibration_root_only:keyboardRows.filter(r=>r.page_key==='TAB_CALIBRATION').length,
 gamepad_products:gamepads.length,gamepad_pages:gamepadRows.length,trigger_subtrees:gamepadRows.filter(r=>r.page_key==='TRIGGERS').length,
 calibration_ui_gaps:gamepadRows.filter(r=>r.page_key==='TAB_CALIBRATION'&&r.native_state!=='partial').length,calibration_ui_partial:gamepadRows.filter(r=>r.page_key==='TAB_CALIBRATION'&&r.native_state==='partial').length,calibration_subtrees:gamepadRows.filter(r=>r.page_key==='TAB_CALIBRATION').reduce((n,r)=>n+r.subtrees.length,0),
 completed_pages:0,completed_products:0};
const inputs=['docs/re/keyboard-product-pages.json','docs/re/source-product-pages.json','src/features/gamepad_products_data.json'].map(file=>({path:file,sha256:hash(read(file))}));
const output={date:'2026-10-07',scope:'Grouped current-source review queue. Root evidence is navigation coverage only; it never means UI completion.',method:'Current file SHA-256, actual navigation AST, component UTF-16 slices/AST and root/alias/connect/JSX mount paths; vendor code never executed',inputs,summary,keyboard_rows:keyboardRows,gamepad_rows:gamepadRows};
const destination='docs/re/keyboard-gamepad-groups-review-2026-10-07.json',text=JSON.stringify(output,null,2)+'\n';
if(check){if(read(destination)!==text)throw Error('Stale '+destination);}else fs.writeFileSync(path.join(root,destination),text);
console.log(JSON.stringify(summary));
