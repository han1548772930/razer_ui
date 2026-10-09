// Current Chroma Settings is parsed as data only; never execute vendor code.
const fs=require('fs'), path=require('path'), acorn=require('acorn');
const {Source,walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'), check=process.argv.includes('--check');
const read=p=>fs.readFileSync(path.join(root,p),'utf8'), outputAssets=[];
const emit=(p,value)=>{const bytes=typeof value==='string'?value:JSON.stringify(value,null,2)+'\n';
  if(p.endsWith('.svg'))outputAssets.push({path:p,sha256:hash(bytes)});
  if(check){if(read(p)!==bytes)throw Error(`Stale ${p}`);}else{fs.mkdirSync(path.dirname(path.join(root,p)),{recursive:true});fs.writeFileSync(path.join(root,p),bytes);}};
function source(route){const s=new Source(route);s.files=[...new Set(Object.values(JSON.parse(read(s.directory+'/asset-manifest.json')).files))]
  .filter(p=>p.startsWith('/'+route+'/static/js/')&&p.endsWith('.js')).map(p=>'.ref/applications'+p);return s;}
const s=source('chroma-app/settings'), receipts=[];
const names=['Zo','Vo','ni','di','ui','mi','vi','Ei','L','j','q','Ne','Hs','ye','An','Zs','de','Ae','Ps','Me','Ls','Ts'];
for(const name of names)receipts.push({module:1743,name,...s.receipt(1743,s.binding(1743,name))});
const text=name=>s.snippet(1743,s.binding(1743,name));
for(const [name,fragment]of [['Zo','gi.init("settings-chroma")'],['Hs','getAppAutoStart(Vs)'],['mi','(0,x.jsx)(de,{})'],['vi','(0,x.jsx)(Ps,{})'],['de','e[2]>=22631||22621==e[2]&&e[3]>=2506']])
  if(!text(name).includes(fragment))throw Error(`Changed Chroma Settings ${name}`);
const symbols=new Set();
for(const name of ['Hs','ye','An','Zs','de','Ae','Ps','Vo','Ts'])walk(s.binding(1743,name),n=>{if(n.type==='MemberExpression'&&n.object.name==='V')symbols.add(key(n.property));});
const labels=Object.fromEntries([...symbols].sort().map(name=>[name,s.literal(4693,s.exported(4693,name))]));
const keys=[...new Set([...Object.values(labels),'CHROMA_APP','GENERAL','CLOSE'])];
const translations={};
const importer=s.binding(7248,'a');
for(const p of importer.properties){const locale=(p.computed?s.literal(7248,p.key):key(p.key)).toLowerCase(),calls=[];
  walk(p.value,n=>{if(n.type==='CallExpression'&&key(n.callee.property)==='bind')calls.push(n);});
  if(calls.length!==1)throw Error('Locale importer changed');const id=calls[0].arguments[1].value;
  translations[locale]=Object.fromEntries(keys.map(k=>[k,s.literal(id,s.exported(id,k))]));}
const languages=s.literal(1743,s.binding(1743,'Me'));
const tutorialKeys=['ffh','c1r','q7M','ISe','HEU','BeG'].map(k=>s.literal(9937,s.exported(9937,k)));
const assets=[];
for(const [name,file]of [['back','icon_arrow_left_thin.e6d37c55.svg'],['forward','icon_arrow_right_thin.bef8ca32.svg'],['refresh','icon_refresh.80aa16c3.svg'],['help','tooltip_questionmark.96138d2f.svg']]){
 const p=s.directory+'/static/media/'+file,out=`assets/synapse/chroma-settings-${name}.svg`;
 emit(out,read(p));assets.push({name,path:p,sha256:hash(read(p)),output:out});}
emit('assets/synapse/chroma-settings-check.svg','<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><g fill="#111"><rect x="0" y="0" width="3" height="15.4" rx="2.4" transform="translate(8.6 16.4) rotate(-145)"/><rect x="0" y="0" width="3" height="9.6" rx="2.4" transform="translate(.8 10.2) rotate(-50)"/></g></svg>\n');
for(const [name,binding]of [['logo','Rn'],['logo-beta','Tn'],['warning','te'],['wdl','ne']]){
 const n=s.binding(1743,binding);if(n.type!=='BinaryExpression'||n.right.type!=='Literal')throw Error('Asset expression changed');
 const file=s.directory+'/'+n.right.value,out=`assets/synapse/chroma-settings-${name}.svg`;
 emit(out,read(file));assets.push({name,path:file,sha256:hash(read(file)),output:out});
}
const socials=[];
const socialDefs=s.binding(1743,'Ls').properties.map(p=>{
 const fields=Object.fromEntries(p.value.properties.map(q=>[key(q.key),q.value]));
 return {name:key(p.key),link:s.literal(1743,fields.link),label:s.literal(1743,s.binding(1743,'Ts').properties.find(q=>key(q.key)===s.literal(1743,fields.translationType)).value),symbol:fields.component.arguments[0].name};
});
socialDefs.push({name:'insider',link:'https://insider.razer.com',label:'Razer Insider',symbol:'Is'});
const xml=v=>String(v).replaceAll('&','&amp;').replaceAll('"','&quot;').replaceAll('<','&lt;');
// Inline SVG style blocks share the document's CSS scope in the source.
// In particular the Facebook component supplies ellipse/social rules for all seven.
const sharedSocialStyles=[];
const facebook=s.binding(1743,s.binding(1743,socialDefs.find(s=>s.name==='facebook').symbol).arguments[0].name);
walk(facebook,n=>{if(n.type==='CallExpression'&&key(n.callee.property)==='createElement'&&n.arguments[0]?.value==='style'&&n.arguments[2]?.type==='Literal')sharedSocialStyles.push(n.arguments[2].value);});
if(!sharedSocialStyles.some(s=>s.includes('.ellipse{fill:#222')))throw Error('Shared social CSS changed');
for(const spec of socialDefs){const wrapper=s.binding(1743,spec.symbol),node=s.binding(1743,wrapper.arguments[0].name),shapes=[],styles=[];let viewBox;
 walk(node,n=>{if(n.type!=='CallExpression'||key(n.callee.property)!=='createElement')return;const tag=n.arguments[0]?.value;
  if(tag==='svg'){const attrs=n.arguments[1].arguments[0];viewBox=attrs.properties.find(p=>key(p.key)==='viewBox').value.value;}
  if(tag==='style'&&n.arguments.length>2){if(n.arguments[2].type!=='Literal')throw Error('Dynamic SVG style');styles.push(n.arguments[2].value);}
  if(tag==='g'&&n.arguments[1]?.properties.some(p=>key(p.key)!=='id'))throw Error('SVG group geometry/styles require preservation');
  if(!['circle','path','rect'].includes(tag))return;
  const attrs=Object.fromEntries(n.arguments[1].properties.map(p=>{if(p.value.type!=='Literal')throw Error('Dynamic social shape');return[key(p.key)==='className'?'class':key(p.key),p.value.value];}));shapes.push({tag,attrs});
 });
 if(!viewBox||!shapes.length)throw Error('Social SVG changed');
 for(const hover of [false,true]){const css=[...sharedSocialStyles,...styles].join('\n').replace(/\.ellipse:hover\s*~\s*\.social/g,hover?'.social':'.unused').replace(/\.[\w-]+:hover/g,m=>hover?m.replace(':hover',''):'.unused');
  const svg=`<svg xmlns="http://www.w3.org/2000/svg" viewBox="${viewBox}"><style>${css}</style>${shapes.map(x=>`<${x.tag} ${Object.entries(x.attrs).map(([k,v])=>`${k}="${xml(v)}"`).join(' ')}/>`).join('')}</svg>\n`;
  emit(`assets/synapse/chroma-settings-${spec.name}${hover?'-hover':''}.svg`,svg);}
 socials.push({...spec,viewBox,shapes,styles,source:s.receipt(1743,node)});
}
const manifest=JSON.parse(read(s.directory+'/asset-manifest.json')),css=[];
for(const url of [...new Set(Object.values(manifest.files))].filter(p=>p.endsWith('.css'))){const file='.ref/applications'+url,t=read(file);
 for(const r of parseCSS(t))if(/main-setting|body-widgets|widget-col|\.widget(?:\s|\.|$)|setting-block|tree-checkbox|about-setting|social-|\.toolbar|nav-tabs|check-item|check-box|check-text|thx-btn|twoWay|wdl-setting|modes-area|release-patch|chroma-app-copyright/.test(r.selector)&&r.declarations.length<6000)css.push({path:file,sha256:hash(t),...r});}
const dashboard=source('chroma-app/dashboard');
for(const [module,name]of [[23322,'Fs'],[84058,'i']])receipts.push({module,name,...dashboard.receipt(module,dashboard.binding(module,name))});
receipts.push({module:62296,name:'dashboard-introduction-consumer',...dashboard.receipt(62296,dashboard.binding(62296,'Wn'))});
const introKey=dashboard.exported(69937,'Fz_');
if(dashboard.literal(69937,introKey)!=='isShowIntroductionBanner')throw Error('Dashboard introduction key changed');
receipts.push({module:69937,name:'dashboard-introduction-key',...dashboard.receipt(69937,introKey.type==='Identifier'?dashboard.binding(69937,introKey.name):introKey)});
const flags=dashboard.exported(84058,'ZP');receipts.push({module:84058,name:'window-flags',...dashboard.receipt(84058,dashboard.binding(84058,flags.name))});
const hostPath='.ref/host-4.0.827/electron/components/Tab/Tab.js',hostText=read(hostPath);let handler;
walk(acorn.parse(hostText,{ecmaVersion:'latest'}),n=>{if(n.type==='PropertyDefinition'&&key(n.key)==='createTab')
  walk(n,p=>{if(p.type==='CallExpression'&&key(p.callee.property)==='setWindowOpenHandler')handler=p;});});
if(!handler||!hostText.slice(handler.start,handler.end).includes('V(s,e.frameName,e.url,n)'))throw Error('Host same-window branch changed');
receipts.push({name:'host-same-window',path:hostPath,sha256:hash(hostText),offset:handler.start,end:handler.end,source:hostText.slice(handler.start,handler.end)});
for(const [file,specs]of [
 ['.ref/applications/profile-migration/static/js/main.512f18b6.js', [['ly','searchParams.get("app")'],['sy','windowName'],['xD','app'],['OD','currentApp']]],
 ['.ref/release-patch-note/static/js/main.b0db08bb.js', [['Ie','https://rzr.to/']]],
]){
 const content=read(file),ast=acorn.parse(content,{ecmaVersion:'latest'});
 for(const [name,needle]of specs){const matches=[];walk(ast,n=>{if(n.type==='VariableDeclarator'&&n.id.name===name&&content.slice(n.start,n.end).includes(needle))matches.push(n);});
  if(matches.length!==1)throw Error(`Expected one ${file}/${name}, found ${matches.length}`);
  const n=matches[0];receipts.push({name,path:file,sha256:hash(content),offset:n.start,end:n.end,source:content.slice(n.start,n.end)});
 }
}
const versionManifestPath='.ref/applications/chroma-app/dashboard/manifest.json',versionManifest=JSON.parse(read(versionManifestPath));
const version=['4',...versionManifest.version.split('.').slice(1),versionManifest.buildVersion].join('.');
emit('crates/razer-app-pages/src/chroma_settings_data.json',{labels,translations,languages,tutorialKeys,version,socials:socials.map(({name,link,label,viewBox})=>({name,link,label,viewBox}))});
emit('assets/synapse/chroma-settings-embedded.rs','&[\n'+outputAssets.map(({path:p})=>{const name=path.basename(p);return `    ("synapse/${name}", include_bytes!("${name}")),`;}).join('\n')+'\n]\n');
const native=['crates/razer-app-pages/src/chroma_settings.rs','crates/razer-shell/src/shell/chroma_window.rs','crates/razer-app-pages/src/chroma_page.rs','crates/razer-app-pages/src/profile_migration.rs','crates/razer-app-pages/src/release_notes.rs','crates/razer-widgets/src/theme.rs','crates/razer-assets/src/lib.rs'].map(path=>({path,sha256:hash(read(path))}));
emit('docs/re/chroma-settings-current-evidence.json',{method:'Manifest-scoped Acorn/CSS parsing; no vendor code, app, tests or DLL execution.',receipts,css,assets,outputAssets,socials,native,version:{path:versionManifestPath,sha256:hash(read(versionManifestPath)),value:version}});
console.log(`Chroma Settings: ${receipts.length} AST receipts, ${css.length} CSS rules, ${Object.keys(translations).length} locales, ${outputAssets.length} SVG assets ${check?'checked':'prepared'}.`);
