// Parse original JS/CSS as data; never execute vendor functions.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {hash,walk,key}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const {CurrentMiddlewareSource}=require('./current-middleware-source.cjs');
const root=path.resolve(__dirname,'..'),read=file=>fs.readFileSync(path.join(root,file),'utf8'),receipts=[],products=[];
function page(file,offset,end){
 const text=read(file),scopes=new WeakMap(),nodes=new Map();
 function visit(node,scope){if(!node?.type)return;if(/^(FunctionDeclaration|ClassDeclaration)$/.test(node.type))scope?.defs.set(node.id.name,node);
  if(/Function/.test(node.type)||node.type==='Program')scope={parent:scope,defs:new Map()};
  if(node.type==='VariableDeclarator'&&node.id.type==='Identifier')scope.defs.set(node.id.name,node.init);
  scopes.set(node,scope);nodes.set(`${node.start}:${node.end}`,node);
  for(const value of Object.values(node)){if(Array.isArray(value))value.forEach(child=>visit(child,scope));else if(value?.type)visit(value,scope);}}
 visit(acorn.parse(text,{ecmaVersion:'latest'}),null);
 const anchor=nodes.get(`${offset}:${end}`);if(!anchor)throw Error('Brightness source node changed');
 const receipt=(name,node)=>{const value={name,path:file,sha256:hash(text),offset:node.start,end:node.end,source:text.slice(node.start,node.end)};receipts.push(value);return value;};
 const binding=name=>{let scope=scopes.get(anchor);while(scope){if(scope.defs.has(name))return scope.defs.get(name);scope=scope.parent;}throw Error('Missing source binding '+name);};
 return{receipt,binding,anchor};
}
function exported(source,id,name){let node=source.exported(id,name);while(node.type==='Identifier')node=source.binding(id,node.name);return node;}
for(const [product,file,start,end,names]of [
 [164,'.ref/devices/164/static/js/main.458d4103.js',4322730,4325065,['Lo','Ro','lo','So','No','Po','fr','vr']],
 [241,'.ref/devices/241/static/js/main.899712fe.js',398471,400801,['L','O','h','I','f','M','Rn','Mn']],
]){
 const p=page(file,start,end),ui={};for(const name of names)ui[name]=p.receipt(`product-${product}-${name}`,p.binding(name));
 const slider=ui[product===164?'lo':'h'].source,brightness=ui[product===164?'Lo':'L'].source,mounted=ui[product===164?'vr':'Mn'].source;
 if(!slider.includes('this.onMouseUp=')||!slider.includes('this.props.changeValue(this.state.value')||!slider.includes('disabledInput')||!slider.includes('this.handleKeyDown=')||!brightness.includes('this.toggleSwitch=')||!brightness.includes('0===e&&this.setState')||!mounted.includes('direction:"left"')||!mounted.includes('direction:"right"'))throw Error('Current interaction/mount changed');
 const manifest=JSON.parse(read(`.ref/devices/${product}/asset-manifest.json`)),cssfile=`.ref/devices/${product}/`+manifest.files['main.css'].slice(2),css=read(cssfile);
 const selectors=new Set(['.body-widgets','.body-widgets .widget-col','.body-widgets .widget','.body-widgets .widget .titleRow','.body-widgets .widget .titleRow .title','.body-widgets .widget .title .switch','.body-widgets .widget .help','.body-widgets .widget .tip','.slider-container','.slider-container.brightness','.slider-container.on','.slider-container.no-pointer','.slider-container .left,.slider-container .right','.slider-container .track','.slider-container .foot','.slider-container .slider-tip']);
 for(const selector of ['.widget-col','.widget .titleRow','.widget .titleRow .title','.widget .title .switch','.widget .help','.widget .tip','.slider-tip','.slider-tip,.thumb-tag'])selectors.add(selector);
 const rules=parseCSS(css).filter(rule=>selectors.has(rule.selector)||rule.selector.split(',').some(part=>selectors.has(part.trim())));
 if(!rules.some(rule=>rule.selector==='.slider-container'&&rule.declarations.includes('height:64px'))||!rules.some(rule=>rule.selector==='.body-widgets .widget'&&rule.declarations.includes('padding:30px 40px')))throw Error('Source layout CSS changed');
 const css_receipts=rules.map(rule=>({...rule,path:cssfile,sha256:hash(css)}));
 const source=new CurrentMiddlewareSource(product),module=product===164?25947:6107,task=product===164?exported(source,module,'ran'):source.binding(module,'It');
 receipts.push({product_id:product,name:'taskMakerSetBrightness',module,...source.receipt(module,task)});
 const branches=[];walk(source.module(41374).fn,node=>{if(node.type==='Property'&&key(node.key)==='ON_SET_BRIGHTNESS')branches.push(node);});
 for(const branch of branches)receipts.push({product_id:product,name:'ON_SET_BRIGHTNESS-state-machine',module:41374,...source.receipt(41374,branch)});
 products.push({product_id:product,ui,css_receipts,acquisition:source.acquisition});
}
const native=['crates/razer-pages/src/features/source_controls/receiver_lighting_page.rs','crates/razer-pages/src/features/source_controls/receiver_brightness_ui.rs','crates/razer-shell/src/shell/receiver_brightness_page.rs','crates/razer-storage/src/receiver_reset.rs'].map(file=>({path:file,sha256:hash(fs.readFileSync(path.join(root,file)))}));
const output={schema_version:1,generator_sha256:hash(read('tools/audit-receiver-brightness-page-current.cjs')),method:'Current frontend exact lexical AST nodes, current middleware AST receipts and CSS rules including enclosing conditions; no vendor execution',products,receipts,native,
 semantics:['Current regular Lighting root mounts brightness and switch-off widgets on the left, effects on the right. A generic vertical descriptor list does not implement this layout.',
  'Brightness switch preserves the remembered value. Slider drag previews while held and commits on mouseup; zero disables brightness. Off brightness remains draggable because the source brightness/no-pointer CSS preserves pointer events.',
  'taskMakerSetBrightness loads serial metadata active profile, increments the 53-bit version, changes isEnabled/value in the active profile, persists mZ before scheduling the skippable setting task. The helper does not write the generic factory-reset opcode.',
  'Native page submission uses the actual serial query, source document producer/cache preparation, local source document adaptation and original pre-read/conditional setter. It does not add a post-setter getter or value comparison. Actual responses and local storage remain separate.',
  'UI request slots survive page/profile invalidation until worker completion; newest desired brightness is queued, stale reads cannot clear the queued edit, and exit retains and joins actual worker threads.'],
 gaps:['Right-side regular Quick/Advanced effects tabs, effect parameter editors, color synchronization and complete effect/engine submission are not completed by the current descriptor effect selector.',
  'Switch-off display/idle settings currently remain local draft controls. Original lifecycle event registration, taskMakerSwitchOffLighting source-document persistence and host memory/cache publication still require their real implementation.',
  'The local JSON adaptation is separate from Chromium storage. Source storage overrides, older schema migration, linked subdevice scope and all other products remain explicit full-scope gaps.',
  'Static CSS receipts and cargo check do not prove runtime pixel parity or device acceptance. Application, builds, tests, vendor JS and DLLs have not been run.']};
const target='docs/re/receiver-brightness-page-current-evidence.json',text=JSON.stringify(output,null,2)+'\n';if(process.argv.includes('--check')){if(read(target)!==text)throw Error('Stale '+target);}else fs.writeFileSync(path.join(root,target),text);
console.log(JSON.stringify({products:products.length,receipts:receipts.length,css_rules:products.reduce((count,p)=>count+p.css_receipts.length,0)}));
