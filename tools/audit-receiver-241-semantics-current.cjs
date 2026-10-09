// Receiver 241 parent tree, state machine and actual transport as static data.
// Reference JS is parsed, never required, imported, evaluated or executed.
const fs = require('fs'), path = require('path');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {CurrentMiddlewareSource} = require('./current-middleware-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const ui = Object.create(Source.prototype);
ui.directory = '.ref/devices/241';
const manifestPath = ui.directory + '/asset-manifest.json';
const manifest = JSON.parse(read(manifestPath));
ui.files = [...new Set(Object.values(manifest.files))]
  .filter(f => /^\.\/static\/js\/[^/]+\.js$/.test(f))
  .map(f => ui.directory + '/' + f.slice(2));
ui.modules = new Map(); ui.texts = new Map(); ui.parsed = new Set();
const mw = new CurrentMiddlewareSource(241);
const receipts = [];
function binding(source, module, name, meaning) {
  const node = source.binding(module, name);
  receipts.push({layer: source === ui ? 'ui' : 'middleware', module, name,
    meaning, ...source.receipt(module, node)});
  return node;
}
function exported(source, module, name, meaning) {
  const node = source.exported(module, name);
  return binding(source, module, node.name, meaning);
}
const uiBindings = {
  D: 'Product SVG banner', Z: 'Modal wrapper', X: 'Imperative modal creation',
  we: 'Mouse-and-keyboard binding predicate; not connectivity',
  Ge: 'Dialog status enum', Je: 'Initial reducer state', Ve: '713 warning dongle',
  Ze: 'Dialog reducer', Qe: 'Command sender', $e: 'Shared timeout cancellation',
  es: 'Candidate list', ss: 'Precautions and result footer', ts: 'Dialog presentation parent',
  as: 'Keyboard-left, mouse-right layout', is: 'Per-column state render',
  os: 'Device image, Add, retry, Unpair', cs: 'Device image lookup',
  ds: 'Dialog controller and response consumption', ps: 'Connected runtime storage subscription',
  Ds: 'Localized product name lookup', Ns: 'Name capitalization', gs: 'Raw connected status 1',
  Es: 'Navigation PID/edition eligibility', hs: 'Connection predicate', fs: 'Delayed target-page broadcast',
  xs: 'PID number normalization', ys: 'Polling property names', Ss: 'Polling property presence',
  Cs: 'Local profile polling capability predicate', Rs: 'Mixed original/dock dongle records',
  _s: 'Original dongle warning sources', Ps: 'Parent pairing widget and I/Y/K/W/J/V',
  js: 'Memoized parent', Us: 'Product banner selection', bs: 'Pairing page body root',
};
for (const [name, meaning] of Object.entries(uiBindings)) binding(ui, 3746, name, meaning);
for (const [name, meaning] of Object.entries({Oi:'Pairing import', on:'Profile bar exclusion',
  nn:'Normal product root, nav, header and active view', rn:'Redux normal root',
  ln:'READY gate', Y:'Delayed setup wrapper', J:'Installation content and cancel/retry'}))
  binding(ui, 9163, name, meaning);
for (const [id, name, meaning] of [[3358,'r','Body wrapper scroll and ref'],
  [3525,'s','Main container'], [1422,'s','Body widgets'], [6299,'p','Widget container and tooltip']])
  binding(ui, id, name, meaning);
exported(ui, 8193, 'DeviceInfo', 'UI product 241 feature descriptor');
exported(ui, 9228, 'Dh', 'Setup status enum');
const mwBindings = {
  _e:'V2 binding query and catalog transformation', Ke:'Action dispatch table', qe:'Dispatch entry',
  Se:'Bind and hardware callback registration', Te:'Unbind, retry and cleanup',
  Ee:'Pair status hardware response', ye:'Local storage and runtime publishing side effects',
  oe:'Slave connection broadcast and publishing', Ne:'Observed device publication loop',
};
for (const [name, meaning] of Object.entries(mwBindings)) binding(mw, 34340, name, meaning);
binding(mw, 45601, '_e', 'Broadcast registration USER_COMMAND -> fe');
binding(mw, 45601, 'fe', 'USER_COMMAND -> pairing dispatcher -> ordinary event queue');
exported(mw, 20236, 'f2', 'Select category-specific rzDevice instance');
for (const id of [48320,14770]) binding(mw, id, 'f', 'Category-specific class extending rzDevice25');
for (const [name, meaning] of Object.entries({VO:'V2 reply parser', ox:'Bind payload and mode',
  Mk:'Unbind payload', O7:'Scan payload and result enum'})) exported(mw,84816,name,meaning);
for (const name of ['IZ','ES','fr','AL','jI','$L','pJ','x$']) exported(mw,30580,name,'Protocol header / description');
exported(mw,99494,'NH','apiElectron.doRzDeviceAction bridge, distinct from DLL bridges');
walk(mw.binding(7755,'il'), node => {
  if (node.type !== 'MethodDefinition' || !['constructor','sendCommand','_createDataSend',
    '_getUSBTransferInResult','getMultipleDeviceWirelessConnectionStatusV2',
    'deviceScan','setDevicePairingMode','setDeviceUnpair'].includes(key(node.key))) return;
  receipts.push({layer:'middleware',module:7755,name:'il.'+key(node.key),
    meaning:'rzDevice25 actual protocol / transport method',...mw.receipt(7755,node)});
});
const hostReceipts=[];
for(const file of ['.ref/host-4.0.827/electron/preload.js',
  '.ref/host-4.0.827/electron/main.js', '.ref/host-4.0.827/electron/UsbRzDeviceAction.js']){
  const text=read(file);
  walk(acorn.parse(text,{ecmaVersion:'latest'}),node=>{
    let name;
    if(node.type==='Property'&&key(node.key)==='doRzDeviceAction')name='apiElectron.doRzDeviceAction';
    if(node.type==='CallExpression'&&node.callee.type==='MemberExpression'&&
      key(node.callee.property)==='handle'&&node.arguments[0]?.value==='rzDeviceAction')name='rzDeviceAction IPC';
    if(node.type==='SwitchCase'&&['hid.sendFeatureReport','hid.sendFeatureReportMutex','hid.getFeatureReport'].includes(node.test?.value))name=node.test.value;
    if(name)hostReceipts.push({layer:'host',name,path:file,sha256:hash(text),offset:node.start,end:node.end,source:text.slice(node.start,node.end)});
  });
}
// Source fragments are receipts, not runtime evaluations. Derived facts are tied
// to the AST nodes which were manually followed branch by branch in the MD.
const parent = ui.binding(3746,'Ps');
const parentExpressions = [];
walk(parent, node => {
  if(node!==parent&&/Function/.test(node.type))return false;
  if (node.type === 'VariableDeclarator' && ['I','W','K','Y','J','V'].includes(node.id.name))
    parentExpressions.push({name:node.id.name,...ui.receipt(3746,node)});
});
const effects = [];
for (const name of ['Ps','ds','ps']) walk(ui.binding(3746,name),node=>{
  if (node.type !== 'CallExpression') return;
  const callee=node.callee.type==='SequenceExpression'?node.callee.expressions.at(-1):node.callee;
  if(callee.type!=='MemberExpression'||key(callee.property)!=='useEffect') return;
  const callback=node.arguments[0];
  const directReturns=callback.body?.type==='BlockStatement'
    ? callback.body.body.filter(n=>n.type==='ReturnStatement') : [];
  effects.push({component:name,direct_return_count:directReturns.length,
    concise_callback:callback.body?.type!=='BlockStatement',...ui.receipt(3746,node)});
});
const css=[...new Set(Object.values(manifest.files))]
  .filter(f=>/^\.\/static\/css\/[^/]+\.css$/.test(f)).map(f=>{
    const file=ui.directory+'/'+f.slice(2),text=read(file);
    const rules=parseCSS(text).filter(r=> /(?:main-container|body-wrapper|body-widgets|widget-container|widget-prod|dot-bg|img-text|HyperPollingWirelessUma_|HyperPollingWirelessMouseDock_|Duallink_|duallink-|box-item|modal_|^\*|^body\b|^html\b|^\.flex\b|scrollable)/.test(r.selector));
    return{path:file,sha256:hash(text),rules};
  }).filter(f=>f.rules.length);
const result={
  scope:'Current product 241 normal root and Pairing parent/dialog semantic audit; Lighting/Help bodies, physical native binary implementation and runtime/DPI verification remain separate.',
  method:'Manifest-scoped static AST, HTTP-verified middleware source and CSS rules. No vendor JS, application, DLL, build or tests executed.',
  offset_unit:'UTF-16 code units; end exclusive; SHA-256 of actual UTF-8 source bytes',
  generator_sha256:hash(fs.readFileSync(__filename)),
  manifest:{path:manifestPath,sha256:hash(read(manifestPath))},
  receipts,host_receipts:hostReceipts,parent_expressions:parentExpressions,effects,css,
  middleware_acquisition:mw.acquisition,
};
const target='docs/re/receiver-241-semantics-current-evidence.json';
const serialized=JSON.stringify(result,null,2)+'\n';
if(process.argv.includes('--check')){if(read(target)!==serialized)throw Error('Stale '+target);}
else fs.writeFileSync(path.join(root,target),serialized);
console.log(`Receiver 241 semantics: ${receipts.length} AST receipts, ${hostReceipts.length} host anchors, ${parentExpressions.length} parent predicates, ${effects.length} effect receipts, ${css.reduce((n,f)=>n+f.rules.length,0)} CSS rules.`);
