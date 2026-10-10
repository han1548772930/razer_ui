// Current 182 UI release/caller -> middleware setter/readback -> Rust consumer.
// Treat vendor JS only as Acorn source data; never import/evaluate it.
const fs = require('fs'), path = require('path'), assert = require('assert');
const acorn = require('acorn');
const {walk, hash} = require('./webpack-source.cjs');
const {inspect} = require('./device-query-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const powers = JSON.parse(read('docs/re/power-range-current-evidence.json'));
const page = powers.products.find(row => row.product_id === 182);
const manifestPath = '.ref/devices/182/asset-manifest.json';
const manifest = JSON.parse(read(manifestPath));
assert.equal(page.path, '.ref/devices/182/' + manifest.files['main.js'].slice(2));
const text = read(page.path), receipts = [];
assert.equal(hash(text), powers.source_inputs[page.path]);
for (const symbol of ['MM', 'mM', 'DM', 'CM', 'CD', '130.T', '130.I']) {
  const node = page.nodes.find(node => node.symbol === symbol);
  assert(node, 'Missing current power mount ' + symbol);
  assert.equal(text.slice(node.offset, node.end), node.source);
  receipts.push({label:'current UI ' + symbol, path:page.path,
    sha256:hash(text), ...node});
}
const source = symbol => receipts.find(receipt => receipt.symbol === symbol).source;
assert(source('CM').includes('this.props.setPowerSaving(e)'));
assert(source('CM').includes('min:1,max:15,step:1,value:this.props.powerSavingValue'));
assert(!source('CM').includes('callOnChangeOnEveryStep'));
assert(source('130.T').includes('this.onMouseUp=')
  && source('130.T').includes('this.props.changeValue(this.state.value,')
  && source('130.T').includes('window.removeEventListener("mouseup"'));
const nodes = [];
walk(acorn.parse(text, {ecmaVersion:'latest'}), node => {
  if (node.type === 'VariableDeclarator' && node.id.name === 'SM'
      && node.init?.type === 'ArrowFunctionExpression'
      && text.slice(node.init.start, node.init.end).includes('"setPowerSaving"')) {
    assert.equal(text.slice(node.init.start, node.init.end),
      'e=>E=>{console.log("setPowerSaving",e),E({type:Ne.S1_,payload:e})}');
    nodes.push({label:'setPowerSaving action', node:node.init});
  }
  if (node.type === 'FunctionExpression') {
    const code = text.slice(node.start, node.end);
    if (code.includes('case Ne.S1_:') && code.includes('ON_SET_POWER_SAVING_VALUE')
        && code.length < 5000) nodes.push({label:'power action broadcast/reducer', node});
  }
});
assert.equal(nodes.length, 2, 'Ambiguous release action/reducer');
for (const {label, node} of nodes) receipts.push({label, path:page.path,
  sha256:hash(text), offset:node.start, end:node.end, source:text.slice(node.start,node.end)});
const writes = JSON.parse(read('docs/re/device-write-capabilities-current-evidence.json'));
const write = writes.products.find(row => row.product_id === 182);
const task = write.receipts.find(receipt => receipt.label === 'idle actual write/readback and product units');
assert(task && task.path.startsWith('.ref/middleware/182/'));
const taskText = read(task.path);
assert.equal(hash(taskText), task.sha256);
assert.equal(taskText.slice(task.offset, task.end), task.source);
for (const part of ['let r=60*o', 'singleProfileDevice&&(r=o)', '.setTimeToSleep(r)',
  '.getTimeToSleep()', 'new Error("data not match")', 'signal.aborted', '.setMemoryStorageItem('])
  assert(task.source.includes(part), 'Current idle task changed: ' + part);
receipts.push(task);
const plans = JSON.parse(read('.ref/middleware/receiver-protocol-requests.json'));
const boots = JSON.parse(read('.ref/middleware/bootstrap-requests.json'));
const audit = inspect(plans.products.find(row => row.product_id === 182), boots.products.find(row => row.product_id === 182));
const dispatches = [];
for (const [id, module] of audit.source.modules) walk(module.fn, node => {
  if (node.type === 'SwitchCase' && node.test?.value === 'ON_SET_POWER_SAVING_VALUE'
      && audit.source.snippet(id, node).includes('executeEventTask')) dispatches.push({id,node});
});
assert.equal(dispatches.length, 1, 'Ambiguous source power event dispatcher');
const dispatch = dispatches[0];
receipts.push(audit.record('power state event -> task maker', dispatch));
let makers = [];
walk(dispatch.node, node => {
  if (node.type === 'CallExpression' && node.callee.type === 'SequenceExpression') makers.push(audit.resolve(dispatch.id,node.callee));
});
assert.equal(makers.length,1);
const maker = makers[0];
const makerSource = audit.source.snippet(maker.id,maker.node);
for (const part of ['RzTaskCategory.NORMAL_SKIPPABLE', 'RzVersion53bits.nextVersion', 'newPowerSavingValue:n', '.enqueueTask('])
  assert(makerSource.includes(part), 'Power task maker changed: ' + part);
receipts.push(audit.record('power version/profile persistence and task enqueue',maker));
const callbacks=[];
walk(maker.node,node=>{
  if(node.type!=='CallExpression'||node.callee.type!=='MemberExpression') return;
  const member=node.callee.property.name;
  if(!['ou3','taw'].includes(member)) return;
  const target=audit.resolve(maker.id,node.callee);
  receipts.push(audit.record(member==='ou3'?'queued task -> sleep setter/readback':'completion/error -> UI action cleanup',target));
  if(member==='ou3') assert.equal(audit.source.snippet(target.id,target.node),task.source);
  callbacks.push(member);
});
assert.deepEqual(callbacks.sort(),['ou3','taw']);
const reads = JSON.parse(read('docs/re/mouse-read-capabilities-current-evidence.json'));
const info = reads.products.find(row => row.product_id === 182).device_info;
const infoText = read(info.receipt.path);
assert.equal(hash(infoText), info.receipt.sha256);
assert.equal(infoText.slice(info.receipt.offset, info.receipt.end), info.receipt.source);
const expression = acorn.parseExpressionAt(infoText, info.receipt.offset, {ecmaVersion:'latest'});
const infoNode = expression.type === 'SequenceExpression' ? expression.expressions[0] : expression;
assert.equal(infoNode.type, 'ObjectExpression');
assert(!infoNode.properties.some(property => (property.key.name ?? property.key.value) === 'singleProfileDevice'),
  '182 native timer unit requires a new review');
receipts.push({label:'182 ordinary timer units: no singleProfileDevice field', ...info.receipt});
const ui = read('crates/razer-pages/src/features/workspace.rs');
const feature = read('crates/razer-pages/src/features/mouse_polling_profile.rs');
const shell = read('crates/razer-shell/src/shell/device_discovery.rs');
const direct = read('crates/razer-discovery/src/direct/mod.rs');
assert(ui.includes('SliderEvent::Change(_) if power_range')
  && ui.includes('SliderEvent::Release(value) if power_range')
  && ui.includes('this.request_mouse_idle(value.round() as u8, cx)')
  && ui.includes('target == Control::Idle && this.mouse_settings_write_pending()'));
assert(feature.includes('self.device.product_id != 182')
  && feature.includes('WorkspaceEvent::MouseIdleRequested')
  && feature.includes('values.idle_raw_time = Some(raw_time)')
  && feature.includes('pub fn mouse_idle_in_flight_matches')
  && feature.includes('let scope_current = self.mouse_polling_scope(cx) == Some(scope)'));
assert(feature.includes('self.mouse_polling.idle_pending.is_some() || self.mouse_polling.polling_pending.is_some()')
  && feature.includes('if self.mouse_settings_write_pending()')
  && read('crates/razer-pages/src/features/device_pages.rs').includes('|| self.mouse_settings_write_pending()'));
const resetProfile=feature.slice(feature.indexOf('pub(super) fn reset_profile'),feature.indexOf('pub(super) fn sync_profile'));
assert(!resetProfile.includes('idle_pending = None'), 'Restore released an in-flight write slot');
const connectionUpdate=feature.slice(feature.indexOf('pub fn observe_mouse_polling'),feature.indexOf('pub(in crate::features) fn mouse_polling_visible'));
assert(!connectionUpdate.includes('idle_pending = None'), 'Connection change released an in-flight write slot');
assert(shell.includes('razer_discovery::direct::resolve(')
  && shell.includes('route.write(&mut client, setting)')
  && shell.includes('if observed == raw_time')
  && shell.includes('workspace.finish_mouse_idle(scope, minutes, confirmed, cx)'));
for (const part of ['ServiceRequest::HidNodeWrite', 'ServiceRequest::DeviceWrite',
  'self.check_reply(&reply)?', 'reply["result"]["requested"] == serde_json::to_value(&setting)?',
  'reply["result"]["verified"] == true', 'setting.matches_value(&observed)',
  'current == node', 'reports_match(client, node, cap.report_id, cap.report_bytes)?'])
  assert(direct.includes(part), 'Shared route no longer checks ' + part);
assert(direct.replace(/\s+/g,'').includes('node.interface_number==i32::from(cap.claim_interface_for(u32::from(node.product_id))?)')
  && direct.includes('observed.peer_product_id().is_none()')
  && direct.includes('ObservedTransport::Wired'));
assert(shell.includes('ServiceRequest::Shutdown')
  && shell.includes('self.device_value_owners.remove(&identity)')
  && shell.includes('self.device_read_scopes.remove(&identity)'));
const nativeHid = read('crates/razer-hid/src/transport/hidapi.rs');
assert(nativeHid.includes('md5::compute(&selected.path)') && nativeHid.includes('lock.try_lock()')
  && nativeHid.includes('_lock: lock') && nativeHid.includes('_lock: File'));
const report = {
  schema_version:1, product_id:182, source_manifest:{path:manifestPath,sha256:hash(read(manifestPath))},
  source_receipts:receipts,
  source_semantics:{range_minutes:[1,15], release_action:'setPowerSaving -> ON_SET_POWER_SAVING_VALUE',
    ordinary_raw_multiplier:60, single_profile_raw_multiplier:1,
    response:'setTimeToSleep -> getTimeToSleep -> exact raw comparison',
    original_cancellation:'abort checks before setter and after setter; no device rollback',
    original_cleanup:'mouseup listener removed on unmount',
    original_persistence:'task memory-cache version/timerTick updated after readback'},
  implementation:{release:'retained preview; Release emits intent after local draft edit',
    transport:'shared direct wired route validates retained portable HID collection or isolated Windows container adapter, actual descriptor and interface; worker HidNodeWrite or DeviceWrite',
    response:'target/request/verified/Idle raw value checked before observation',
    refresh:'merge only confirmed idle_raw_time into actual parameter observations; discard older queued discovery reads; draft remains separate',
    failure:'pending cleared; actual service failure shown; draft retained; no fallback or retry mutation',
    stale_reply:'retain in-flight slot across restore/connection epochs; real completion releases token even if stale; current-scope checks gate observation and feedback; no old reply accepted',
    serialization_scope:'Idle and Polling intents share the retained page write slot and disable both controls until completion; not a cross-process HID lock or original task queue implementation',
    native_serialization:'existing portable NativeBackend holds a path-keyed OS file try_lock throughout the opened handle lifetime across worker processes; contention fails explicitly instead of queueing',
    cleanup:'ServiceRequest::Shutdown attempted on success and failure'},
  gaps:[
    'Other 19 mouse product sliders and polling/DPI UI actions are not connected by this change.',
    'Receiver-relayed and BLE idle writes are not implemented; UI reports unavailable before sending.',
    'Original taskMaker profile/version save, NORMAL_SKIPPABLE queue, callback UI actions and taskRunner memory cache persistence are not implemented; local profile Save remains local.',
    'No caller abort propagation into the running worker; stale replies are rejected but already sent writes cannot be rolled back.',
    'The shared UI write slot and existing native fail-on-contention file lock do not reproduce original task queue waiting, skipping or coalescing semantics.',
    'Initial reads remain parameter observations; restored local slider drafts are not replaced by reads.'
  ],
  runtime_acceptance:'not_run'
};
const output='docs/re/mouse-idle-ui-current-evidence.json';
const serialized=JSON.stringify(report,null,2)+'\n';
if(process.argv.includes('--write')) fs.writeFileSync(path.join(root,output),serialized);
else assert.equal(read(output).replace(/\r\n/g,'\n'),serialized, 'Mouse idle UI receipts changed');
console.log('182 Idle UI: current release action, task and consumer statically checked; no runtime acceptance.');
