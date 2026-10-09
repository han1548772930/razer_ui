// Current Studio defaults, locale literals, root receipts and declared CSS assets.
// The downloaded bundle is parsed by Acorn only; no factory is evaluated.
const fs=require('fs'),path=require('path');
const {source:s}=require('./audit-chroma-studio.cjs');
const {walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),receipts=[];
const manifest=JSON.parse(fs.readFileSync(path.join(root,s.directory,'asset-manifest.json'),'utf8'));
function node(id,name){const n=s.binding(id,name);receipts.push({module:id,symbol:name,...s.receipt(id,n)});return n;}
function value(id,n,env={}){
 if(n.type==='Identifier')return Object.hasOwn(env,n.name)?env[n.name]:value(id,s.binding(id,n.name),env);
 if(n.type==='UnaryExpression'&&n.operator==='void')return undefined;
 if(n.type==='CallExpression'&&n.callee.type==='SequenceExpression'){
  const callee=n.callee.expressions.at(-1);
  if(callee.type==='MemberExpression'&&callee.object.type==='Identifier'){
   const b=s.binding(id,callee.object.name);
   if(b.type==='CallExpression'&&b.arguments[0]?.value===9220&&key(callee.property)==='A'){
    // 9220:i statically maps stops and RGB bytes, then sorts by Stop.
    return value(id,n.arguments[0],env).map(v=>({Stop:Math.round(100*v.stop),Color:v.rgb?.reduce((a,b)=>256*a+b,0)})).sort((a,b)=>a.Stop-b.Stop);
   }
  }
 }
 if(n.type==='MemberExpression'){
  if(n.object.type==='Identifier'){
   const b=s.module(id).definitions.get(n.object.name);
   if(b?.type==='CallExpression'&&b.callee.name===s.module(id).fn.params[2]?.name&&b.arguments[0]?.type==='Literal'){
    const target=b.arguments[0].value;return value(target,s.exported(target,key(n.property)),env);
   }
  }
  return value(id,n.object,env)[n.computed?value(id,n.property,env):key(n.property)];
 }
 if(n.type==='ObjectExpression')return Object.fromEntries(n.properties.map(p=>[p.computed?value(id,p.key,env):key(p.key),value(id,p.value,env)]));
 if(n.type==='ArrayExpression')return n.elements.map(v=>value(id,v,env));
 if(n.type==='ConditionalExpression')return value(id,value(id,n.test,env)?n.consequent:n.alternate,env);
 if(n.type==='BinaryExpression'){
  const a=value(id,n.left,env),b=value(id,n.right,env);
  if(n.operator==='/')return a/b;
  if(n.operator==='*')return a*b;
  if(n.operator==='+')return a+b;
  if(n.operator==='-')return a-b;
 }
 return s.literal(id,n);
}
function cases(id,name){
 const n=node(id,name),sw=n.body.body.find(n=>n.type==='SwitchStatement');
 return Object.fromEntries(sw.cases.filter(c=>c.test).map(c=>{
  const assignment=c.consequent.find(n=>n.type==='ExpressionStatement').expression;
  return[value(id,c.test),assignment.right];
 }));
}
const defaults=Object.fromEntries(Object.entries({editor:'I',device:'D',effect_layer:'M',profile:'z',history:'Y',modal:'E'}).map(([field,name])=>[field,value(1638,node(1638,name))]));
const names=value(6257,node(6257,'x')),effectLabels=cases(2904,'a'),effectValues=cases(2904,'c'),effectParams=cases(2904,'d');
node(9220,'i');
const effects=names.map(name=>({name,label:value(2904,effectLabels[name]),value:value(2904,effectValues[name]),params:value(2904,effectParams[name],{t:false}),paint_params:value(2904,effectParams[name],{t:true})}));
for(const effect of effects){
 const key={spectrum:'Q9',breathing:'NH',reactive:'MB',starlight:'f5'}[effect.name];
 effect.duration_values=key?value(6257,s.exported(6257,key)):[];
 if(key&&effect.duration_values.length!==3)throw Error(`Unexpected duration scale ${effect.name}`);
}
const locales={};
for(const [locale,id]of Object.entries({en:8362,es:2011,de:2175,fr:5990,ja:7185,kr:4408,ru:3234,'pt-BR':9581,'zh-CN':1993,'zh-TW':4843})){
 locales[locale]=Object.fromEntries([...s.module(id).exports].map(([key,n])=>[key,value(id,n)]));
 receipts.push({module:id,symbol:'locale exports',...s.receipt(id,s.module(id).fn)});
}
const contracts=[[-1,['rr','xt','rt','nt','Z','qo','Pt']], [4264,['vt','J','se','ue','me','_','De','Se','we','re']], [9286,['S','E','w']], [7943,['C']]];
for(const[id,names]of contracts)for(const name of names)node(id,name);
const css=[];
for(const key of ['main.css','EditorCanvas.css','Help.css','ModalNoDevices.css','static/css/42.04f66612.chunk.css']){
 const file=`${s.directory}/${manifest.files[key].replace(/^\.\//,'')}`,raw=fs.readFileSync(path.join(root,file),'utf8');
 css.push(...parseCSS(raw).map(rule=>({path:file,sha256:hash(raw),...rule})));
}
const assetNames=['ambient-white','audio-white','breathing-white','chroma-generate-white','fire-white','reactive-white','ripple-white','spectrum-white','starlight-white','static-white','tidal-white','wave-white','wheel-white','folder-add-gray','folder-gray','eye-gray','eye-disabled-white','ellipsis-gray','icon_reset','cursor-white','cursor-green','pen-white','pen-green','bucket-white','bucket-green','move-white','move-green','trash-white','undo-white','redo-white','plus-white','minus-white','keyboard-white','icon_help','dropdown','logo_chromastudio','icon-label','icon-label-disable','icon-chroma-preview','icon-chroma-slow-preview','icon-chroma-stop','tooltip_questionmark'];
assetNames.push('wave-lg-gray','wave-md-gray','wave-sm-gray','wave-reactive-md-gray','wave-reactive-sm-gray');
assetNames.push(...['clockwise','counterclockwise','outward','inward'].flatMap(name=>[name+'-gray',name+'-black']));
const assets=[];
for(const name of assetNames){
 const selector=name.startsWith('icon-label')||name.startsWith('icon-chroma')?`.${name}`:name==='keyboard-white'?'.btn-icon-2.btn-keyboard':null;
 const urls=[...new Set(css.filter(r=>!selector||r.selector===selector).flatMap(r=>[...r.declarations.matchAll(/url\(([^)]+)\)/g)].map(m=>m[1].replaceAll('"','').replaceAll("'",'').split('#')[0])).filter(url=>selector||new RegExp('/'+name+'\\.[a-f0-9]+\\.svg$').test(url)))];
 if(urls.length!==1)throw Error(`Expected one current asset ${name}, got ${urls.length}`);
 const relative=urls[0].replace(/^\.\.\/\.\.\//,'');
 const entry=Object.entries(manifest.files).find(([_,v])=>v===`./${relative}`);
 if(!entry)throw Error(`Asset absent from current manifest: ${relative}`);
 assets.push({name,manifest_key:entry[0],source:`${s.directory}/${relative}`,url:`https://apps.razer.com/synapse/chroma-studio/${relative}`,output:`assets/synapse/chroma-studio-${name}.svg`});
}
const gradients=Object.fromEntries(Object.entries({spectrum:['Zk','BI',2],audio:['sR','kI',1],default:['dp','VD',1]}).map(([name,[presets,max,min]])=>[name,{presets:value(6257,s.exported(6257,presets)),max_stops:value(6257,s.exported(6257,max)),min_stops:min}]));
const playback={start:value(6257,s.exported(6257,'W4')),end:value(6257,s.exported(6257,'$e')),min:value(6257,s.exported(6257,'YZ')),max:value(6257,s.exported(6257,'QG')),labels:Object.fromEntries(Object.entries(cases(126,'n')).map(([name,n])=>[name,value(126,n)]))};
const empty_color=value(6257,s.exported(6257,'FU'));
const data={defaults,effects,gradients,playback,empty_color,tools:value(6257,node(6257,'R')),zoom_levels:value(6257,node(6257,'T')),locales,assets:Object.fromEntries(assets.map(a=>[a.name,a.output.replace(/^assets\//,'')]))};
const evidence={method:'Current independent Studio; static Acorn and CSS parsing, no vendor execution.',route:'/synapse/chroma-studio/',manifest:{path:`${s.directory}/asset-manifest.json`,sha256:hash(fs.readFileSync(path.join(root,s.directory,'asset-manifest.json')))},receipts,css,assets};
const check=process.argv.includes('--check');
for(const[file,obj]of [['crates/razer-pages/src/features/chroma_studio_data.json',data],['docs/re/chroma-studio-source.json',evidence]]){
 const out=JSON.stringify(obj,null,2)+'\n',target=path.join(root,file);
 if(check){if(fs.readFileSync(target,'utf8')!==out)throw Error(`Stale ${file}`);}else fs.writeFileSync(target,out);
}
console.log(`Studio: ${effects.length} effects, ${Object.keys(locales).length} locales, ${assets.length} source assets.`);
