// Current dock sources are parsed as Acorn data. Never evaluate vendor JavaScript.
const fs=require('fs'), path=require('path'), crypto=require('crypto'), acorn=require('acorn');
const root=path.resolve(__dirname,'..');
const read=p=>fs.readFileSync(path.join(root,p),'utf8');
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
function walk(n,f){if(!n?.type||f(n)===false)return;for(const v of Object.values(n)){if(Array.isArray(v))v.forEach(c=>walk(c,f));else if(v?.type)walk(v,f);}}
function literal(n){if(n.type==='Literal')return n.value;if(n.type==='UnaryExpression'&&n.operator==='!')return !literal(n.argument);if(n.type==='ObjectExpression')return Object.fromEntries(n.properties.map(p=>[p.key.name??p.key.value,literal(p.value)]));throw Error('Nonliteral '+n.type);}
function emit(p,data){const out=JSON.stringify(data,null,2)+'\n';if(process.argv.includes('--check')){if(read(p)!==out)throw Error('Stale '+p);}else fs.writeFileSync(path.join(root,p),out);}
const specs=[],evidence=[];
for(const pid of [164,241]){
  const receipt=JSON.parse(read(`docs/re/pending-product-${pid}-source.json`));
  for(const file of receipt.source_files)if(hash(read(file.path))!==file.sha256)throw Error('Changed '+file.path);
  const dir=`.ref/devices/${pid}/static/js`,parsed=[];
  function parse(p){let part=parsed.find(x=>x.path===p);if(part)return part;const source=read(p),ast=acorn.parse(source,{ecmaVersion:'latest'});part={path:p,source,ast};parsed.push(part);return part;}
  const files=fs.readdirSync(path.join(root,dir)).filter(f=>f.endsWith('.js')&&!f.startsWith('trans-')).map(f=>dir+'/'+f);
  const labelFile=files.find(p=>/(?:^|[,{])4693:/.test(read(p)));if(!labelFile)throw Error('No current labels');
  const part=parse(labelFile);let labelModule;walk(part.ast,n=>{if(n.type==='Property'&&n.key.value===4693)labelModule=n.value;});
  const bindings=new Map(), exports=new Map();
  walk(labelModule,n=>{if(n.type==='VariableDeclarator')bindings.set(n.id.name,n.init);if(n.type==='CallExpression'&&n.callee.property?.name==='d'&&n.arguments[1]?.type==='ObjectExpression')for(const p of n.arguments[1].properties)exports.set(p.key.name??p.key.value,p.value.body);});
  const exportKeys=pid===164?['UVv','Ts4','p5h','CEh','Qkp','VYK','f9h','EXM','A4y','JWw']:['CfR','FY7','VYK','Lps','M9m','hIF','yWF','kTs','T8n'];
  const labels=Object.fromEntries(exportKeys.map(key=>{const n=bindings.get(exports.get(key)?.name);if(!n)throw Error('Missing '+key);return[key,literal(n)];}));
  const componentPath=pid===164?receipt.source_files[0].path:files.find(p=>p.includes('/914.'));
  const component=parse(componentPath),states=[];
  walk(component.ast,n=>{if(n.type==='VariableDeclarator'&&n.id.name===(pid===164?'J':'Ge')&&n.start<(pid===164?55000:10000)&&n.init?.type==='ObjectExpression')states.push(literal(n.init));});
  if(states.length!==1)throw Error('Ambiguous dock state enum');
  const page=receipt.pages[0],config=page.components.find(c=>c.offset===(pid===164?72173:54400)).jsx.find(x=>x.props.deviceInfo)?.props.deviceInfo;
  const css=[];
  for(const f of fs.readdirSync(path.join(root,`.ref/devices/${pid}/static/css`)).filter(f=>f.endsWith('.css'))){const p=`.ref/devices/${pid}/static/css/${f}`,s=read(p);const rules=s.split('}').filter(r=>{const selector=r.split('{')[0];return pid===164?/HyperPollingWireless_|hyperpolling-device-content|Dialog_(backDrop|modal|header)|\.nospecificdevice|\.img-text \.multipairing|MultiDevicePairing_/.test(selector):/Duallink_|duallink-device-content|modal_(backDrop|modal|header|close)|HyperPollingWirelessMouseDock_|\.nospecificdevice.*duallink/.test(selector);}).map(r=>r+'}');if(rules.length)css.push({path:p,sha256:hash(s),rules});}
  const manifestPath=`.ref/devices/${pid}/asset-manifest.json`,manifest=JSON.parse(read(manifestPath));
  const requests=[...new Set(css.flatMap(c=>c.rules.flatMap(r=>[...r.matchAll(/url\(\.\.\/\.\.\/(static\/media\/[^)]+)\)/g)].map(m=>m[1]))))];
  // The single-device branch supplies its own imgPairing, distinct from the
  // multi-pairing DeviceInfo. Verify that declaration against the mounted entry.
  if(pid===164){
    const request='static/media/zia_pairing.8cf19beb.avif';
    if(!component.source.includes('Ne=a.p+"'+request+'"') || !page.components.find(c=>c.offset===72173).source.includes('imgPairing:Ne'))throw Error('Changed single dock artwork binding');
    requests.push(request);
  }
  for(const name of ['icon-multideviceparing','icon-multidevicepairing2','icon_category_mouse','icon_category_keyboard','icon_mouse_ensure','icon_mousemat_ensure','icon-progress_spinner','close']){
    const matches=Object.values(manifest.files).map(p=>p.replace(/^\.\//,'')).filter(p=>new RegExp('/'+name+'\\.[a-f0-9]+\\.svg$').test(p));
    if(matches.length===1)requests.push(matches[0]);
  }
  const assets=[...new Set(requests)].sort().map(request=>{if(!Object.values(manifest.files).some(p=>p.replace(/^\.\//,'')===request))throw Error('Undeclared '+request);const name=path.basename(request).replace(/\.[a-f0-9]{8}\.(svg|avif)$/,(m,ext)=>'.'+(ext==='avif'?'png':ext));return{source:`.ref/devices/${pid}/`+request,url:`https://apps.razer.com/synapse/products/${pid}/ui/`+request,output:`assets/synapse/dock-${pid}-`+name};});
  // URLs are literal source declarations, not executable imports.
  const urls=[...component.source.matchAll(/https:\/\/[^"'`\s]+/g)].map(m=>m[0]).filter(u=>/razer|support/.test(u)&&!u.includes('${'));
  const keys=new Set([...Object.values(labels),...page.components.flatMap(c=>c.jsx.flatMap(x=>[x.props.text,x.props.title,x.props.tips])).filter(v=>typeof v==='string'),config.widgetTips,config.pairingDescription,'HYPER_SPEED_MULTI_DEVICE_DONGLE_DUALLINK_NOTE_ONE','DUALLINK_NOTE_TWO','DUALLINK_NOTE_THREE','DUALLINK_NOTE_FIVE','DUALLINK_NOTE_FOUR_MOUSE_DOCK','MOUSE_DOCK_UNPAIR_CONFIRM_TEXT','MOUSE_DOCK_BOTH_DEVICES_PAIRING_UTILITY_WARNING','MOUSE_DOCK_PRO_DUAL_DEVICE_POLLING_RATE_WARNING','MOUSE_DOCK_ORIGINAL_DONGLE_WARNING','PAIRED_WITH','CONFIGURE_POLLING_RATE_DEVICE_DISCONNECTED_TEXT'].filter(Boolean));
  const translations={},locale_receipts=[];
  if(pid===164){
    const scopes=new WeakMap(),declarations=[],calls=[];
    function visit(n,scope){
      if(!n?.type)return;
      if(/Function/.test(n.type)||n.type==='Program')scope={parent:scope,defs:new Map()};
      scopes.set(n,scope);
      if(n.type==='VariableDeclarator'&&n.id.type==='Identifier'){scope.defs.set(n.id.name,n.init);declarations.push(n);}
      if(n.type==='CallExpression'&&n.callee.property?.name==='d'&&n.arguments[1]?.type==='ObjectExpression')calls.push(n);
      for(const v of Object.values(n)){if(Array.isArray(v))v.forEach(c=>visit(c,scope));else if(v?.type)visit(v,scope);}
    }
    visit(component.ast,null);
    function lookup(n){let scope=scopes.get(n);while(scope){if(scope.defs.has(n.name))return scope.defs.get(n.name);scope=scope.parent;}throw Error('Missing locale binding '+n.name);}
    function decode(n){if(n.type==='Identifier')return decode(lookup(n));return literal(n);}
    const map=declarations.find(n=>n.init?.type==='ObjectExpression'&&n.init.properties.length===10&&n.init.properties.every(p=>p.value.type==='Identifier')&&n.init.properties.some(p=>p.key.name==='en')&&n.init.properties.some(p=>{try{return (p.computed?decode(p.key):p.key.value)==='zh-CN';}catch{return false;}}));
    if(!map)throw Error('Missing embedded dock locales');
    for(const entry of map.init.properties){
      const locale=entry.computed?decode(entry.key):entry.key.name??entry.key.value,namespace=lookup(entry.value);
      const call=calls.find(c=>{try{return lookup(c.arguments[0])===namespace;}catch{return false;}});
      if(!call)throw Error('Missing dock locale exports '+locale);
      translations[locale]={};
      for(const p of call.arguments[1].properties){const key=p.key.name??p.key.value;if(keys.has(key))translations[locale][key]=decode(p.value.body);}
    }
    locale_receipts.push({path:componentPath,sha256:hash(component.source)});
  }
  for(const f of fs.readdirSync(path.join(root,dir)).filter(f=>/^trans-.*\.js$/.test(f))){
    const locale=f.match(/^trans-(.+)\.[a-f0-9]+\.chunk\.js$/)?.[1];if(!locale)throw Error('Changed locale path');
    const p=dir+'/'+f,s=read(p),ast=acorn.parse(s,{ecmaVersion:'latest'}),values=new Map(),refs=new Map();
    walk(ast,n=>{if(n.type==='VariableDeclarator'&&n.init?.type==='Literal')values.set(n.id.name,n.init.value);if(n.type==='Property'&&keys.has(n.key.name??n.key.value)&&n.value.type==='ArrowFunctionExpression')refs.set(n.key.name??n.key.value,n.value.body.name);});
    translations[locale]=Object.fromEntries([...refs].map(([k,v])=>[k,values.get(v)]).filter(([k,v])=>typeof v==='string'));
    locale_receipts.push({path:p,sha256:hash(s)});
  }
  specs.push({product_id:pid,page:page.key,config,labels,states:states[0],assets,translations});
  evidence.push({product_id:pid,source_files:receipt.source_files,label_source:{path:labelFile,sha256:hash(part.source)},labels,locale_receipts,css,manifest:{path:manifestPath,sha256:hash(read(manifestPath))},urls:[...new Set(urls)],components:page.components.filter(c=>c.path===componentPath&&c.offset<100000).map(({path,offset,end,source})=>({path,offset,end,source:source.replace(/data:image\/[^"\s]+/g,'[embedded bitmap omitted; retained in hashed source]')}))});
}
emit('src/features/dock_pairing_data.json',specs);
emit('docs/re/dock-pairing-current-evidence.json',{method:'Acorn literal extraction and current mounted component receipts; vendor JavaScript never executed',generator_sha256:hash(fs.readFileSync(__filename)),products:evidence});
console.log('Resolved both dock pairing pages, labels, states and resource declarations.');
