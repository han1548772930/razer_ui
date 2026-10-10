// Static current-source stage-table capability extraction; never execute vendor JS.
const fs=require('fs'),path=require('path'),assert=require('assert'),acorn=require('acorn');
const {walk,key,hash,Source}=require('./webpack-source.cjs');
const {inspect}=require('./device-query-source.cjs');
const root=path.resolve(__dirname,'..'),body=p=>fs.readFileSync(path.join(root,p),'utf8'),json=p=>JSON.parse(body(p));
const previous=json('docs/re/mouse-dpi-ui-current-evidence.json');
assert.equal(previous.product_id,182);
for(const r of previous.source_receipts){const text=body(r.path);assert.equal(hash(text),r.sha256);assert.equal(text.slice(r.offset,r.end),r.source,'DPI page/middleware receipt changed '+r.label);}
const plans=json('.ref/middleware/receiver-protocol-requests.json'),boots=json('.ref/middleware/bootstrap-requests.json');
const plan=plans.products.find(p=>p.product_id===182),audit=inspect(plan,boots.products.find(p=>p.product_id===182));
const cap=json('assets/data/device-read-capabilities.json').products.find(p=>p.product_id===182);
const prior=json('docs/re/mouse-read-capabilities-current-evidence.json').products.find(p=>p.product_id===182);
const timing=json('assets/data/receiver-query-capabilities.json').capabilities.find(t=>t.source_product_id===182);
assert(cap&&prior&&timing&&cap.source_class===plan.selected_class);
for(const field of ['direct_pids','vendor_id','report_id','report_bytes','claim_interface'])assert.deepEqual(cap[field],prior[field],'Source transport identity changed '+field);
for(const field of ['max_retry_in','max_retry_out','sleep_between_out_ms','sleep_between_out_in_ms','sleep_between_in_ms','transaction_prefix','transaction_modulus'])assert.equal(cap[field],timing[field],'Source transport timing changed '+field);
for(const r of prior.receipts){const text=body(r.path);assert.equal(hash(text),r.sha256);assert.equal(text.slice(r.offset,r.end),r.source,'Transport receipt changed '+r.label);}
assert.deepEqual(previous.source_semantics.protocol.commands,{setDPIStages:[80,4,6],getDPIStages:[80,4,134]});
const set=previous.source_receipts.find(r=>r.label==='inline DPI stages wrapper setDPIStages');
const get=previous.source_receipts.find(r=>r.label==='inline DPI stages wrapper getDPIStages');
assert(set&&get);
for(const token of ['7*o+3','Uint8Array','sendCommand','a.E4(o[t+1])','a.l7(o[t+1])'])assert(set.source.includes(token),'Inline setter semantics changed '+token);
assert(get.source.includes('=-1')&&get.source.includes('sendCommand'),'Getter packet-size default changed');
const manifestPath='.ref/devices/182/asset-manifest.json',manifest=json(manifestPath);
assert.equal(hash(body(manifestPath)),previous.source_manifest.sha256);
const ui=Object.assign(Object.create(Source.prototype),{directory:'.ref/devices/182',files:[...new Set(Object.values(manifest.files))].filter(f=>/\.js$/.test(f)).map(f=>'.ref/devices/182/'+f.replace(/^\.\//,'')),modules:new Map(),texts:new Map(),parsed:new Set()});
const page='.ref/devices/182/'+manifest.files['main.js'].replace(/^\.\//,'');ui.parse(page);
const exported=ui.exported(6482,'an');assert.equal(exported.type,'Identifier');
const choices=ui.binding(6482,exported.name),values=ui.literal(6482,choices);assert.deepEqual(values,[{content:'2'},{content:'3'},{content:'4'},{content:'5'}]);
const editorCounts=values.map(v=>Number(v.content));
const receipts=[...previous.source_receipts,{label:'current editor stage count choices',...ui.receipt(6482,choices)}];
// The current page imports the count choices; retain the actual import and use.
const choiceImports=[],choiceUses=[];
const pageText=body(page);walk(acorn.parse(pageText,{ecmaVersion:'latest'}),n=>{
 if(n.type==='VariableDeclarator'&&n.id?.name==='Wp'&&n.init?.type==='CallExpression'&&n.init.arguments[0]?.value===6482)choiceImports.push(n);
 if(n.type==='MemberExpression'&&n.object?.name==='Wp'&&key(n.property)==='an')choiceUses.push(n);
});
assert.equal(choiceImports.length,1);assert(choiceUses.length>0);
for(const node of [...choiceImports,...choiceUses])receipts.push({label:'current page imports/uses stage count choices',path:page,sha256:hash(pageText),offset:node.start,end:node.end,source:pageText.slice(node.start,node.end)});
const encoding=new Map();walk(audit.method('setDPIStages').node,n=>{if(n.type==='MemberExpression'&&['E4','l7'].includes(key(n.property))){const target=audit.resolve(audit.method('setDPIStages').id,n);encoding.set(key(n.property),target);}});
assert.equal(audit.source.snippet(encoding.get('E4').id,encoding.get('E4').node),'e=>e>>8&255');
assert.equal(audit.source.snippet(encoding.get('l7').id,encoding.get('l7').node),'e=>255&e');
for(const [name,target]of encoding)receipts.push(audit.record('actual big-endian stage encoder '+name,target));
const parser=audit.resolve(audit.method('getDPIStages').id,audit.source.binding(audit.method('getDPIStages').id,'m'));
let decoder;walk(parser.node,n=>{if(n.type==='MemberExpression'&&key(n.property)==='Jz')decoder=audit.resolve(parser.id,n);});
assert(decoder);assert.equal(audit.source.snippet(decoder.id,decoder.node),'(e,t)=>(255&e)<<8|255&t');
receipts.push(audit.record('actual big-endian stage response decoder',decoder));
let slots;walk(audit.source.module(30525).fn,n=>{if(n.type==='FunctionDeclaration'&&n.id?.name==='Ye')slots={id:30525,node:n};});
assert(slots);const slotsText=audit.source.snippet(slots.id,slots.node);
assert(slotsText.includes('n.guid===e&&t.push(n.slotId)')&&slotsText.includes('null!=o?o:[]'));
receipts.push(audit.record('actual active GUID to observed OBM slot IDs',slots));
const infoReceipt=plan.device_info.receipt;assert.equal(hash(body(infoReceipt.path)),infoReceipt.sha256);
const infoNode=[...audit.source.module(plan.device_info.module).definitions.values()].find(n=>n?.start===infoReceipt.offset&&n.end===infoReceipt.end);
assert(infoNode,'Current DeviceInfo binding is missing');
assert.equal(body(infoReceipt.path).slice(infoNode.start,infoNode.end),infoReceipt.source);
assert.equal(infoNode.type,'ObjectExpression');
for(const field of ['productId','minDPI','maxDPI','dpiStep','OBMSlots']){
 const property=infoNode.properties.find(p=>key(p.key)===field);assert(property);receipts.push(audit.record('actual DeviceInfo '+field,{id:plan.device_info.module,node:property}));
}
assert.equal(audit.infoField('minDPI'),100);assert.equal(audit.infoField('maxDPI'),30000);assert.equal(audit.infoField('dpiStep'),50);assert.equal(audit.infoField('OBMSlots'),1);
assert.equal(audit.infoField('useCycleSensitivity'),null);
receipts.push(audit.record('DeviceInfo has no useCycleSensitivity override',{id:plan.device_info.module,node:infoNode}));
const product={product_id:182,source_class:plan.selected_class,get_command:previous.source_semantics.protocol.commands.getDPIStages,set_command:previous.source_semantics.protocol.commands.setDPIStages,
 active_profile:previous.source_semantics.protocol.class_id,editor_stage_counts:editorCounts,protocol_record_capacity:Math.floor((80-3)/7),
 min_dpi:audit.infoField('minDPI'),max_dpi:audit.infoField('maxDPI'),dpi_step:audit.infoField('dpiStep'),use_cycle_sensitivity:false,transport:cap};
const rustPath='crates/razer-device/src/mouse_dpi_stages.rs',rust=body(rustPath);
for(const token of ['pub fn pack(','pub fn read_current(','pub fn apply_current(','pub fn select_profile_slots','pub fn synchronize_profile_slots(',
 'outgoing[6] = packet_size','data.extend_from_slice(&row.x.to_be_bytes())','data.extend_from_slice(&row.y.to_be_bytes())','sent.index + 1','row.index != 0','same_table(&packed, &observed)'])assert(rust.includes(token),'Rust stage-table connection changed '+token);
assert(!rust.includes('DeviceWriteSetting::Dpi'),'Single-level selector0 is not stage-table evidence');
const connections=[
 {path:'crates/razer-service/src/runtime/portable.rs',tokens:['ServiceRequest::HidNodeDpiStagesRead','ServiceRequest::HidNodeDpiStagesWrite','mouse_dpi_stages::read_current(','mouse_dpi_stages::apply_current(','Self::revalidate(node)','device.report_lengths()?']},
 {path:'crates/razer-service/src/runtime/windows/device_reads.rs',tokens:['pub(super) fn dpi_stages(','target.peer_product_id.is_none()','receiver::ReceiverLock::acquire(','mouse_dpi_stages::read_current(','mouse_dpi_stages::apply_current(']},
 {path:'crates/razer-service/src/runtime/windows/native.rs',tokens:['ServiceRequest::DeviceDpiStagesRead','ServiceRequest::DeviceDpiStagesWrite','device_reads::dpi_stages(&target, None)','device_reads::dpi_stages(&target, Some(&draft))']},
 {path:'crates/razer-discovery/src/direct/mod.rs',tokens:['pub fn read_dpi_stages(','pub fn write_dpi_stages(','result.requested == draft','result.verified','mouse_dpi_stages::same_table(','mouse_dpi_stages::validate_current_reading(']},
 {path:'crates/razer-pages/src/features/sensitivity.rs',tokens:['SliderEvent::Change(_)','controls.preview[axis] = Some(value)','SliderEvent::Release(_)','owner.set_dpi_value(id, axis, value, window, cx)','owner.request_mouse_dpi(cx)','let submit = independent','state.stages[index][0] != state.stages[index][1]','owner.dpi_local_only_commit(cx)','IM.setStage dispatches only','slot.enabled']},
 {path:'crates/razer-pages/src/features/sensitivity.rs',tokens:['disabled: false','hidden rows retain editable numeric input','set_slot_axis_from_input']},
 {path:'crates/razer-model/src/settings.rs',tokens:['pub fn set_slot_axis_from_input','Source 182 numeric editor remains active']},
 {path:'crates/razer-pages/src/features/mouse_dpi_profile.rs',tokens:['pub fn begin_dpi_basic_reads','pub fn finish_dpi_basic_reads','!self.mouse_dpi.basic_reads_ready','self.mouse_dpi.reading.is_some()','self.mouse_dpi.revision == revision','self.dispatch_queued_mouse_dpi(cx)','values.dpi = Some((row.x, row.y))','valid_scope.store(false, Ordering::Release)','impl Drop for State']},
 {path:'crates/razer-pages/src/features/product_workspace.rs',tokens:['pub fn mouse_dpi_observation_matches','pub fn finish_mouse_dpi','WorkspaceEvent::MouseDpiStagesRequested','WorkspaceEvent::MouseDpiStagesReadRequested','pub fn finish_dpi_basic_reads']},
 {path:'crates/razer-pages/src/features/workspace.rs',tokens:['mouse_dpi: mouse_dpi_profile::State','self.sync_dpi_scope(cx)','MouseDpiStagesRequested','MouseDpiStagesReadRequested','self.mouse_polling.reset_profile()']},
 {path:'crates/razer-shell/src/shell/mouse_dpi_stages.rs',tokens:['direct.read_dpi_stages','direct.write_dpi_stages','mouse_dpi_observation_matches','this.discovery_revision == discovery_revision','valid_scope.load(std::sync::atomic::Ordering::Acquire)','ServiceRequest::Shutdown']},
 {path:'crates/razer-shell/src/shell/device_discovery.rs',tokens:['workspace.begin_dpi_basic_reads(cx)','workspace.finish_dpi_basic_reads(cx)','self.request_initial_dpi_stage_reads(cx)']},
 {path:'crates/razer-shell/src/shell.rs',tokens:['mod mouse_dpi_stages;','WorkspaceEvent::MouseDpiStagesRequested','WorkspaceEvent::MouseDpiStagesReadRequested','this.write_mouse_dpi_stages','this.read_mouse_dpi_stages']},
];
for(const connection of connections)for(const token of connection.tokens)assert(body(connection.path).includes(token),'DPI stage backend connection changed '+connection.path+': '+token);
const consumers=connections.map(({path})=>({path,sha256:hash(body(path))}));
const report={schema_version:1,product_id:182,source_receipts:receipts,transport_evidence:{path:'docs/re/mouse-read-capabilities-current-evidence.json',product_id:182,receipt_count:prior.receipts.length},
 semantics:{current_profile:1,set_command:product.set_command,get_command:product.get_command,stage_stride_bytes:7,table_header_bytes:3,editor_stage_counts:editorCounts,protocol_record_capacity:11,
 set_packet_size:'3+7*count overrides header[0]=80; report payload remains zero-padded to 80 bytes',get_packet_size:'wrapper defaults to -1; transport sends header[0]=80',
 packing:'Ke filters visible rows, remaps one-based local active position, uses index0/X/Y/Z0; disabled mode sends only the selected row',
 response:'preserve profile/active/count and every one-based index/X/Y/Z record in returned packet size, including index-zero padding',
 independent_flag:'local profile/UI only; never inferred from a device read or encoded as a table flag',
 current_write:'Xe always set profile1 -> get profile1 -> compare nonzero records -> update cache; no read-before skip',
 obm_slots:'Ye selects every actual metadata entry whose guid equals active profile, in original order; no manufactured numeric slot IDs',
 obm_write:'Mt getter -> compare record values plus active/count -> conditional setter; collect per-slot errors; no post-set readback'},
 implementation:{status:'current_182_direct_stage_table_ui_chain_implemented',rust:rustPath,capability_asset:'assets/data/mouse-dpi-stages-capabilities.json',product,backend_connections:connections.map(c=>c.path),consumer_fingerprints:consumers,
 ui_submission:{mount:'current TAB_PERFORMANCE nav -> lM/connect(OM) -> AM/connect(IM) -> Zm/jm -> Wm/Vm -> actual 4230 and 801 modules; all current receipts retained',
   slider:'Change updates retained preview/input only; Release normalizes/commits local full table and queues real profile1 set/get',
   number:'Enter and Escape blur, Blur normalizes once; focused wheel/spinner preview until Blur; unfocused spinner step submits, repeated each300ms',
   xy:'ON_STAGE_XY_CHANGE is profile-only; only relinking unequal X/Y additionally submits table with Y=X and selected edited stage',
   other:'stage-mode, enabled row and reorder submit full latest local table; stage selection itself only updates local/display active stage (Jm/setActiveDPI), no device command',
   serialization:'one retained DPI read/write, shared Idle/Polling write exclusion, latest DPI intent waits behind retained operation; basic query worker completion gates initial stage read',
   late_results:'profile/connection epochs, local input revision, route identity, discovery revision and request/response table verification; readback never replaces local hidden rows or flags',
   reads:'verified returned active stage X/Y updates actual DeviceReadValues.dpi observation and existing device status consumer; full table observation remains separate from local profile',
   cancellation:'scope change/restore or owner Drop invalidates atomic guard before resolve/submission; ordinary user preview advances revision and leaves dispatched operation intact'},
 application_validation:['Reject hidden/missing selection, invalid editor/range/count, truncated response and mismatched profile rather than fabricate values.',
 'Current confirmation requires matching active/count and complete returned record count in addition to source Xe record checks.',
 'OBM comparison requires complete record count as well as source Mt active/count and record comparison.',
 'Current getter requires internally consistent active/count, continuous one-based indices and source DPI ranges; invalid readings remain errors.',
 'Setter acknowledgement preserves full raw80 buffer and declared size; source tasks ignore setter jsonData, so it is not treated as a getter table.',
 'Identity/cancellation/deadline validation is supplied by caller around I/O; no fallback to single-level DPI.',
 'DPI step metadata is preserved; the protocol does not invent quantization or silently round input.'],
 result_contract:'current ack and confirmed readback are separate; OBM per-slot ack/error is separate and never labeled verified persistence'},
 gaps:['Original active-profile GUID version/save/cache broadcasts and native profile engine differ from existing local draft JSON save; current stage-table write does not prove complete profile persistence.',
 'Receiver/BLE DPI stage-table routing is unproved and explicitly rejected; direct commands are not sent to a receiver as if it were a mouse.',
 'Original version/cache broadcasts, task queue, observed OBM metadata acquisition and complete profile write engine are not implemented by this module.',
 'The original global task runner is not reproduced by the retained application slot: busy Idle/Polling intents are rejected while DPI read/write owns the slot; latest DPI intent is queued. This is application policy, not a claim of original queue equivalence.',
 'An atomic scope guard prevents submission before the IPC request. Cross-process cancellation after request dispatch does not reproduce the original task AbortSignal; an already accepted device write is not rolled back.',
 'Other product DPI stage-table capabilities are not inferred from 182. They require their own source feature/caller/transport proof.',
 'Runtime acceptance was not executed.'],runtime_acceptance:'not_run'};
for(const [output,data]of [['assets/data/mouse-dpi-stages-capabilities.json',{schema_version:1,products:[product]}],['docs/re/mouse-dpi-stages-current-evidence.json',report]]){
 const serialized=JSON.stringify(data,null,2)+'\n';
 if(process.argv.includes('--write'))fs.writeFileSync(path.join(root,output),serialized);
 else assert.equal(body(output).replace(/\r\n/g,'\n'),serialized,'Current DPI stage extraction changed '+output);
}
console.log('182 DPI stages: inline variable packet size, profile1, index remapping, actual OBM slots and source transport checked.');
