// Current Macro launch source, parsed statically without executing vendor code.
const fs=require('node:fs'),path=require('node:path');
const {Source,walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),source=new Source('synapse/macro');
const components=Object.fromEntries(['rt','et','z','En','va','ha','ja'].map(name=>[name,source.receipt(58190,source.binding(58190,name))]));
const templates=[13139,47990].map(id=>{
  let result;
  walk(source.module(id).fn,node=>{
    if(node.type==='Property'&&key(node.key)==='launch'&&node.value.type==='ObjectExpression') {
      const value=source.literal(id,node.value);
      if(value.Type===10)result={...source.receipt(id,node.value),value};
    }
  });
  if(!result||result.value.RadioIndex!==null||result.value.Content0!==''||result.value.Content1!=='')throw Error('Launch template changed');
  return result;
});
let picker;
walk(source.module(37226).fn,node=>{if(node.type==='MethodDefinition'&&key(node.key)==='showFileOpenDialog')picker=source.receipt(37226,node);});
if(!picker)throw Error('Current Macro picker wrapper missing');
const directory='.ref/applications/synapse/macro';
const manifest=JSON.parse(fs.readFileSync(path.join(root,directory,'asset-manifest.json'),'utf8'));
const css=[...new Set(Object.values(manifest.files))].filter(file=>file.endsWith('.css')).map(file=>{
  const filename=directory+'/'+file.slice(2),text=fs.readFileSync(path.join(root,filename),'utf8');
  return {path:filename,sha256:hash(text),rules:parseCSS(text).filter(rule=>/CustomModal_|radio-item|type=radio|^\.thx-btn|^\.keymap-action/.test(rule.selector))};
});
const embedded=fs.readFileSync(path.join(root,'assets/synapse/embedded.rs'),'utf8');
const assets=[['icon_folder.0ac33709.svg','launch-folder.svg'],['icon_folder_g.220b24b0.svg','drag-folder.svg'],['close.1d7eff2a.svg','close.svg']].map(([original,name])=>{
  const reference=directory+'/static/media/'+original,output='assets/synapse/macro/'+name;
  const data=fs.readFileSync(path.join(root,reference));
  if(!data.equals(fs.readFileSync(path.join(root,output)))||!embedded.includes('"synapse/macro/'+name+'"'))throw Error('Launch resource differs or is not embedded: '+output);
  return {reference,output,sha256:hash(data),byte_equal:true};
});
const value={method:'Current manifest-scoped Acorn and CSS parsing; no vendor execution',components,templates,picker,css,assets,
  labels:Object.fromEntries(['zUM','Qei','oeC','rE$','a_Y','H7L'].map(name=>[name,source.literal(37927,source.exported(37927,name))])),
  contract:{
    fields:'RadioIndex null/0/1; Content0 full program path; Content1 website text; displayed program name splits on backslash',
    selection:'showFileOpenDialog("Select Launch App",false) with default types []; consume first returned path; cancel preserves draft; no URL validation or implicit launch',
    save:'Content changes recompute enabled from active nonempty content; the later mode effect arms any numeric RadioIndex even with empty content. Close disarms; cancel resets item fields and effects rerun only if their dependencies changed.',
    persistence:'Popup Save updates action; outer Macro Save persists document; local picker generation/document/index/baseline checks reject stale replies',
    placement:'rt.W climbs four parents to ja #item_editor; row.offsetTop is index * 42 and excludes scrollTop. Remaining height > 100 keeps static position after trigger, otherwise top:-300. Direction selected at opening; popup remains clipped by editor.',
    remaining:'Six-parent Phased hierarchy depends on the still-incomplete Phased editor; browser file-input baseline, DOM virtualization lifecycle and final pixels are not runtime-verified',
  },
  native:{verification:'Reviewed file fingerprints, not runtime tests',files:Object.fromEntries([
    'crates/razer-app-pages/src/macro_page/launch.rs','crates/razer-app-pages/src/macro_page.rs','crates/razer-app-pages/src/macro_page/state.rs','crates/razer-app-pages/src/macro_page/body.rs',
    'crates/razer-app-pages/src/macro_page/text.rs','crates/razer-app-pages/src/macro_page/text_overlay.rs','crates/razer-app-pages/src/macro_page/unsaved.rs',
    'tools/macro_assets.py','tools/prepare-macro-assets.py',
  ].map(file=>[file,hash(fs.readFileSync(path.join(root,file)))]))},
};
const target=path.join(root,'docs/re/macro-launch-current-evidence.json'),output=JSON.stringify(value,null,2)+'\n';
if(process.argv.includes('--check')){if(fs.readFileSync(target,'utf8')!==output)throw Error('Stale Macro launch receipt');}
else fs.writeFileSync(target,output);
console.log('Current Macro Launch: two null-mode templates, picker wrapper, draft effects, CSS and three original SVGs verified.');
