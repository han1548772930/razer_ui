// Current Macro text/emoji source is parsed as data, never evaluated.
const fs = require('node:fs'), path = require('node:path');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), source = new Source('synapse/macro');
const id = 58190;
const receipt = name => source.receipt(id, source.binding(id, name));
const groups = source.binding(id, 'wt').elements.map(group => {
  const property = name => group.properties.find(p => key(p.key) === name).value;
  const map = property('values'), split = map?.callee?.object;
  if (key(map?.callee?.property) !== 'map' || key(split?.callee?.property) !== 'split'
      || split.arguments[0]?.value !== ' ' || split.callee.object.type !== 'Literal'
      || source.snippet(id, map.arguments[0]) !== 'e=>({emo:e})') throw Error('Changed Macro emoji literal');
  return {label:source.literal(id, property('label')), values:split.callee.object.value.split(' ')};
});
let tabs;
walk(source.binding(id, 'on'), node => {
  if (node.type !== 'VariableDeclarator' || node.id.name !== 'E'
      || node.init?.callee?.object?.type !== 'ArrayExpression') return;
  tabs = node.init.callee.object.elements.map(tab => {
    const property = name => tab.properties.find(p => key(p.key) === name).value;
    const lookup = property('name').arguments[0];
    return {emo:source.literal(id,property('emo')), pos:source.literal(id,property('pos')),
      name:source.literal(37927,source.exported(37927,key(lookup.property)))};
  });
});
if (tabs?.length !== 8 || groups.length !== 8) throw Error('Changed Macro emoji category count');
const data = {
  groups:groups.map((group,index) => ({...group, name:tabs.find(tab => tab.pos === index).name})), tabs,
  search:source.literal(id,source.binding(id,'ft')).split(',').map(value => {
    const words=value.split(' '); return {emo:words.shift(),name:words.join(' ')};
  }),
  variants:source.literal(id,source.binding(id,'Lt')),
  excluded_windows_11:source.literal(id,source.binding(id,'Ut')),
};
const manifestPath='.ref/applications/synapse/macro/asset-manifest.json';
const manifest=JSON.parse(fs.readFileSync(path.join(root,manifestPath),'utf8'));
const unmatchedClasses=['emoji_popup_main_label','emoji_popup_main_item'];
const css=[...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css')).map(file=>{
  const sourcePath='.ref/applications/synapse/macro/'+file.slice(2);
  const text=fs.readFileSync(path.join(root,sourcePath),'utf8');
  const rules=parseCSS(text);
  for(const name of unmatchedClasses) if(rules.some(rule=>new RegExp('\\.'+name+'(?![\\w-])').test(rule.selector)))
    throw Error('Previously unscoped Macro emoji class now has a matching rule: '+name);
  return {path:sourcePath,sha256:hash(text),rules:rules.filter(rule=>
    /CustomModal_|EmojiPopup_|^\.thx-btn|^\.keymap-action|^\.scrollable|^\.mb20/.test(rule.selector))};
});
const embedded=fs.readFileSync(path.join(root,'assets/synapse/embedded.rs'),'utf8');
const assets=[
 ['icon_emoji.07a71bf8.svg','shortcuts-emoji.svg'], ['icon_emoji-2.b1a4efad.svg','shortcuts-emoji-active.svg'],
 ['icon_charactermap.025aae0b.svg','shortcuts-character.svg'], ['icon_charactermap-10.3c3d7fb3.svg','shortcuts-character-active.svg'],
 ['icon_search.b84dee08.svg','shortcuts-emoji-search.svg'], ['icon_close_enclosed.6056b667.svg','shortcuts-search-clear.svg'],
 ['icon_close_enclosed_a.83aff4bb.svg','shortcuts-search-clear-active.svg'], ['icon_close.4f578909.svg','macro/binding-close.svg'],
 ['close.1d7eff2a.svg','macro/close.svg'],
].map(([file,output])=>{
  const sourcePath='.ref/applications/synapse/macro/static/media/'+file;
  const destination='assets/synapse/'+output;
  const bytes=fs.readFileSync(path.join(root,sourcePath));
  if(!bytes.equals(fs.readFileSync(path.join(root,destination))) || !embedded.includes('"synapse/'+output+'"'))
    throw Error('Unregistered or different Macro text resource: '+destination);
  return {source:sourcePath,output:destination,sha256:hash(bytes),byte_equal:true};
});
const evidence={
  method:'Current manifest-scoped Acorn and CSS parsing; no vendor code execution',
  components:Object.fromEntries(['dn','on','en','an','tn','$t','Gt','Pt','vt','ze'].map(name=>[name,receipt(name)])),
  labels:Object.fromEntries(['$V8','ZDi','pjv','qQ4','Hiw','Y6o'].map(name=>[name,source.literal(37927,source.exported(37927,name))])),
  action:source.literal(4173,source.exported(4173,'Lt')),
  native:{verification:'Reviewed file fingerprints; not rendered or runtime verification',files:Object.fromEntries([
    'crates/razer-app-pages/src/macro_page.rs','crates/razer-app-pages/src/macro_page/body.rs','crates/razer-app-pages/src/macro_page/text.rs',
    'crates/razer-app-pages/src/macro_page/text_emoji.rs','crates/razer-app-pages/src/macro_page/text_overlay.rs','crates/razer-pages/src/features/shortcuts.rs',
  ].map(file=>[file,hash(fs.readFileSync(path.join(root,file)))]))},
  unscopedCss:{classes:unmatchedClasses,matchingRules:0,scope:'Every stylesheet in the current Macro manifest'},
  css,assets,data_counts:{groups:groups.reduce((sum,group)=>sum+group.values.length,0),search:data.search.length,variants:Object.keys(data.variants).length},
  contract:{
    text:'250 UTF-16 units; emoji appends to the end, checks full length and does not replace selection',
    save:'O -> N and trigger document.body.click() disarm Save. A later nonempty draft change arms it; clearing does not disarm it. Cancel/outside/close restore item.Text. Only popup Save updates the action.',
    placement:'fixed x=trigger.left, y=trigger.bottom+10; if y+250 > innerHeight then y=trigger.top-250; scroll repositions after 150ms debounce and hides when trigger.top+8 leaves item_editor',
    popup:'Single 500ms linear opacity wrapper at top:184px; immediate visibility. Tooltip opacity is 300ms linear with no delay.',
    quirks:['Unknown Windows version retains five cat glyphs; only literal 11 filters them',
      'clear search changes input only; resetEmoji is a no-op and filtered results remain until a later input change',
      'category title uses unscoped emoji_popup_main_label; variant children use unscoped emoji_popup_main_item; scoped CSS does not match them'],
  },
};
for(const [file,value] of [['crates/razer-app-pages/src/macro_page/text_data.json',data],['docs/re/macro-text-current-evidence.json',evidence]]) {
  const target=path.join(root,file), text=JSON.stringify(value,null,2)+'\n';
  if(process.argv.includes('--check')) {if(fs.readFileSync(target,'utf8')!==text)throw Error('Stale '+file);}
  else fs.writeFileSync(target,text);
}
console.log(`Current Macro text: ${evidence.data_counts.groups} category entries, ${data.search.length} search entries, ${Object.keys(data.variants).length} variant lists; nine shared assets verified.`);
