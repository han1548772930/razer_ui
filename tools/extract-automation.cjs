// Current August T2 automation source: static Acorn parsing only.
const fs=require('fs'),path=require('path'),crypto=require('crypto'),acorn=require('acorn');
const root=path.resolve(__dirname,'..'),read=p=>fs.readFileSync(path.join(root,p),'utf8');
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
const pending=JSON.parse(read('docs/re/pending-product-3946-source.json'));
const file=pending.source_files[0],source=read(file.path);
if(hash(source)!==file.sha256)throw Error('Changed automation source');
const ast=acorn.parse(source,{ecmaVersion:'latest'}),scopes=new WeakMap(),declarations=[],calls=[],modules=new Map();
function visit(n,scope){
  if(!n?.type)return;
  if(/Function/.test(n.type)||n.type==='Program')scope={parent:scope,defs:new Map()};
  scopes.set(n,scope);
  if(n.type==='VariableDeclarator'&&n.id.type==='Identifier'){scope.defs.set(n.id.name,n.init);declarations.push(n);}
  if(n.type==='CallExpression'&&n.callee.property?.name==='d'&&n.arguments[1]?.type==='ObjectExpression')calls.push(n);
  if(n.type==='Property'&&Number.isInteger(n.key.value)&&/Function/.test(n.value.type))modules.set(n.key.value,n.value);
  for(const v of Object.values(n)){if(Array.isArray(v))v.forEach(c=>visit(c,scope));else if(v?.type)visit(v,scope);}
}
visit(ast,null);
function lookup(n){let scope=scopes.get(n);while(scope){if(scope.defs.has(n.name))return scope.defs.get(n.name);scope=scope.parent;}throw Error('Unbound '+n.name);}
function decode(n){
  if(n.type==='Literal')return n.value;
  if(n.type==='Identifier')return decode(lookup(n));
  if(n.type==='MemberExpression'){
   const key=n.computed?decode(n.property):n.property.name;
   if(n.object.type==='Identifier'){
    const binding=lookup(n.object);
    if(binding.type==='CallExpression'&&binding.arguments[0]?.type==='Literal'){
     const module=modules.get(binding.arguments[0].value);
     const exported=calls.find(c=>c.start>=module.start&&c.end<=module.end)?.arguments[1].properties.find(p=>(p.key.name??p.key.value)===key);
     if(exported)return decode(exported.value.body);
    }
   }
   return decode(n.object)[key];
  }
  if(n.type==='CallExpression'&&n.callee.property?.name==='sort')return decode(n.callee.object).sort((a,b)=>a.name.localeCompare(b.name));
  if(n.type==='UnaryExpression'&&n.operator==='!')return !decode(n.argument);
  if(n.type==='BinaryExpression'&&n.operator==='+')return decode(n.left)+decode(n.right);
  if(n.type==='ArrayExpression')return n.elements.map(decode);
  if(n.type==='ObjectExpression')return Object.fromEntries(n.properties.map(p=>[p.computed?decode(p.key):p.key.name??p.key.value,decode(p.value)]));
  throw Error('Nonliteral '+source.slice(n.start,n.end).slice(0,70));
}
const page=pending.pages[0],components=page.components.filter(c=>c.offset>=6658000&&c.offset<6953320&&!(c.offset>=6735000&&c.offset<6909500));
const keys=new Set(components.flatMap(c=>c.jsx.map(x=>x.props.text)).filter(v=>typeof v==='string'));
// Resolve labels used in conditional expressions as well as literal JSX props.
const labelImport=declarations.find(n=>n.id.name==='ze'&&n.start>6000000&&n.start<6100000);
const labelsModule=modules.get(labelImport.init.arguments[0].value);
const labelExports=calls.find(c=>c.start>=labelsModule.start&&c.end<=labelsModule.end);
const labels={};
for(const p of labelExports.arguments[1].properties){try{labels[p.key.name??p.key.value]=decode(p.value.body);}catch{}}
for(const c of components)for(const m of c.source.matchAll(/ze\.([A-Za-z_$][\w$]*)/g))if(labels[m[1]])keys.add(labels[m[1]]);
const kinds=declarations.find(n=>n.id.name==='cH'&&n.start>6935000&&n.end<6935612);
if(!kinds)throw Error('Changed automation categories');
const actions=kinds.init.elements.map(n=>Object.fromEntries(n.properties.filter(p=>p.key.name!=='icon').map(p=>[p.key.name,p.value.type==='MemberExpression'?labels[p.value.property.name]:decode(p.value)])));
const productModule=modules.get(78193),productExports=calls.find(c=>c.start>=productModule.start&&c.end<=productModule.end);
const exported=name=>decode(productExports.arguments[1].properties.find(p=>p.key.name===name).value.body);
const effects=exported('QUICK_EFFECTS'),metadata=exported('DeviceInfo');
const constantsModule=modules.get(13254),constantsExports=calls.find(c=>c.start>=constantsModule.start&&c.end<=constantsModule.end);
const effectDefaults=decode(constantsExports.arguments[1].properties.find(p=>p.key.name==='U2A').value.body);
const quickColorClasses=[];
let colorPickerRender;
const laneMounts=[];
function captureQuickColor(node){
 if(!node?.type)return;
 if(node.type==='ClassDeclaration'&&['_l','vl','Ql','AI','uP','yT'].includes(node.id?.name)&&node.start>6390000&&node.start<6690000)quickColorClasses.push(node);
 if(node.type==='ClassDeclaration'&&node.id?.name==='sl'&&node.start>6400000&&node.start<6430000)colorPickerRender=node.body.body.find(method=>method.key?.name==='render');
 if(node.type==='CallExpression'&&node.start>6695000&&node.start<6696000&&node.arguments[0]?.name==='GP')laneMounts.push(node);
 for(const value of Object.values(node))if(Array.isArray(value))value.forEach(captureQuickColor);else if(value?.type)captureQuickColor(value);
}
captureQuickColor(ast);
const paletteDeclaration=declarations.find(node=>node.id.name==='ZT'&&node.start>6400000&&node.start<6410000);
const paletteNode=paletteDeclaration.init.type==='SequenceExpression'?paletteDeclaration.init.expressions.at(-1):paletteDeclaration.init;
const quickColorPalette=decode(paletteNode);
const sourceColorRendering=components.find(component=>component.source.startsWith('function LP('));
for(const item of quickColorClasses)for(const match of source.slice(item.start,item.end).matchAll(/ze\.([A-Za-z_$][\w$]*)/g))if(labels[match[1]])keys.add(labels[match[1]]);
for(const key of ['COLOR','COLOR_DROP_NAME','RANDOM_COLOR'])keys.add(key);
for(const effect of effects)keys.add(effect.name);
const empty=declarations.find(n=>n.id.name==='lH'&&n.start>6935000&&n.start<6935612);
const empty_messages=Object.fromEntries(empty.init.properties.map(p=>[p.key.value,labels[p.value.properties[0].value.property.name]]));
for(const key of Object.values(empty_messages))keys.add(key);
for(const a of actions)keys.add(a.content);
for(const key of ['PICK_UP','PUT_DOWN','RESTORE_PREVIOUS_SETTINGS','QUICK_EFFECTS','ADVANCED_EFFECTS','CHROMA','AUDIO_DEVICE','MICROPHONE','GLOBAL_SHORTCUT_AUTO','LAUNCH_A_GAME','MACRO','CANCEL','SAVE','ADD','DELETE','PAUSE_GAME_AUTO'])keys.add(key);
const quickMacroTypes=decode(declarations.find(n=>n.id.name==='Jv'&&n.start>6910000&&n.start<6920000).init);
const localeMap=declarations.find(n=>n.init?.type==='ObjectExpression'&&n.init.properties.length===10&&n.init.properties.every(p=>p.value.type==='Identifier')&&n.init.properties.some(p=>p.key.name==='en')&&n.init.properties.some(p=>{try{return (p.computed?decode(p.key):p.key.value)==='zh-CN';}catch{return false;}}));
if(!localeMap)throw Error('Missing locale map');
const translations={};
for(const entry of localeMap.init.properties){
 const locale=entry.computed?decode(entry.key):entry.key.name??entry.key.value,namespace=lookup(entry.value);
 const call=calls.find(c=>{try{return lookup(c.arguments[0])===namespace;}catch{return false;}});
 if(!call)throw Error('Missing locale '+locale);
 translations[locale]={};
 for(const p of call.arguments[1].properties){const key=p.key.name??p.key.value;if(keys.has(key))translations[locale][key]=decode(p.value.body);}
}
const manifestPath='.ref/devices/3946/asset-manifest.json',manifest=JSON.parse(read(manifestPath)),assets=[];
for(const [ix,node] of kinds.init.elements.entries()){
 const icon=node.properties.find(p=>p.key.name==='icon').value;
 const binding=lookup(icon);
 let request;
 if(binding.type==='BinaryExpression'&&binding.right.type==='Literal')request=binding.right.value;
 if(!request){
  // Icons are webpack asset modules imported by an ordinary require call.
  if(binding.type==='CallExpression'&&binding.arguments[0]?.type==='Literal'){
   const module=modules.get(binding.arguments[0].value);
   request=source.slice(module.start,module.end).match(/"(static\/media\/[^" ]+)"/)?.[1];
  }
 }
 if(!request)throw Error('Unresolved automation icon '+source.slice(binding.start,binding.end));
 if(!Object.values(manifest.files).some(p=>p.replace(/^\.\//,'')===request))throw Error('Undeclared icon');
 assets.push({source:'.ref/devices/3946/'+request,url:'https://apps.razer.com/synapse/products/3946/ui/'+request,output:`assets/synapse/automation-${ix}.svg`});
}
for(const name of ['icon_add_light_grey','icon_delete','tooltip_questionmark','icon_warning','thx_spatial_audio_logo','logo-7.1','icon_folder','icon_expand','icon_close','icon_direction_up_999','icon_direction_up','icon_direction_down_999','icon_direction_down']){
 const request=Object.values(manifest.files).find(v=>new RegExp('/'+name+'\\.[a-f0-9]+\\.svg$').test(v))?.replace(/^\.\//,'');
 if(!request)throw Error('Missing auxiliary icon '+name);
 assets.push({source:'.ref/devices/3946/'+request,url:'https://apps.razer.com/synapse/products/3946/ui/'+request,output:`assets/synapse/automation-${name}.svg`});
}
// Convert only literal JSX SVG props. No downloaded JavaScript is evaluated.
const macroIcon=declarations.find(n=>n.id.name==='tH'&&n.start>6917000&&n.end<6920390).init;
const escapeXml=value=>String(value).replace(/&/g,'&amp;').replace(/"/g,'&quot;').replace(/</g,'&lt;').replace(/>/g,'&gt;');
function literalSvg(node,size=20){
 if(node.type!=='CallExpression'||node.arguments[0]?.type!=='Literal'||node.arguments[1]?.type!=='ObjectExpression')throw Error('Changed quick macro JSX');
 const tag=node.arguments[0].value;
 if(!['svg','path'].includes(tag))throw Error('Unexpected quick macro SVG node');
 const props=node.arguments[1].properties,attributes=[],children=[];
 for(const p of props){
  const key=p.key.name??p.key.value;
  if(key==='children'){children.push(literalSvg(p.value,size));continue;}
  if(key==='width'||key==='height'){attributes.push(`${key}="${size}"`);continue;}
  if(p.value.type!=='Literal')throw Error('Nonliteral quick macro SVG attribute');
  attributes.push(`${key}="${escapeXml(p.value.value)}"`);
 }
 return `<${tag} ${attributes.join(' ')}>${children.join('')}</${tag}>`;
}
for(const branch of macroIcon.body.body.find(n=>n.type==='SwitchStatement').cases){
 const statement=branch.consequent.find(n=>n.type==='ReturnStatement'&&n.argument?.type==='CallExpression');
 if(!statement||!branch.test)continue;
 const kind=branch.test.value,svg=literalSvg(statement.argument)+'\n';
 assets.push({source:file.path,source_offset:statement.argument.start,source_end:statement.argument.end,source_fragment:source.slice(statement.argument.start,statement.argument.end),inline_svg:svg,output:`assets/synapse/automation-quick-macro-${kind}.svg`});
}
const macroDelete=declarations.find(n=>n.id.name==='eH'&&n.init?.start===6915402).init;
const macroDeleteSvg=macroDelete.body.body.find(n=>n.type==='ReturnStatement').argument;
assets.push({source:file.path,source_offset:macroDeleteSvg.start,source_end:macroDeleteSvg.end,
 source_fragment:source.slice(macroDeleteSvg.start,macroDeleteSvg.end),inline_svg:literalSvg(macroDeleteSvg,24)+'\n',
 output:'assets/synapse/automation-quick-macro-delete.svg'});
const css=[];
for(const f of fs.readdirSync(path.join(root,'.ref/devices/3946/static/css')).filter(f=>f.endsWith('.css'))){
 const p='.ref/devices/3946/static/css/'+f,s=read(p),rules=s.split('}').filter(r=>/automation-|AutomationModal_|LaunchSoundApp_|quick-macro|macro-type-select|macro-keyboard|macro-pill|effects-area|color-opts|dropdown-color|random-color|toggle-btn|modes-tab|dir-up|dir-down|stepper|slider-container|range-slider|^\.slider[:{]|^\.foot\./.test(r.split('{')[0])||r.startsWith('.slider{')).map(r=>r+'}');
 if(rules.length)css.push({path:p,sha256:hash(s),rules});
}
function emit(p,v){const s=JSON.stringify(v,null,2)+'\n';if(process.argv.includes('--check')){if(read(p)!==s)throw Error('Stale '+p);}else fs.writeFileSync(path.join(root,p),s);}
emit('src/features/automation_data.json',{product_id:3946,page:page.key,metadata,effects,effect_defaults:effectDefaults,empty_messages,actions,quick_macro_types:quickMacroTypes,quick_color_palette:quickColorPalette,translations,assets});
const profile_bar=[];
for(const needle of ['setProfileDropdownState','enableSwitchProfile:','displayProfileBar:','isEnableProfileBar:'])for(const match of source.matchAll(new RegExp(needle,'g')))profile_bar.push({needle,offset:Math.max(0,match.index-120),source:source.slice(Math.max(0,match.index-120),match.index+350)});
const profile_bar_labels=Object.fromEntries(['tPc','_$r','lgc'].map(k=>[k,labels[k]]));
const persistence=[];
for(const needle of ['this.loadActiveProfileSettings=async()=>','case le.nI5:','case le.jy3:','case le.c24:']){
 const offset=source.indexOf(needle);if(offset<0)throw Error('Missing persistence anchor '+needle);
 persistence.push({needle,offset,source:source.slice(offset,offset+(needle.startsWith('this.')?2800:550))});
}
const automation_actions=Object.fromEntries(['jy3','c24','nI5','HTo'].map(k=>[k,decode(constantsExports.arguments[1].properties.find(p=>p.key.name===k).value.body)]));
const fragment=node=>({offset:node.start,end:node.end,source:source.slice(node.start,node.end)});
const quick_color_parameters={palette:fragment(paletteNode),components:quickColorClasses.map(node=>({name:node.id.name,...fragment(node)})),mounted_by:sourceColorRendering,
 color_picker_render:fragment(colorPickerRender),lane_mounts:laneMounts.map(fragment),
 lane_connector:fragment(declarations.find(node=>node.id.name==='GP'&&node.start>6693000&&node.start<6694000)),
 number_input:{module:44230,...fragment(modules.get(44230))},
 parameter_labels:Object.fromEntries(['arT','XE0','rwE','LS','mQh','vvC','G2f','WLd'].map(key=>[key,labels[key]])),
 aliases:declarations.filter(node=>['NP','dI','Jl','CP','II','lI','RP','GT'].includes(node.id.name)&&node.start>6400000&&node.start<6690000).map(node=>({name:node.id.name,...fragment(node)}))};
emit('docs/re/automation-current-evidence.json',{method:'Acorn literals and mounted current source; vendor code never executed',generator_sha256:hash(fs.readFileSync(__filename)),source_files:pending.source_files,profile_bar,profile_bar_labels,persistence,automation_actions,quick_color_parameters,manifest:{path:manifestPath,sha256:hash(read(manifestPath))},actions:{offset:kinds.start,end:kinds.end,source:source.slice(kinds.start,kinds.end)},css,components});
console.log('Extracted August T2 automation categories, localized labels and artwork.');
