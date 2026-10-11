// Statically read current product bundles; never evaluate vendor JavaScript.
const fs=require('fs'),crypto=require('crypto'),acorn=require('acorn');
const receipts=[];
const nativeProducts=JSON.parse(fs.readFileSync('crates/razer-pages/src/features/keyboard_products_data.json','utf8'));
const nativeData=[];
const controllerData=[];
const key=node=>node?.name??node?.value;
function walk(node,visit){if(!node?.type||visit(node)===false)return;for(const value of Object.values(node)){if(Array.isArray(value))for(const child of value)walk(child,visit);else if(value?.type)walk(value,visit)}}
function moduleExports(source,id){
 const head=new RegExp('(?:^|[,\\{])'+id+':').exec(source);if(!head)return null;
 let mod=acorn.parseExpressionAt(source,head.index+head[0].length,{ecmaVersion:'latest'});if(mod.type==='SequenceExpression')mod=mod.expressions[0];
 const vars=new Map(),exports=[];
 walk(mod.body,node=>{if(node!==mod.body&&/Function|Class/.test(node.type))return false;if(node.type==='VariableDeclarator'&&node.id.type==='Identifier')vars.set(node.id.name,node.init);if(node.type==='CallExpression'&&key(node.callee.property)==='d'&&node.arguments[1]?.type==='ObjectExpression')for(const prop of node.arguments[1].properties){const resolved=prop.value.body?.type==='Identifier'?vars.get(prop.value.body.name):prop.value.body;exports.push({name:key(prop.key),symbol:prop.value.body?.name,resolved})}});
 // d() export declarations precede the variables in Webpack modules.
 for(const exported of exports)if(!exported.resolved&&exported.symbol)exported.resolved=vars.get(exported.symbol);
 return {id,byte_offset:Buffer.byteLength(source.slice(0,mod.start)),char_range:[mod.start,mod.end],exports:exports.map(({resolved,...exported})=>({...exported,byte_offset:resolved?Buffer.byteLength(source.slice(0,resolved.start)):null,source:resolved?source.slice(resolved.start,resolved.end):null})),vars,mod};
}
function literal(node){
 if(node?.type==='Literal')return node.value;
 if(node?.type==='ArrayExpression')return node.elements.map(literal);
 if(node?.type==='ObjectExpression')return Object.fromEntries(node.properties.map(prop=>[key(prop.key),literal(prop.value)]));
 if(node?.type==='UnaryExpression'&&node.operator==='!')return !literal(node.argument);
 if(node?.type==='UnaryExpression'&&node.operator==='-')return -literal(node.argument);
 if(node?.type==='CallExpression'&&node.callee.name==='parseInt')return Number.parseInt(literal(node.arguments[0]),literal(node.arguments[1]));
 throw Error('Unresolved static keyboard literal '+node?.type);
}
for (const pid of process.argv.slice(2).map(Number).filter(Number.isFinite)) {
 const root=`local-ui-reverse/source/official/apps.razer.com/synapse/products/${pid}/ui/static`;
 const jsName=fs.readdirSync(root+'/js').find(name=>name.startsWith('main.'));
 const cssName=fs.readdirSync(root+'/css').find(name=>name.startsWith('main.'));
 const js=fs.readFileSync(root+'/js/'+jsName,'utf8'),css=fs.readFileSync(root+'/css/'+cssName,'utf8');
 const markers=[...js.matchAll(/className:"[^"\r\n]*(?:drawer|button-panel|mapping-panel)[^"\r\n]*"|this\.(?:displaySaveAlert|saveMapping|cancelMapping|handleSave|handleCancel|toggleButtonPanel|updateToggleButtonPanel|setMappingList|updateMappingChanged)=/g)].map(match=>({marker:match[0],byte_offset:Buffer.byteLength(js.slice(0,match.index)),char_offset:match.index,text:js.slice(Math.max(0,match.index-180),match.index+2200)}));
 for(const marker of ['config-drawer','drawer-top','drawer-btn','close-drawer','setSaveRef:this.setSaveRef','this.saveMap=','this.cancelMap=','saveAlertOpen:this.state.saveAlertOpen','ON_SET_KEYMAPPING','"REMOVE_ANALOG_EVENT_CALLBACK"']) {
  let offset=-1;
  while((offset=js.indexOf(marker,offset+1))>=0)markers.push({marker,byte_offset:Buffer.byteLength(js.slice(0,offset)),char_offset:offset,text:js.slice(Math.max(0,offset-800),offset+2600)});
 }
 const rules=[...css.matchAll(/[^{}]*(?:drawer|button-panel|mapping-panel|key-config|keymap-head|keymap-action|two-tap-key-mapping)[^{}]*\{[^}]*\}/g)].map(match=>({byte_offset:Buffer.byteLength(css.slice(0,match.index)),text:match[0]}));
 const chunks=fs.readdirSync(root+'/js').filter(name=>/^(?:ButtonPanelComponent\.|TwoTapKeyMapping\.|2383\.|Map[^.]*\.)/.test(name)).map(name=>{
  const source=fs.readFileSync(root+'/js/'+name,'utf8');
  const nodes=[...source.matchAll(/this\.\w*(?:[sS]ave|[cC]ancel|[dD]ismiss|[sS]ubmit|[cC]lose|[sS]etMapping|[uU]pdateMapping)\w*=|setSaveRef\(|setSaveRef:|ON_SET_KEYMAPPING|config-drawer|drawer-btn|this\.getMapping[\w]*=|this\.setMapping[\w]*=|this\.getDisplayFunctionList=/g)].map(match=>({marker:match[0],byte_offset:Buffer.byteLength(source.slice(0,match.index)),char_offset:match.index,text:source.slice(Math.max(0,match.index-150),match.index+4200)}));
  return {source:root+'/js/'+name,sha256:crypto.createHash('sha256').update(source).digest('hex'),nodes};
 });
 const specimen=nativeProducts.find(product=>product.product_id===pid);
 const jsHash=crypto.createHash('sha256').update(js).digest('hex');
 const fullProduct=JSON.parse(fs.readFileSync(`assets/synapse/keyboard-products/${pid}.json`,'utf8'));
 if(!fullProduct.source_files.some(file=>file.sha256===jsHash))throw Error('Current source hash disagrees with extracted source groups '+pid);
 const fields=['buttonKey','inputID','inputType','counter','defaultValue','isEnabled','disabled','isSidePanelList','isKeyToggle','disableHypershiftMapping','HID','assignment','assignmentValue','functionList','buttonPos','unRemappable','localizedName','MappingGroup','isDisabledLeftTitle','disableTurboInAssignment'];
 const buttons=groups=>(groups?.[0]?.group?.buttonList??[]).map(button=>Object.fromEntries(fields.filter(field=>Object.prototype.hasOwnProperty.call(button,field)).map(field=>[field,button[field]])));
 const keyModule=moduleExports(js,46114),keyExport=keyModule.exports.find(exported=>exported.name==='Rk');
 const keyTargets=literal(acorn.parseExpressionAt(keyExport.source,0,{ecmaVersion:'latest'}));
 const helperModule=moduleExports(js,29267),groupModule=moduleExports(js,69937);
 const labels=moduleExports(js,54693);
 const read=(module,name)=>literal(acorn.parseExpressionAt(module.exports.find(exported=>exported.name===name).source,0,{ecmaVersion:'latest'}));
 const device=specimen.config.DeviceInfo;
 const mouse=read(helperModule,'ir').concat(read(helperModule,'Qh').filter(choice=>device.extendSupportMappings?.mouseGroup?.includes(choice.id)));
 if(device.supportsBossKey)mouse.push(...read(helperModule,'Vx'));
 if(device.supportScrollModeToggle)throw Error('Audit per-input ScrollModeToggle capability before adding choices '+pid);
 const windows=read(helperModule,'Zi').concat(device.isAdvancedMultiDial?read(helperModule,'rH'):[],specimen.config.ADDITIONAL_WINDOW_SHORTCUTS??[]);
 const options={MOUSE_FUNCTION:{group:read(groupModule,'Ir9'),field:'mouseAssignment',choices:mouse},MULTIMEDIA:{group:read(groupModule,'CLP'),field:'multimediaAssignment',choices:specimen.config.controllerMediaAssignments?.length?specimen.config.controllerMediaAssignments:read(helperModule,'hx')},WINDOWS_SHORTCUT:{group:read(groupModule,'G7s'),field:'windowsShortcutAssignment',choices:windows},DEVICE_BRIGHTNESS:{group:read(groupModule,'s_$'),field:'backlightAssignment',choices:read(helperModule,'oP')}};
 const controllerArray=name=>acorn.parseExpressionAt(helperModule.exports.find(exported=>exported.name===name).source,0,{ecmaVersion:'latest'}).elements.map(row=>({content:literal(row.properties.find(prop=>key(prop.key)==='content').value)}));
 let modes=acorn.parseExpressionAt(groupModule.exports.find(exported=>exported.name==='wpw').source,0,{ecmaVersion:'latest'});
 if(modes.type==='SequenceExpression')modes=modes.expressions.at(-1);
 const controllerModes=modes.elements.map(row=>Object.fromEntries(row.properties.map(prop=>[key(prop.key),prop.value.type==='MemberExpression'?read(labels,key(prop.value.property)):literal(prop.value)])));
 controllerData.push({product_id:pid,source:root+'/js/'+jsName,source_sha256:jsHash,controller:{analog:controllerArray('$T'),digital:controllerArray('Fk'),modes:controllerModes,graph:device.analogSpecs.controllerGraph,without_synapse:device.controllerAssignmentsWithoutSynapse??[],disable_analog:device.disableControllerAnalogMapping===true,heading:read(labels,'lmA'),placeholder:read(labels,'x3E')}});
 const productData={product_id:pid,source:root+'/js/'+jsName,source_sha256:jsHash,default_buttons:buttons(fullProduct.default_groups?.groups),layouts:fullProduct.layouts.map(layout=>({layout_id:layout.layout_id,buttons:buttons(layout.groups)})),key_targets:keyTargets,options,no_turbo:read(groupModule,'LrF')};
 nativeData.push(productData);
 const exports=[29267,46114,69937,54693,13254,61350,84350].map(id=>moduleExports(js,id)).filter(Boolean).map(({vars,mod,...module})=>module);
 receipts.push({pid,js:root+'/js/'+jsName,js_sha256:jsHash,css:root+'/css/'+cssName,css_sha256:crypto.createHash('sha256').update(css).digest('hex'),markers,rules,chunks,native_keys:specimen.keys,exports});
}
fs.mkdirSync('.work/keyboard-mapping-drawer',{recursive:true});
fs.writeFileSync('.work/keyboard-mapping-drawer/receipts.json',JSON.stringify(receipts,null,2));
if(process.argv.includes('--prepare')){
 fs.writeFileSync('crates/razer-pages/src/features/keyboard_mapping_drawer_data.json',JSON.stringify(nativeData));
 fs.writeFileSync('crates/razer-pages/src/features/keyboard_mapping_controller_data.json',JSON.stringify(controllerData));
}
for(const {pid,markers,rules,chunks} of receipts)console.log(JSON.stringify({pid,markers:markers.map(({text,...marker})=>marker),chunks:chunks.map(({nodes,...chunk})=>({...chunk,nodes:nodes.map(({text,...node})=>node)}))}));
