// Mounted TwoTap/12383/Y audit; Acorn only, never execute vendor JavaScript.
const fs=require('fs'),crypto=require('crypto'),acorn=require('acorn');
const key=n=>n?.name??n?.value,hash=s=>crypto.createHash('sha256').update(s).digest('hex');
function walk(n,visit){if(!n?.type||visit(n)===false)return;for(const v of Object.values(n))for(const c of Array.isArray(v)?v:v?.type?[v]:[])walk(c,visit);}
function module(s,id){const h=new RegExp('(?:^|[,\\{])'+id+':').exec(s);if(!h)throw Error('Missing module '+id);let n=acorn.parseExpressionAt(s,h.index+h[0].length,{ecmaVersion:'latest'});if(n.type==='SequenceExpression')n=n.expressions[0];const vars=new Map(),exports=new Map();walk(n.body,x=>{if(x!==n.body&&/Function|Class/.test(x.type))return false;if(x.type==='VariableDeclarator'&&x.id.type==='Identifier')vars.set(x.id.name,x.init);if(x.type==='CallExpression'&&key(x.callee.property)==='d'&&x.arguments[1]?.type==='ObjectExpression')for(const p of x.arguments[1].properties)exports.set(key(p.key),p.value.body);});return{node:n,vars,exports,get(name){let x=exports.get(name);return x?.type==='Identifier'?vars.get(x.name):x;}};}
function literal(n){if(n?.type==='Literal')return n.value;if(n?.type==='ArrayExpression')return n.elements.map(literal);if(n?.type==='ObjectExpression')return Object.fromEntries(n.properties.map(p=>[key(p.key),literal(p.value)]));if(n?.type==='UnaryExpression'&&n.operator==='!')return !literal(n.argument);if(n?.type==='UnaryExpression'&&n.operator==='-')return -literal(n.argument);throw Error('Nonliteral '+n?.type);}
function uniqueFile(directory,prefix){const matches=fs.readdirSync(directory).filter(f=>f.startsWith(prefix));if(matches.length!==1)throw Error(`Expected one ${directory}/${prefix}, found ${matches.length}`);return directory+'/'+matches[0];}
function inlineSync(main){
 const tree=acorn.parse(main,{ecmaVersion:'latest'}),matches=[],stack=[];
 function visit(n){
  if(!n?.type)return;
  // A numbered webpack module cannot own the inline application-entry reducer.
  if(n.type==='Property'&&Number.isInteger(n.key.value)&&['ArrowFunctionExpression','FunctionExpression'].includes(n.value.type))return;
  stack.push(n);
  if(n.type==='VariableDeclarator'&&n.id.type==='Identifier'&&['ArrowFunctionExpression','FunctionExpression'].includes(n.init?.type)){
   const text=main.slice(n.init.start,n.init.end);
   if(text.includes('disableActuationMappings')&&text.includes('ON_SET_KEYMAPPING')&&text.includes('ON_SYNC_GLOBAL_ACTUATION')){
    const owner=[...stack].reverse().find(p=>p.type==='CallExpression'&&['ArrowFunctionExpression','FunctionExpression'].includes(p.callee.type));
    if(!owner)throw Error(`Sync helper ${n.id.name} is not owned by an inline entry IIFE`);
    matches.push({declaration:n,node:n.init,name:n.id.name,owner});
   }
  }
  for(const v of Object.values(n))for(const c of Array.isArray(v)?v:v?.type?[v]:[])visit(c);
  stack.pop();
 }
 visit(tree);
 if(matches.length!==1)throw Error(`Expected one inline Sync helper, found ${matches.length}`);
 const result=matches[0],dispatch=[];
 walk(result.owner.callee.body,n=>{if(n.type==='SwitchCase'&&n.consequent.some(c=>c.type==='ReturnStatement'&&c.argument?.type==='CallExpression'&&c.argument.callee.name===result.name))dispatch.push(n);});
 if(dispatch.length!==1)throw Error(`Missing or ambiguous source Sync dispatch for ${result.name}`);
 result.dispatch=dispatch[0];
 const importNames=new Set();
 walk(result.node,n=>{if(n.type==='MemberExpression'&&['hF','aC'].includes(key(n.property))&&n.object.type==='Identifier')importNames.add(n.object.name);});
 if(importNames.size!==1)throw Error('Unresolved Sync mapping-helper aliases');
 const alias=[...importNames][0],imports=[];
 walk(result.owner.callee.body,n=>{if(n.type==='VariableDeclarator'&&n.id.name===alias&&n.init?.type==='CallExpression'&&n.init.arguments[0]?.value===3342)imports.push(n);});
 if(imports.length!==1)throw Error(`Missing or ambiguous ${alias}→3342 Sync helper import`);
 result.import=imports[0];return result;
}
if(process.argv.includes('--self-check')){
 const valid='(()=>{const Me=n(3342);let Qa=(e,t)=>{const x=t.disableActuationMappings;send("ON_SET_KEYMAPPING");send("ON_SYNC_GLOBAL_ACTUATION");return Me.aC(Me.hF(x))};function reducer(e,t){switch(t.type){case SYNC:return Qa(e,t)}}})()';
 if(inlineSync(valid).name!=='Qa')throw Error('Valid inline fixture failed');
 const cases=[valid.replace('ON_SYNC_GLOBAL_ACTUATION','MISSING_EVENT'),valid.replace('return Qa(e,t)','return other(e,t)'),valid.replace('n(3342)','n(99999)'), '({99857:(()=>{let Qa=(e,t)=>{const x=t.disableActuationMappings;send("ON_SET_KEYMAPPING");send("ON_SYNC_GLOBAL_ACTUATION");return x}})})'];
 for(const fixture of cases){let failed=false;try{inlineSync(fixture)}catch{failed=true}if(!failed)throw Error('Invalid inline fixture was silently accepted');}
 console.log(JSON.stringify({valid_inline_fixture:1,rejected_missing_event_dispatch_import_or_numeric_module:cases.length,executed_vendor_javascript:false}));process.exit(0);
}
const records=[],native=[];
for(const pid of [678,679,688]){
 const root=`local-ui-reverse/source/official/apps.razer.com/synapse/products/${pid}/ui/static/`,mainPath=uniqueFile(root+'js','main.'),main=fs.readFileSync(mainPath,'utf8');
 const row={product_id:pid,main:{path:mainPath,sha256:hash(main)},files:[],helpers:[],css:[]};
 const receipt=(s,n)=>{if(!n||!Number.isInteger(n.start)||!Number.isInteger(n.end))throw Error(`Missing required source receipt for ${pid}`);return{byte_offset:Buffer.byteLength(s.slice(0,n.start)),char_range:[n.start,n.end],utf16_range:[n.start,n.end],slice_sha256:hash(s.slice(n.start,n.end)),source:s.slice(n.start,n.end)};};
 const deviceModule=module(main,78193),deviceNode=deviceModule.get('DeviceInfo');
 const wanted=['analogSpecs','AnalogGenVersion','isLowProfile','isShowDoubleMappingOption','disableControllerAnalogMapping'];
 const device=Object.fromEntries(deviceNode.properties.filter(p=>wanted.includes(key(p.key))).map(p=>[key(p.key),literal(p.value)]));
 row.helpers.push({module:78193,name:'DeviceInfo',...receipt(main,deviceModule.get('DeviceInfo'))});
 for(const [id,names] of [[29267,['N0','tW']],[31867,['fH']],[60481,['r']],[13254,['fMo','pEN']]]){const m=module(main,id);for(const name of names)row.helpers.push({module:id,name,...receipt(main,m.get(name))});if(id===29267)for(const name of ['mt','Pt','Mt','Lt'])if(m.vars.has(name))row.helpers.push({module:id,name,...receipt(main,m.vars.get(name))});}
 const excludedModule=module(main,69937),syncExcluded=[...literal(excludedModule.get('hYc')),...literal(excludedModule.get('IOh')),...(deviceNode.properties.some(p=>key(p.key)==='OBM_PRESET_PROFILES_HOT_KEYS')?literal(deviceNode.properties.find(p=>key(p.key)==='OBM_PRESET_PROFILES_HOT_KEYS').value):[])];
 const sync=inlineSync(main);
 row.helpers.push({module:99857,name:'SetCache-unrelated-to-Sync',...receipt(main,module(main,99857).node)});
 const syncAction=module(main,13254).get('c2U');row.helpers.push({module:13254,name:'c2U-Sync-action',...receipt(main,syncAction)});
 row.helpers.push({module:null,scope:'inline-entry-IIFE',name:sync.name,semantic_name:'syncActuationKeys-reducer',owning_iife_utf16_range:[sync.owner.callee.start,sync.owner.callee.end],...receipt(main,sync.node)});
 row.helpers.push({module:null,scope:'inline-entry-IIFE',name:sync.name+'-declaration',...receipt(main,sync.declaration)});
 row.helpers.push({module:null,scope:'inline-entry-IIFE',name:sync.name+'-sync-dispatch',...receipt(main,sync.dispatch)});
 row.helpers.push({module:null,scope:'inline-entry-IIFE',name:key(sync.import.id)+'-mapping-helper-import',...receipt(main,sync.import)});
 const mappingHelpers=module(main,3342);for(const name of ['QY','hF','aC'])row.helpers.push({module:3342,name,...receipt(main,mappingHelpers.get(name))});for(const name of ['S','D','d'])row.helpers.push({module:3342,name,...receipt(main,mappingHelpers.vars.get(name))});for(const name of ['hYc','IOh'])row.helpers.push({module:69937,name,...receipt(main,excludedModule.get(name))});
 const labelModule=module(main,54693),labels={};for(const symbol of ['AfU','cHL','yXn','fab','e4R','$so','QuW','EQ']){labels[symbol]=literal(labelModule.get(symbol));row.helpers.push({module:54693,name:symbol,...receipt(main,labelModule.get(symbol))});}
 for(const prefix of ['TwoTapKeyMapping.','2383.']){
  const path=uniqueFile(root+'js',prefix),s=fs.readFileSync(path,'utf8'),tree=acorn.parse(s,{ecmaVersion:'latest'}),methods=[];
  walk(tree,n=>{if(n.type==='AssignmentExpression'&&n.left.object?.type==='ThisExpression'&&['checkSecondaryIsExist','showSecondaryFunction','closeSecondaryFunction','getTwoTapMapping','getDefaultMapping','onActuationPointChange','onCheckboxCustomReleaseChange','getDisplayFunctionList','getCustomItems','saveTwoTapChanges','isDefaultMapping','shouldDisabledSecondaryFunction','requireSecondaryFunction','removeSecondaryFunction','renderFunction','onMakeActuationPointChange','onCustomReleasePointChange','onCheckboxCustomRelease','syncActuationReleasePoint','handleAnalogKeyRegister','isFunctionConfigured','hasValidCurrentValue','hasExistingMappedValue','requiresExplicitSelection'].includes(key(n.left.property)))methods.push({name:key(n.left.property),...receipt(s,n)});if(n.type==='ClassDeclaration'&&n.id?.name==='Y')methods.push({name:'mounted-ActuationPoint-Y',...receipt(s,n)});});
  row.files.push({path,sha256:hash(s),methods});
 }
 for(const cssName of fs.readdirSync(root+'css').filter(f=>f.endsWith('.css'))){const path=root+'css/'+cssName,s=fs.readFileSync(path,'utf8');const rules=[...s.matchAll(/[^{}]*(?:key-config-flex|secondary-keymap|secondary-function|two-tap|actutal-point-wrapper|actuation-text-guide|actual-point|extra-head|remove-2nd-func)[^{}]*\{[^}]*\}/g)].map(m=>({byte_offset:Buffer.byteLength(s.slice(0,m.index)),source:m[0]}));if(rules.length)row.css.push({path,sha256:hash(s),rules});}
 native.push({product_id:pid,info:device.analogSpecs.actuationInfo,sync_excluded:syncExcluded,analog_v1:device.AnalogGenVersion==='analogV1',low_profile:device.isLowProfile===true,double_mapping:device.isShowDoubleMappingOption===true,disable_analog:device.disableControllerAnalogMapping===true,labels,callback_set:literal(module(main,13254).get('fMo')),callback_remove:literal(module(main,13254).get('pEN'))});
 records.push(row);
}
fs.writeFileSync('docs/re/keyboard-two-tap-current-source.json',JSON.stringify({method:'Direct current JS/CSS, static AST-only; byte_offset is UTF-8 bytes, char_range and utf16_range are UTF-16 code units with exclusive end. Required inline Sync helper/declaration/dispatch/import must each resolve uniquely or extraction fails. Method definitions and mounted Y.render remain separate from UI capability flags and runtime acceptance.',records},null,2)+'\n');
if(process.argv.includes('--prepare'))fs.writeFileSync('crates/razer-pages/src/features/keyboard_mapping_two_tap_data.json',JSON.stringify(native));
console.log(JSON.stringify(native));
