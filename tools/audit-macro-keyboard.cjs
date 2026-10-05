// Parse the current Macro bundle as data; never execute vendor JavaScript.
const fs=require('node:fs'),path=require('node:path');
const {Source,walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),s=new Source('synapse/macro');
function literal(id,node){
  if(node.type==='ArrayExpression')return node.elements.map(n=>literal(id,n));
  if(node.type==='ObjectExpression')return Object.fromEntries(node.properties.map(p=>[key(p.key),literal(id,p.value)]));
  if(node.type==='CallExpression'&&node.callee.name==='parseInt'&&node.arguments.length===2&&node.arguments.every(n=>n.type==='Literal'))
    return Number.parseInt(node.arguments[0].value,node.arguments[1].value);
  return s.literal(id,node);
}
const keys=literal(46114,s.binding(46114,'o'));
if(keys.length!==144)throw Error('Current ordered keyboard catalogue changed');
for(const entry of keys)for(const [field,type] of [['name','string'],['keyCode','string'],['type','string'],['flag','number']])
  if(typeof entry[field]!==type)throw Error('Invalid embedded Key field '+field);
if(keys.some(k=>k.inputID==='')||keys.find(k=>k.keyCode==='13').inputID!=='KEY_NUMPAD_ENTER'||keys.find(k=>k.inputID==='KEY_RIGHT_SHIFT').outputFlag!==0)
  throw Error('Clear / catalogue-order / right-Shift contract changed');
const ids=new Set(keys.map(k=>k.inputID).filter(Boolean));
const names=Object.fromEntries(Object.entries(literal(90857,s.binding(90857,'Y'))).map(([layout,values])=>[layout,Object.fromEntries(Object.entries(values).filter(([id])=>ids.has(id)))]));
const extended=literal(13139,s.binding(13139,'d'));
const data={keys,names,extended};
function output(file,value){const text=JSON.stringify(value,null,2)+'\n',target=path.join(root,file);if(process.argv.includes('--check')){if(fs.readFileSync(target,'utf8')!==text)throw Error('Stale '+file);}else fs.writeFileSync(target,text);}
output('src/shell/macro_page/keyboard_data.json',data);
const templates=[13139,47990].map(id=>{let result;walk(s.module(id).fn,node=>{if(node.type==='Property'&&key(node.key)==='keyboard'&&node.value.type==='ObjectExpression')result={...s.receipt(id,node.value),value:literal(id,node.value)};});if(result?.value.Type!==1||result.value.KeyEvent.State!==null)throw Error('Keyboard template changed');return result;});
const directory=s.directory,manifest=JSON.parse(fs.readFileSync(path.join(root,directory,'asset-manifest.json'),'utf8'));
const css=[...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css')).map(f=>{const file=directory+'/'+f.slice(2),text=fs.readFileSync(path.join(root,file),'utf8');return {file,sha256:hash(text),rules:parseCSS(text).filter(r=>/InputKey_|ShortcutKey_|CustomInput_|MacroItem_flex_container|MacroItem_functional_item|^\.mr10$/.test(r.selector))};});
const original=directory+'/static/media/icon_close_enclosed.6056b667.svg',asset='assets/synapse/shortcuts-search-clear.svg';
if(!fs.readFileSync(path.join(root,original)).equals(fs.readFileSync(path.join(root,asset))))throw Error('Keyboard clear SVG differs');
const embedded=fs.readFileSync(path.join(root,'assets/synapse/embedded.rs'),'utf8');
const directionAssets=[['icon_key_down.4d5fb90e.svg','key-down.svg',9919],['icon_key_up.5fe01d07.svg','key-up.svg',80144]].map(([name,out,id])=>{
  const original=directory+'/static/media/'+name,asset='assets/synapse/macro/'+out,bytes=fs.readFileSync(path.join(root,original));
  if(!bytes.equals(fs.readFileSync(path.join(root,asset)))||!embedded.includes('"synapse/macro/'+out+'"'))throw Error('Key direction asset differs or is unregistered');
  return {original,asset,sha256:hash(bytes),module:s.receipt(id,s.module(id).fn)};
});
const nativeFiles=['src/features/macro_library.rs','src/shell/macro_page.rs','src/shell/macro_page/state.rs','src/shell/macro_page/body.rs','src/shell/macro_page/unsaved.rs','src/shell/macro_page/keyboard.rs','src/shell/macro_page/keyboard_windows.rs'];
// --data-only is useful while implementing the independently extracted data.
if(process.argv.includes('--data-only')){console.log('144 ordered keys and keyboard layout names extracted.');process.exit(0);}
output('docs/re/macro-keyboard-current-evidence.json',{
  method:'Manifest-scoped Acorn/CSS static parsing, no vendor execution',
  components:Object.fromEntries(['Nn','yn','pn','va'].map(n=>[n,s.receipt(58190,s.binding(58190,n))])),
  mutations:Object.fromEntries(['O','C','R','M','P'].map(n=>[n,s.receipt(25572,s.binding(25572,n))])),
  catalogue:s.receipt(46114,s.binding(46114,'o')),display:s.receipt(90857,s.binding(90857,'F')),templates,css,
  placeholder:s.literal(37927,s.exported(37927,'bAs')),
  data:{path:'src/shell/macro_page/keyboard_data.json',sha256:hash(fs.readFileSync(path.join(root,'src/shell/macro_page/keyboard_data.json'))),keys:keys.length,layouts:Object.keys(names).length},
  asset:{original,asset,byte_equal:true,sha256:hash(fs.readFileSync(path.join(root,asset)))},directionAssets,
  contract:{capture:'Single key; both down/up; keydown 92->91 only; Enter versus NumpadEnter and ShiftRight override; 150ms debounce; outside/blur immediate commit.',pair:'Standard/Phased create down/up sharing a local event identity; Sequence creates one null-state event. Updates propagate by identity, preserving row parity; lone rows retain their old state as in 25572.C.',clear:'pn passes empty inputID, yn dereferences the missing entry before updates. Native clear preserves this no-change result without reproducing the exception.',boundary:'Windows current-thread/current-window message capture only; no Razer inputredirect, device mappings or global-shortcut service. Unknown systemKeyboardLayout is not fabricated: outer US fallback and inner default names. Non-Windows raw capture unavailable.'},
  native:{verification:'Source review and file fingerprints, not runtime verification',files:Object.fromEntries(nativeFiles.map(f=>[f,hash(fs.readFileSync(path.join(root,f)))]))}
});
console.log('Current Macro Keyboard: 144 keys, paired templates/mutations, capture components, labels, CSS and original clear SVG verified.');
