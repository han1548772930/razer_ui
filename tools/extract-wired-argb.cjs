// Parse current wired ARGB sources as Acorn data; never execute vendor code.
const fs=require('fs'),path=require('path'),crypto=require('crypto'),acorn=require('acorn');
const root=path.resolve(__dirname,'..'),read=p=>fs.readFileSync(path.join(root,p),'utf8');
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
function walk(n,f){if(!n?.type||f(n)===false)return;for(const v of Object.values(n)){if(Array.isArray(v))v.forEach(c=>walk(c,f));else if(v?.type)walk(v,f);}}
function emit(p,v){const s=JSON.stringify(v,null,2)+'\n';if(process.argv.includes('--check')){if(read(p)!==s)throw Error('Stale '+p);}else fs.writeFileSync(path.join(root,p),s);}
const specs=[],evidence=[];
for(const pid of [778,3871]){
  const pending=JSON.parse(read(`docs/re/pending-product-${pid}-source.json`));
  const file=pending.source_files[0],source=read(file.path);
  if(hash(source)!==file.sha256)throw Error('Changed source '+file.path);
  const ast=acorn.parse(source,{ecmaVersion:'latest'}),scopes=new WeakMap(),declarations=[],exports=[],modules=new Map();
  function visit(n,scope){
    if(!n?.type)return;
    if(/Function/.test(n.type)||n.type==='Program')scope={parent:scope,defs:new Map()};
    scopes.set(n,scope);
    if(n.type==='VariableDeclarator'&&n.id.type==='Identifier'){scope.defs.set(n.id.name,n.init);declarations.push(n);}
    if(n.type==='CallExpression'&&n.callee.property?.name==='d'&&n.arguments[0]?.type==='Identifier'&&n.arguments[1]?.type==='ObjectExpression')exports.push(n);
    if(n.type==='Property'&&Number.isInteger(n.key.value)&&/Function/.test(n.value.type))modules.set(n.key.value,n.value);
    for(const v of Object.values(n)){if(Array.isArray(v))v.forEach(c=>visit(c,scope));else if(v?.type)visit(v,scope);}
  }
  visit(ast,null);
  function lookup(name,scope){while(scope){if(scope.defs.has(name))return scope.defs.get(name);scope=scope.parent;}throw Error('Unbound '+name);}
  function decode(n){
    if(n.type==='Literal')return n.value;
    if(n.type==='Identifier')return decode(lookup(n.name,scopes.get(n)));
    if(n.type==='UnaryExpression'&&n.operator==='!')return !decode(n.argument);
    if(n.type==='BinaryExpression'&&n.operator==='+')return decode(n.left)+decode(n.right);
    if(n.type==='ArrayExpression')return n.elements.map(decode);
    if(n.type==='ObjectExpression')return Object.fromEntries(n.properties.map(p=>[p.computed?decode(p.key):p.key.name??p.key.value,decode(p.value)]));
    throw Error('Nonliteral '+n.type+' '+source.slice(n.start,n.end).slice(0,40));
  }
  const receipt=n=>({path:file.path,sha256:file.sha256,offset:n.start,end:n.end,source:source.slice(n.start,n.end)});
  const page=pending.pages[0];
  const labelModule=modules.get(4693);
  const profileLabel=exports.filter(n=>n.start>=labelModule.start&&n.end<=labelModule.end).flatMap(n=>n.arguments[1].properties).find(p=>p.key.name==='lgc');
  const profileEnabledPage=decode(profileLabel.value.body);
  const profileConditions=[];walk(ast,n=>{if(n.type==='Property'&&n.key.name==='isEnableProfileBar')profileConditions.push(receipt(n));});
  if(profileEnabledPage!=='TAB_LIGHTING'||profileConditions.length!==1)throw Error('Changed profile enablement');
  const profileConsumers=[];walk(ast,n=>{if(n.type==='AssignmentExpression'&&['renderProfileBarIcon','enableSwitchProfile','renderProfileBar'].includes(n.left.property?.name))profileConsumers.push(receipt(n));});
  const profileDefault=declarations.find(n=>n.init?.type==='ObjectExpression'&&n.init.properties.some(p=>p.key.name==='enableSwitchProfile')&&n.init.properties.some(p=>p.key.name==='selectedProfileGuid'));
  const defaultProfile=declarations.find(n=>n.init?.type==='ObjectExpression'&&n.init.properties.some(p=>p.key.name==='mappings')&&n.init.properties.some(p=>p.key.name==='switchOffLighting')&&n.init.properties.some(p=>p.key.name==='brightness'));
  const persistence=[];walk(ast,n=>{if(n.type==='VariableDeclarator'&&n.init?.type==='ArrowFunctionExpression'&&source.slice(n.start,n.end).includes('ON_SET_PORT_VALUES')&&n.end-n.start<6000)persistence.push(receipt(n));});
  const keys=new Set(page.components.flatMap(c=>c.jsx.map(x=>x.props.text)).filter(k=>typeof k==='string'));
  for(const key of ['TEXT_DEVICE_TYPE','TEXT_LED_STRIP','TEXT_FAN','GLITTER_TIP_AUTO_DETECTION','GLITTER_TIP_REFRESH','GLITTER_DETECTING_DEVICES','GLITTER_POWER_SUPPLY_REQUIRED','GLITTER_POWER_SUPPLY_REQUIRED_MSG','GLITTER_TIP_LED_DETECTION','GLITTER_LED_DETECTED','GLITTER_PORT','CANCEL','SAVE','RENAME','DISMISS'])keys.add(key);
  // Dictionary references are recovered from the locale map in this source,
  // not selected by the minified variable names in another product.
  const localeMap=declarations.find(n=>n.init?.type==='ObjectExpression'&&n.init.properties.length===10&&n.init.properties.every(p=>p.value?.type==='Identifier')&&n.init.properties.some(p=>p.key.name==='en')&&n.init.properties.some(p=>p.computed&&(()=>{try{return decode(p.key)==='zh-CN';}catch{return false;}})()));
  if(!localeMap)throw Error('Missing locale map');
  const translations={};
  for(const entry of localeMap.init.properties){
    const locale=entry.computed?decode(entry.key):entry.key.name??entry.key.value;
    const namespace=lookup(entry.value.name,scopes.get(entry.value));
    const exportCall=exports.find(e=>{try{return lookup(e.arguments[0].name,scopes.get(e.arguments[0]))===namespace;}catch{return false;}});
    if(!exportCall)throw Error('Missing locale exports '+locale);
    const values={};
    for(const p of exportCall.arguments[1].properties){const key=p.key.name??p.key.value;if(keys.has(key)||key.startsWith('GLITTER_')){try{values[key]=decode(p.value.body);}catch{}}}
    translations[locale]=values;
  }
  const fans=declarations.find(n=>n.init?.type==='ArrayExpression'&&source.slice(n.init.start,n.init.end)==='[6,8,9,10,12,15,16,18,20,22,24,25,32,40]');
  const config=declarations.find(n=>n.init?.type==='ObjectExpression'&&n.init.properties.some(p=>p.key.name==='minimumLedValue')&&n.init.properties.some(p=>p.key.name==='productId'&&p.value.value===pid));
  if(!fans||!config)throw Error('Changed ARGB constants');
  const configValues=Object.fromEntries(config.init.properties.filter(p=>['productId','deviceName','subCategory','minimumLedValue'].includes(p.key.name)).map(p=>[p.key.name,decode(p.value)]));
  const assetIds=pid===3871?{fan:2966,strip1:5766,strip2:941,strip3:7408,strip4:8519}:{};
  if(pid===778){const component=page.components.find(c=>c.source.includes('this.renderLedImages='));const calls=[...component.source.matchAll(/src:a\((\d+)\)/g)].map(m=>Number(m[1]));if(calls.length!==5)throw Error('Changed bend illustrations');for(const [ix,name] of ['fan','strip2','strip3','strip4','strip1'].entries())assetIds[name]=calls[ix];}
  const manifestPath=`.ref/devices/${pid}/asset-manifest.json`,manifest=JSON.parse(read(manifestPath));
  const assets=[];
  for(const [name,id] of Object.entries(assetIds)){
    const module=modules.get(id);if(!module)throw Error('Missing image module '+id);
    let request,embedded;walk(module,n=>{if(n.type==='Literal'&&typeof n.value==='string'&&/^static\/media\//.test(n.value))request=n.value;if(n.type==='Literal'&&typeof n.value==='string'&&n.value.startsWith('data:image/png;base64,'))embedded=n;});
    if(embedded){assets.push({name,source:file.path,source_offset:embedded.start,source_end:embedded.end,embedded:true,output:`assets/synapse/wired-argb-${pid}-${name}.png`,context:file.path,context_sha256:file.sha256});continue;}
    if(!request)throw Error('No image literal '+id);
    assets.push({name,source:`.ref/devices/${pid}/${request}`,url:`https://apps.razer.com/synapse/products/${pid}/ui/${request}`,output:`assets/synapse/wired-argb-${pid}-${name}.png`,context:file.path,context_sha256:file.sha256});
  }
  for(const name of ['detecting','chroma_studio','tooltip_questionmark','stepper_up','stepper_down']){
    const request=Object.values(manifest.files).map(p=>p.replace(/^\.\//,'')).find(p=>new RegExp('/'+name+'\\.[a-f0-9]+\\.svg$').test(p));
    if(request)assets.push({name,source:`.ref/devices/${pid}/${request}`,url:`https://apps.razer.com/synapse/products/${pid}/ui/${request}`,output:`assets/synapse/wired-argb-${pid}-${name}.svg`,context:manifestPath,context_sha256:hash(read(manifestPath))});
  }
  // Product artwork comes from the exact webpack context request.
  const art=[];
  walk(ast,n=>{if(n.type==='Property'&&typeof n.key.value==='string'&&new RegExp('^\\./'+pid+'_[0-9]+/img_prods/prd-3x\\.png$').test(n.key.value))art.push(n);});
  for(const node of art){
    const decoded=decode(node.value),id=Array.isArray(decoded)?decoded[0]:decoded;let module=modules.get(id),request,context=file.path,context_sha256=file.sha256;
    if(!module){const dir=path.posix.dirname(file.path);for(const f of fs.readdirSync(path.join(root,dir)).filter(f=>f.endsWith('.js'))){const p=dir+'/'+f,s=read(p);if(!new RegExp('(?:^|[,{])'+id+':').test(s))continue;const part=acorn.parse(s,{ecmaVersion:'latest'});walk(part,n=>{if(n.type==='Property'&&n.key.value===id)module=n.value;});if(module){context=p;context_sha256=hash(s);break;}}}
    if(!module)throw Error('Missing product art module');
    walk(module,n=>{if(n.type==='Literal'&&typeof n.value==='string'&&/^static\/media\//.test(n.value))request=n.value;});
    if(!request)throw Error('Missing product art request');
    const edition=Number(node.key.value.match(/_(\d+)\//)[1]);
    assets.push({name:`product-${edition}`,source:`.ref/devices/${pid}/${request}`,url:`https://apps.razer.com/synapse/products/${pid}/ui/${request}`,output:`assets/synapse/wired-argb-${pid}-product-${edition}.png`,context,context_sha256});
  }
  if(!assets.some(a=>a.name==='product-0'))throw Error('Missing standard product artwork');
  const cssDir=`.ref/devices/${pid}/static/css`,cssPath=cssDir+'/'+fs.readdirSync(path.join(root,cssDir)).find(f=>/^main.*\.css$/.test(f)),css=read(cssPath);
  const rules=css.split('}').filter(r=>r.includes('#multipleBrightness')||/\.widget \.help|\.icon\.spinner|\.stepper:/.test(r)).map(r=>r+'}');
  const xml=s=>String(s).replaceAll('&','&amp;').replaceAll('"','&quot;').replaceAll('<','&lt;');
  function props(n){if(n?.type==='ObjectExpression')return n.properties;if(n?.type==='CallExpression')return n.arguments.flatMap(props);return [];}
  function svgNode(n){
    if(n?.type==='ArrayExpression')return n.elements.map(svgNode).join('');
    if(n?.type!=='CallExpression'||!['jsx','jsxs'].includes(n.callee.expressions?.at(-1)?.property?.name))return '';
    const tag=n.arguments[0]?.value;if(typeof tag!=='string')return '';
    let children='',attributes='';
    for(const p of props(n.arguments[1])){const key=p.key?.name??p.key?.value;if(key==='children'){children+=svgNode(p.value);continue;}if(key==='className'){try{attributes+=' class="'+xml(decode(p.value))+'"';}catch{}continue;}if(!key||key.startsWith('on'))continue;try{const value=decode(p.value);if(['string','number'].includes(typeof value))attributes+=' '+({clipPath:'clip-path',fillRule:'fill-rule',clipRule:'clip-rule'}[key]??key)+'="'+xml(value)+'"';}catch{}}
    return `<${tag}${attributes}>${children}</${tag}>`;
  }
  const iconOffsets=pid===3871?{rename:3987653,remove:3986438,warning:3958247,refresh:3957791,auto:3956533,'auto-active':3957132,detected:3964743}:{rename:4007143,remove:4005928,warning:3985460,detected:3983858};
  for(const [name,offset] of Object.entries(iconOffsets)){
    const component=page.components.find(c=>c.offset===offset);if(!component)throw Error('Missing icon component');let call;
    walk(ast,n=>{if(n.start<component.offset||n.end>component.end)return;if(n.type==='CallExpression'&&n.arguments[0]?.value==='svg')call=n;});
    if(!call)throw Error('Missing icon SVG');let svg=svgNode(call).replace('<svg','<svg xmlns="http://www.w3.org/2000/svg"');
    // Detection icon colors are CSS data, retained directly from the current stylesheet.
    if(name.startsWith('auto')){
      const styles=[];for(const rule of rules){const [selector,body]=rule.split('{');if(/:hover|:active|animation/.test(selector)||!selector.includes('detect-'))continue;for(const item of selector.split(',')){const classes=item.slice(item.indexOf('.icon-detection-wrapper')+'.icon-detection-wrapper'.length).trim();if(classes.startsWith('.icon-detection'))styles.push(classes+'{'+body);}}
      svg=svg.replace(/ xmlns="http:\/\/www.w3.org\/2000\/svg"(?=[\s\S]* xmlns=)/,'');
      svg=svg.replace(/(<svg[^>]*>)/,'$1<style>svg{fill:none}'+styles.join('')+'</style>');
    }
    assets.push({name,source:file.path,source_offset:component.offset,source_end:component.end,inline_svg:svg,output:`assets/synapse/wired-argb-${pid}-${name}.svg`,context:file.path,context_sha256:file.sha256});
  }
  const reducers=declarations.filter(n=>n.init&&((n.id.name===(pid===3871?'nH':'jh')&&n.start>4600000)||n.init.type==='ObjectExpression'&&n.init.properties.some(p=>p.key.name==='isAutoDetectionEnable'))).map(receipt);
  // Keep port edit reducers and the split helper as reviewable source receipts.
  const editReducers=[];walk(ast,n=>{if(n.type==='FunctionExpression'&&n.end-n.start<10000&&source.slice(n.start,n.end).includes('numberOfLeds')&&source.slice(n.start,n.end).includes('ports.slice()'))editReducers.push(receipt(n));});
  let mainboardPorts=[];
  if(pid===778){const main=page.components.find(c=>c.source.includes('e.filter(e=>{let E=e.argb'));walk(ast,n=>{if(n.start>=main.offset&&n.end<=main.end&&n.type==='CallExpression'&&n.callee.property?.name==='includes'&&n.callee.object?.type==='ArrayExpression')mainboardPorts=decode(n.callee.object);});if(JSON.stringify(mainboardPorts)!=='[2147483651,2147483652,2147483656]')throw Error('Changed motherboard port IDs');}
  const spec={product_id:pid,page:page.key,name:configValues.deviceName,minimum_leds:configValues.minimumLedValue,fan_counts:decode(fans.init),translations,assets,mainboard_ports:mainboardPorts};
  specs.push(spec);
  evidence.push({product_id:pid,profile_bar:{sync_icon_enabled_page:profileEnabledPage,condition:profileConditions[0],label_export:receipt(profileLabel),consumers:profileConsumers,initial:receipt(profileDefault)},state_scope:{owner:'device/localstorage',default_profile:receipt(defaultProfile),persistence},source_files:pending.source_files,config:receipt(config),fan_counts:receipt(fans),locale_map:receipt(localeMap),css:{path:cssPath,sha256:hash(css),rules},manifest:{path:manifestPath,sha256:hash(read(manifestPath))},components:page.components.map(({path,offset,end,source})=>({path,offset,end,source})),reducers,editReducers});
}
emit('src/features/wired_argb_data.json',specs);
emit('docs/re/wired-argb-current-evidence.json',{method:'Acorn static literals and source receipts; vendor code never executed',generator_sha256:hash(fs.readFileSync(__filename)),products:evidence});
console.log('Resolved both wired ARGB pages, ten locales, limits, illustrations and current CSS.');
