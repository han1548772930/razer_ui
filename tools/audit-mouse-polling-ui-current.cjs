// Static current 182 button -> state event -> profile tasks -> Rust consumer.
// Vendor JavaScript is parsed as source data and never imported or evaluated.
const fs = require('fs'), path = require('path'), assert = require('assert');
const acorn = require('acorn');
const {walk, hash} = require('./webpack-source.cjs');
const {inspect} = require('./device-query-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const model = JSON.parse(read('docs/re/mouse-polling-model-current-evidence.json'));
const page = model.evidence.find(row => row.product_id === 182);
const manifest = JSON.parse(read(page.manifest.path));
assert.equal(hash(read(page.manifest.path)), page.manifest.sha256);
const receipts = [];
for (const symbol of ['OM', 'lM', 'dm', 'Rm', 'yE', '1057:DeviceInfo']) {
  const receipt = page.receipts.find(row => row.symbol === symbol);
  assert(receipt, 'Missing current polling mount ' + symbol);
  const text = read(receipt.path);
  assert.equal(receipt.path, '.ref/devices/182/' + manifest.files['main.js'].slice(2));
  assert.equal(hash(text), receipt.sha256);
  assert.equal(text.slice(receipt.offset, receipt.end), receipt.source);
  receipts.push({label:'current UI ' + symbol, ...receipt});
}
const uiSource = symbol => receipts.find(row => row.symbol === symbol).source;
assert(uiSource('Rm').includes('this.onClickPollingRateButton=e=>'));
assert(uiSource('Rm').includes('this.props.isDongle?this.props.setPollingRateWireless(e):this.props.setPollingRate(e)'));
assert(uiSource('Rm').includes('e[i]!==o&&this.props[a?"setPollingRateWireless":"setPollingRate"](o)'));
assert(uiSource('1057:DeviceInfo').includes('OBMSlots:1'));
const pagePath = receipts[0].path, pageText = read(pagePath);
const ast = acorn.parse(pageText, {ecmaVersion:'latest'});
const actions = [], reducers = [];
walk(ast, node => {
  if (node.type === 'VariableDeclarator' && ['am', 'Em'].includes(node.id.name)
      && node.init?.type === 'ArrowFunctionExpression') {
    const source = pageText.slice(node.init.start, node.init.end);
    if ((node.id.name === 'am' && source.includes('type:Ne.QF$'))
        || (node.id.name === 'Em' && source.includes('type:vE'))) actions.push({node:node.init, symbol:node.id.name});
  }
  if (node.type === 'FunctionExpression' && node.end - node.start < 8000) {
    const source = pageText.slice(node.start, node.end);
    if (source.includes('case Ne.QF$:') && source.includes('case vE:')
        && source.includes('"ON_POLLING_RATE"')) reducers.push(node);
  }
});
assert.equal(actions.length, 2, 'Ambiguous current polling action');
assert.equal(reducers.length, 1, 'Ambiguous current polling reducer');
for (const action of actions) receipts.push({label:'polling action ' + action.symbol,
  path:pagePath, sha256:hash(pageText), offset:action.node.start, end:action.node.end,
  source:pageText.slice(action.node.start, action.node.end)});
const reducer = reducers[0];
receipts.push({label:'wired/wireless event broadcast and UI action reducer',
  path:pagePath, sha256:hash(pageText), offset:reducer.start, end:reducer.end,
  source:pageText.slice(reducer.start, reducer.end)});
const plans = JSON.parse(read('.ref/middleware/receiver-protocol-requests.json'));
const boots = JSON.parse(read('.ref/middleware/bootstrap-requests.json'));
const audit = inspect(plans.products.find(row => row.product_id === 182), boots.products.find(row => row.product_id === 182));
const dispatches = [];
for (const [id, module] of audit.source.modules) walk(module.fn, node => {
  if (node.type === 'SwitchCase' && node.test?.value === 'ON_POLLING_RATE'
      && audit.source.snippet(id,node).includes('executeEventTask')) dispatches.push({id,node});
});
assert.equal(dispatches.length, 1);
const dispatch = dispatches[0];
assert(audit.source.snippet(dispatch.id,dispatch.node).includes('r=n||t'));
receipts.push(audit.record('polling event -> ordinary or controller task maker',dispatch));
let maker;
walk(dispatch.node, node => {
  if (node.type === 'MemberExpression' && node.property.name === 'DF7') maker = audit.resolve(dispatch.id,node);
});
assert(maker, 'Missing ordinary polling maker');
const makerSource = audit.source.snippet(maker.id,maker.node);
for (const part of ['RzVersion53bits.nextVersion', 'p.pollingRateWireless=n', 'p.pollingRate=n',
  '.enqueueTask(', 'RzTaskCategory.NORMAL_SKIPPABLE', 'u.$AI.bind(', 'u.G7J.bind(',
  'u.I_I.bind(', 'u.lgd.bind(', '.setMemoryStorageItem('])
  assert(makerSource.includes(part), 'Polling maker changed: ' + part);
receipts.push(audit.record('profile/version persistence, in-game override and task enqueue',maker));
const targets = new Map();
walk(maker.node, node => {
  if (node.type === 'MemberExpression' && ['$AI','G7J','I_I','lgd'].includes(node.property.name))
    targets.set(node.property.name, audit.resolve(maker.id,node));
});
assert.equal(targets.size,4);
for (const [symbol,target] of targets) receipts.push(audit.record('polling task/callback ' + symbol,target));
const task = targets.get('$AI'), taskSource = audit.source.snippet(task.id,task.node);
for (const part of ['params:{pollingRate:o}', 'o>1e3&&!i', 'yield(0,p.gN)()', 'yield(0,p._4)(o)',
  'r!==o', 'new Error("data not match")', 'signal.aborted', 'taskRunnerPollingRateMemoryStorageCache'])
  assert(taskSource.includes(part), 'Polling task changed: ' + part);
const branches = new Map();
walk(task.node, node => {
  if (node.type === 'MemberExpression' && ['gN','_4'].includes(node.property.name))
    branches.set(node.property.name, audit.resolve(task.id,node));
});
assert.equal(branches.size,2);
for (const [symbol,target] of branches) {
  const source = audit.source.snippet(target.id,target.node);
  assert(source.includes('isObm') && source.includes('pollingRate')
    && source.includes(symbol === 'gN' ? 'getProfilePollingRate(e)' : 'setProfilePollingRate(t,e)'));
  receipts.push(audit.record('profile polling helper ' + symbol,target));
}
const slots = [];
walk(audit.source.modules.get(maker.id).fn,node => {
  if (node.type === 'FunctionDeclaration' && node.id?.name === 'Ye') slots.push({id:maker.id,node});
});
assert.equal(slots.length,1);
const slotSource = audit.source.snippet(maker.id,slots[0].node);
assert(slotSource.includes('n.guid===e&&t.push(n.slotId)'));
receipts.push(audit.record('OBM slots selected by active profile GUID',slots[0]));
const obm = targets.get('I_I'), obmSource = audit.source.snippet(obm.id,obm.node);
assert(obmSource.includes('Promise.all(i.map(') && obmSource.includes('(yield(0,p.gN)(e))!==o?yield(0,p._4)(o,e)'));
const featurePath = 'crates/razer-pages/src/features/mouse_polling_profile.rs';
const feature = read(featurePath), workspace = read('crates/razer-pages/src/features/workspace.rs');
assert(workspace.includes('MousePollingRequested {'));
for (const part of ['polling_pending: Option<(MousePollingScope, PollingField, u32)>',
  '.disabled(self.mouse_settings_write_pending())', 'this.request_mouse_polling(field, rate, cx)',
  'self.device.product_id != 182', 'pub fn mouse_polling_in_flight_matches',
  'observed.filter(|rate| scope_current && *rate == hz)', 'values.polling_hz = Some(observed_hz)'])
  assert(feature.includes(part), 'Rust polling consumer changed: ' + part);
const reset = feature.slice(feature.indexOf('pub(super) fn reset_profile'),feature.indexOf('pub(super) fn sync_profile'));
assert(!reset.includes('polling_pending = None'), 'Restore released in-flight write slot');
const connection = feature.slice(feature.indexOf('pub fn observe_mouse_polling'),feature.indexOf('pub(in crate::features) fn mouse_polling_visible'));
assert(!connection.includes('polling_pending = None'), 'Disconnect released in-flight write slot');
const click = feature.slice(feature.indexOf('.on_click(cx.listener(move |this, _, window, cx|'), feature.indexOf('.when(value >'));
assert(!click.includes('this.mouse_polling_value(field) == rate'), 'Local draft prevents retry of selected rate');
assert(feature.includes('self.mouse_polling.idle_pending.is_some() || self.mouse_polling.polling_pending.is_some()')
  && feature.includes('self.device.product_id != 182 || self.mouse_settings_write_pending()')
  && click.includes('this.mouse_settings_write_pending()'));
assert(workspace.includes('target == Control::Idle && this.mouse_settings_write_pending()')
  && read('crates/razer-pages/src/features/device_pages.rs').includes('|| self.mouse_settings_write_pending()'));
const nativeHid = read('crates/razer-hid/src/transport/hidapi.rs');
assert(nativeHid.includes('md5::compute(&selected.path)') && nativeHid.includes('lock.try_lock()')
  && nativeHid.includes('_lock: lock') && nativeHid.includes('_lock: File'));
const shell = read('crates/razer-shell/src/shell/mouse_polling_write.rs').replace(/\s+/g,'');
const direct = read('crates/razer-discovery/src/direct/mod.rs').replace(/\s+/g,'');
for (const part of ['mouse_polling_request_matches(scope,field,hz,cx)',
  'field!=PollingField::Wired', 'direct::resolve(&mutclient,&selected,cap)?.write(&mutclient,DeviceWriteSetting::Polling{hz})?',
  'DeviceReadValue::Polling{hz:observed}ifobserved==hz', 'current.hid_node()==route.hid_node()',
  'mouse_polling_in_flight_matches(scope,field,hz,cx)', 'finish_mouse_polling(scope,field,hz,observed,cx)',
  'ServiceRequest::Shutdown', 'this.device_value_owners.remove(&identity)', 'this.device_read_scopes.remove(&identity)'])
  assert(shell.includes(part), 'Polling transport/response gate changed: ' + part);
for (const part of ['ServiceRequest::HidNodeWrite', 'ServiceRequest::DeviceWrite',
  'reply["result"]["requested"]==serde_json::to_value(&setting)?', 'reply["result"]["verified"]==true',
  'setting.matches_value(&observed)', 'self.check_reply(&reply)?', 'node.interface_number==i32::from(cap.claim_interface_for(u32::from(node.product_id))?)'])
  assert(direct.includes(part), 'Shared direct route gate changed: ' + part);
const report = {schema_version:1, product_id:182, source_manifest:page.manifest, source_receipts:receipts,
  source_semantics:{choice:'Rm button immediately dispatches wired or wireless by isDongle; selecting existing value still dispatches',
    action:'am/Em -> reducer ON_POLLING_RATE with pollingRate or pollingRateWireless -> event task dispatcher',
    profile:'task maker mutates active profile field, advances 53-bit version and saves/broadcasts profile before enqueueing hardware task',
    current_task:'read -> skip if equal -> setter -> readback exact comparison; abort checked between operations; >1000 clamped absent highspeed',
    obm_task:'activeProfile GUID selects all matching obmData.profiles slotIds; separate NORMAL task compares each slot getter then calls setter',
    cache:'successful current task updates currValue, lastModifiedDate and versioned task memory cache',
    cleanup:'completion/error callbacks dispatch device status and post-process cleanup; no device rollback'},
  implementation:{page:'182 button edits local draft then emits typed field/hz intent; shared Idle/Polling UI write slot disables both controls and rejects both intents until actual completion',
    serialization_scope:'current retained mouse UI only, based on original NORMAL_SKIPPABLE task serialization; not a cross-process HID lock or the complete task queue',
    native_serialization:'existing portable NativeBackend holds a path-keyed OS file try_lock for the entire handle lifetime across worker processes; contention fails explicitly rather than queueing',
    transport:'typed field/hz intent -> shell mouse_polling_write -> shared direct wired route -> worker HidNodeWrite or isolated Windows DeviceWrite; source capability selects profile1 polling',
    response:'route checks collection/container, exact requested setting, verified flag and typed matching polling value; shell verifies observed Hz and retained identity/node/revision before publishing',
    failure:'wireless and absent/ambiguous wired routes explicitly fail before mutation; service errors leave draft and actual observations separate; same selection can retry',
    cleanup:'Shutdown attempted on success and failure; a real device acknowledgement survives shutdown failure; older queued discovery reads invalidated',
    refresh:'exact current-scope readback observes Rate; wired readback merges polling_hz into device and saved observations, preserving local draft',
    stale_reply:'profile/connection changes retain worker token; stale completion releases token without publishing old observations',
    retry:'same local selection can be clicked again after failure; displayed draft never treated as device ack'},
  gaps:['Original taskMaker profile save/version broadcast, in-game override, NORMAL_SKIPPABLE queue and versioned memory cache are not reproduced by this page connection.',
    'OBM active-profile slot list and independent multi-slot task are not connected to retained profile storage; default profile1 current-setting write is separate.',
    'Other mouse polling pages retain local drafts; receiver-relayed/BLE transport branches remain unresolved and must report unavailable.',
    'No caller abort propagation into an already dispatched worker; old-scope replies are rejected but device writes cannot be rolled back.',
    'The UI write slot and existing native fail-on-contention file lock do not reproduce original task queue waiting, skipping or coalescing semantics.',
    'No runtime acceptance was executed.'],runtime_acceptance:'not_run'};
const output='docs/re/mouse-polling-ui-current-evidence.json', serialized=JSON.stringify(report,null,2)+'\n';
if (process.argv.includes('--write')) fs.writeFileSync(path.join(root,output),serialized);
else assert.equal(read(output).replace(/\r\n/g,'\n'),serialized,'Polling UI receipts changed');
console.log('182 polling UI: button, actions, profile/OBM tasks and retained readback consumer statically checked.');
