// Maintained Acorn-only inspection. Never execute vendor JS or load DLLs.
const fs = require('node:fs'), path = require('node:path'), acorn = require('acorn');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), source = new Source('synapse/macro');
const main = source.files.find(f => /\/main\.[^/]+\.js$/.test(f));
if (!main) throw Error('No current Macro main bundle');
const text = source.text(main), ast = acorn.parse(text, {ecmaVersion: 'latest'});
let business;
walk(ast, node => {
  if (node.type !== 'BlockStatement') return;
  const bindings = new Map();
  for (const statement of node.body) if (statement.type === 'VariableDeclaration') {
    for (const declaration of statement.declarations) if (declaration.id.type === 'Identifier')
      bindings.set(declaration.id.name, declaration.init);
  }
  if (['_r','wr','br','Nr','Rr','Or','dr'].every(name => bindings.has(name))) business = bindings;
});
if (!business) throw Error('Current Macro business scope not found');
const receipt = (file, text, node) => ({path:file, sha256:hash(text), offset:node.start, end:node.end, source:text.slice(node.start,node.end)});
const mainReceipt = name => receipt(main, text, business.get(name));
const mainCode = name => mainReceipt(name).source;
function requireText(code, snippets) {for (const value of snippets) if (!code.includes(value)) throw Error('Source contract changed: '+value);}
requireText(mainCode('_r'), ['JSON.parse(e.macroItem)', 't.pop()', 'parseInt(n.timerTick)+1', 'cr.includes(n.vkCode)']);
requireText(mainCode('wr'), ['Dr.onMacroStarted=Date.now()', 'sr.send("RECORDING"']);
requireText(mainCode('Nr'), ['n.time/1e3', 'ar(_)', 'O[0].Number=o.toString()', 'a.top!==s.top']);
const converter = source.receipt(13139, source.binding(13139, 'G'));
requireText(converter.source, ['Number(e.time)/1e6', 'this.timeId===o', '131072===e.data?4:3']);
const combine = source.receipt(13139, source.binding(13139, 'H'));
requireText(combine.source, ['KeyEvent.State%2===0', 'o.has(i)', 'parseFloat(t.Number)+parseFloat(r.Number)']);
const preview = source.receipt(58190, source.binding(58190, 'le'));
requireText(preview.source, ['fake_ui_container','r.children.length>50&&r.removeChild(r.firstChild)','this.keydownDelay+=Number(parseFloat(e.time)/1e6)','131072===e.data?4:3']);
const phased = source.receipt(58190, source.binding(58190, 'Ka'));
requireText(phased.source, ['changeRecording:E,countDownRef:u}=xa(e)', 'onClick:()=>{return e=t.key,l(e),void E();var e}']);
const manifest = JSON.parse(fs.readFileSync(path.join(root,source.directory,'asset-manifest.json'),'utf8'));
const css = [...new Set(Object.values(manifest.files))].filter(file=>file.endsWith('.css')).map(file=>{
  const name=source.directory+'/'+file.slice(2), text=fs.readFileSync(path.join(root,name),'utf8');
  return {path:name,sha256:hash(text),rules:parseCSS(text).filter(rule=>/SplitBtn_(?:split_btn|countdown)/.test(rule.selector))};
}).filter(file=>file.rules.length);
const hostPath = '.ref/host-4.0.827/electron/modules/mapping_engine/win/index.js';
const hostText = fs.readFileSync(path.join(root,hostPath),'utf8');
const hostAst = acorn.parse(hostText, {ecmaVersion:'latest'});
const names = ['startMacroRecording','stopMacroRecording','registerMacroRecorderEvent','unregisterMacroRecorderEvent','setMacroRecorderEventCallback','disableMapping','enableMapping'];
const host = Object.fromEntries(names.map(name => [name, []]));
walk(hostAst, node => {
  if (node.type === 'Property' && names.includes(key(node.key))) host[key(node.key)].push(receipt(hostPath,hostText,node));
});
for (const name of names) if (!host[name].length) throw Error('Missing current host method '+name);
const nativePaths = ['crates/razer-pages/src/features/macro_library.rs','crates/razer-app-pages/src/macro_page.rs','crates/razer-app-pages/src/macro_page/body.rs','crates/razer-app-pages/src/macro_page/phased.rs','crates/razer-app-pages/src/macro_page/record_options.rs','crates/razer-app-pages/src/macro_page/record_shortcut.rs','crates/razer-app-pages/src/macro_page/recording.rs','crates/razer-app-pages/src/macro_page/recording_actor.rs','crates/razer-app-pages/src/macro_page/recording_decode.rs','crates/razer-app-pages/src/macro_page/row_view.rs'];
const native = Object.fromEntries(nativePaths.map(file => [file,hash(fs.readFileSync(path.join(root,file)))]));
const result = {
  method:'Manifest-scoped current Macro Acorn parse and current host static ABI inspection; no vendor execution, DLL loading or runtime tests',
  main:Object.fromEntries(['_r','wr','br','Or','Nr','Rr','dr'].map(n=>[n,mainReceipt(n)])),
  ui:Object.fromEntries(['xa','Gr','Fr'].map(n=>[n,source.receipt(58190,source.binding(58190,n))])),
  converter,combine,preview,phased,css,host,native,
  contract:{
    state:'start accepted is not started; await started, suspend only this session mappings; stop accepted is not stopped; await stopped, decode local draft, restore mappings and shut down on actor thread',
    time:'Item time is microseconds; callback time_tick is identity only. Initial recorded delay is first callback receipt minus started receipt in milliseconds.',
    rows:'Standard recorded/fixed/random/no-delay; source zero fixed delay omits delay rows; Sequence releases and wheel; Phased selected phase. Local pair IDs use document high-water mark.',
    raw:'Each surviving recorded input row retains original callback JSON, including preceding mouse movement buffer. No coordinate transformation or fabricated monitor metadata.',
    preview:'Separate current 58190 le converter; temporary latest 50 rows and scroll follow after append. Preview can differ from final 13139 H normalization. UI drafts, saved baseline and undo remain unchanged until valid stopped and cleanup completion.',
    failure:'Malformed JSON/events, callback overflow, missing started/stopped, unknown keys and bounded session limits fail without changing baseline. Escape cancels. UI cancellation rejects even a completed result awaiting cleanup. Unexpected actor channel closure clears preview and reports unconfirmed native cleanup. UI drop performs worker cleanup.',
    phase:'Ka phase record icon selects its phase, then invokes xa.changeRecording; native phase record follows the same recorder/countdown entry.',
    limitations:'Movement recording unavailable pending monitor queries; shortcut settings remain local and cannot start/stop global recording. Runtime behavior is unexecuted under current verification restriction.',
    test_support:['macro-window','macro-item-list','macro-phased-item-list','macro-phase-record','macro-record','macro-record-options','macro-record-settings','macro-record-start-delay','macro-record-shortcut','macro-record-shortcut-clear','macro-record-countdown','macro-recording-status','macro-recording-error','macro-recording-row','macro-action-row','macro-save','macro-undo','macro-redo'],
  },
};
const out = path.join(root,'docs/re/macro-recording-current-evidence.json'), serialized = JSON.stringify(result,null,2)+'\n';
if (process.argv.includes('--check')) {if(fs.readFileSync(out,'utf8') !== serialized) throw Error('Stale macro recording audit');}
else fs.writeFileSync(out,serialized);
console.log('Current Macro recording lifecycle, event conversion, current host ABI and native instrumentation statically verified.');
