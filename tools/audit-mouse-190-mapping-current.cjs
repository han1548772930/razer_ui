// Parse current PID190 source text statically. Never evaluate vendor code.
const fs = require('fs');
const crypto = require('crypto');
const acorn = require('../.work/local-ui/analyzer/node_modules/acorn');
const root = 'local-ui-reverse/source/official/apps.razer.com/synapse/products/190/ui/static';
const output = 'docs/re/mouse-190-mapping-current-source.json';
const page = 'local-ui-reverse/pages/local-cache--synapse--products--190--ui--TAB_CUSTOMIZE--4794403.md';
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const files = [];
const slices = [];
function read(relative, expected) {
  const file = root + '/' + relative;
  const bytes = fs.readFileSync(file);
  const sha256 = hash(bytes);
  if (expected && sha256 !== expected) throw Error('Current source hash changed: ' + file);
  files.push({file, bytes: bytes.length, sha256});
  return {file, sha256, source: bytes.toString('utf8')};
}
function slice(input, name, start, end, extra = {}) {
  const source = input.source.slice(start, end);
  const record = {name, file: input.file, file_sha256: input.sha256,
    utf16_range: [start, end], utf8_byte_range: [Buffer.byteLength(input.source.slice(0,start)), Buffer.byteLength(input.source.slice(0,end))],
    slice_sha256: hash(source), ...extra, source};
  slices.push(record);
  return record;
}
function module(input, id) {
  const match = new RegExp('(?:^|[,\\{])' + id + ':').exec(input.source);
  if (!match) throw Error('Missing module ' + id);
  let node = acorn.parseExpressionAt(input.source, match.index + match[0].length, {ecmaVersion:'latest'});
  if (node.type === 'SequenceExpression') node = node.expressions[0];
  if (!/FunctionExpression|ArrowFunctionExpression/.test(node.type)) throw Error('Unexpected module ' + id + ' ' + node.type);
  return slice(input, 'Webpack module ' + id, node.start, node.end, {webpack_module:id});
}
function cls(input, name) {
  const start = input.source.indexOf('class ' + name + ' extends ');
  if (start < 0) throw Error('Missing class ' + name);
  const node = acorn.parseExpressionAt(input.source,start,{ecmaVersion:'latest'});
  return slice(input, 'Mounted class ' + name, node.start,node.end,{class_name:name});
}
const main = read('js/main.81b09779.js','4c6d60b23bebcc9423a561aa86ebf6ed24cd9cbe0938aa13d7fac46022a4dc2f');
const shared = read('js/316.f2f64e83.chunk.js','59baebb8b5d744a78bbdd922a32281ad36b7ed3116b731d1d7e9a92f9259f210');
const css = read('css/main.7ce428ff.css','108e1e1f5035ab0746450b6744c7a9bd00f888bdd20980cde797c0b8b3ce5023');
cls(main,'CO'); cls(main,'gO');
cls(main,'Bm'); cls(main,'Ir');
cls(main,'an'); cls(main,'d_'); cls(main,'GR');
cls(main,'Yt'); cls(main,'Ft');
for (const id of [4230,8193,9267,6114,9937,1050,4693,6482,7734,7278,3466]) module(main,id);
module(shared,5316); module(shared,5169);
const panel=read('js/ButtonPanelComponent.00b3692a.chunk.js');
acorn.parse(panel.source,{ecmaVersion:'latest'});
slice(panel,'Complete mounted ButtonPanelComponent',0,panel.source.length,{syntax_verified:true});
module(panel,5035);module(panel,4355);
const chunkNames = ['Default','Keyboard','Mouse','Sensitivity','Macro','SwitchProfile','InterDevice','Lighting','Hyper','Launch','Text','Media','Windows','Disable','AILauncher'];
for (const name of chunkNames) {
  const filename = fs.readdirSync(root+'/js').find(file => file.startsWith('Map'+name+'.'));
  if (!filename) throw Error('Missing current chunk Map'+name);
  const chunk = read('js/'+filename);
  acorn.parse(chunk.source,{ecmaVersion:'latest'});
  slice(chunk, 'Complete mapping function Map'+name, 0,chunk.source.length, {syntax_verified:true});
}
// Preserve full matched rules, including keyframe bodies' individual rules.
const cssRules = [];
for (const match of css.source.matchAll(/[^{}]+\{[^{}]*\}/g)) {
  const selector = match[0].slice(0,match[0].indexOf('{')).trim();
  if (/(?:^div$|main-container|customize|thx-btn|key-config|keymap-|key-mapping|keymapping|config-wrapper|config-block|config-drawer|drawer-|mapping-panel|button-panel|tooltip-panel-button|toggle-drawer|close-drawer|body-wrapper|backdrop|dropdown-area|s3-dropdown|s3-options|require-synapse|spinner|turbo|stepper|\.actions|action-wrapper|save-alert|save-mapping|number-input|drop-tips|\.tip(?:[ .:#]|$)|synapse-popup|custom-btn|custom-panel)/.test(selector)) {
    const rule = slice(css, 'CSS '+selector, match.index,match.index+match[0].length,{selector});
    cssRules.push(rule);
  }
}
// The help artwork differs only in line endings from the existing bundled icon.
const currentHelp=read('media/tooltip_questionmark.96138d2f.svg');
const helpAsset='assets/synapse/automation-tooltip_questionmark.svg';
const helpBytes=fs.readFileSync(helpAsset);
const sameNormalized=currentHelp.source.replace(/\r\n/g,'\n')===helpBytes.toString('utf8').replace(/\r\n/g,'\n');
if (!sameNormalized) throw Error('Require Synapse help icon no longer matches');
const gaps=[
  'Native setMappingList to selected profile/service transport, response, refresh, failure, cancellation and cleanup is absent; Save persists the local draft only.',
  'Macro, profile, Chroma, other-device and AI producer observations and full function bodies remain absent; unknown observations are distinct from source canSave=false.',
  'AI installation-dependent menu filtering remains unconnected.',
  'Drawer compact numeric counters are implemented for all eight PID190 rows; secondary mapping tips and generic nonnumeric counter shrink/split logic remain absent (not used by the current PID190 row descriptors).',
  'Shared dropdown option overflow tips, exact document-click gesture parity (current GPUI outside release adapter), same-frame option-width relayout and nested source stacking-context acceptance remain incomplete; the measured180px options, source open-direction, outside release and window-blur dismissal now have Rust implementations.',
  'Mapping document-click exact whitelist adapter, concrete nav/navs-wrapper caller and history arrow class adapter are connected; outer body bounds are captured. History adapter runs after step_page_history and pending-guards duplicate close. Profile-dropdown trigger adapter uses the SynapseSelect trigger-click callback because Kit Popover stops mouse-down propagation. Profile options are source-exempt raw-text option ancestors. Configure Stages and icon-refresh special continuations remain missing.',
  'Save alert viewport400px border-box, translateY(-100%) position and100ms linear backdrop fade are implemented. Original nested stacking-context parity and runtime acceptance remain incomplete; local save guards reject invalid/unavailable drafts with typed MouseMappingSaveRejected and retain pending/draft instead of inventing native success.',
  'Text exact multiline/emoji/character-map native action remains incomplete.',
  'Sensitivity numeric/grid/sliders and Configure Stages navigation remain incomplete.',
  'Current outer tab/history unsaved confirmation is connected; history class adapter is ordered after step_page_history and is guarded against pending duplicate close. Profile trigger bridge uses the SynapseSelect trigger-click callback because Kit Popover stops mouse-down propagation; profile options remain source-exempt raw-text option ancestors. Broader native profile producers/submission remain absent.',
  'Require Synapse update/default state for unimplemented dependency functions remains incomplete.',
  'Help background hover timing differs from source 300ms easing.',
  'Application runtime acceptance and real device operations have not run and remain prohibited.'
];
const receipt={
  product_id:190, date:'2026-10-11', scope:'Full mounted current Customize mapping drawer source evidence; implementation remains partial',
  acquisition:{source:'Existing current official static product acquisition', files, vendor_code_executed:false, source_revision:'Current hash-verified PID190 files; no obsolete reference directories used'},
  reverse_engineering:{method:'Acorn parseExpressionAt exact module/class boundaries and syntax parsing of every mounted Map function chunk; exact CSS rule text',
    save_alert:{caller:'gO.renderSaveAlert -> always mounted backdrop and save-alert; Yt main-container absolute viewport containing block',
      geometry:{width:400,padding:[20,30],border:1,border_radius:5,x:'viewport.width/2 - measured.border_box.width/2',y:'viewport.height/2 - measured.border_box.height',backdrop:'main-container absolute left0/top0 width100% height100%!important'},
      motion:{backdrop_opacity_ms:100,easing:'linear',visibility_close:'hidden immediately with0s transition',button_opacity_ms:300,button_easing:'CSS default ease',hover_opacity:0.8,active_opacity:0.6},
      controls:{save_source_disabled_condition:false,close:'dismissSave(true) -> saveMapRef(false,true), preserves outer dirty observation',dont_save:'saveMapRef(false) then dontSaveAction',save:'saveMapRef(true) then nextAction',escape_handler:false,focus_return:false,backdrop_dismiss:false,discard_long_label:'localized e.length>9 UTF16 code units -> widthauto,px16',title:'green RazerF5 uppercase16/19',close_geometry:'20px at right8/top8',action:'27px min-width100, margin-right10, container margin-right-10/mt20'},
      close_resource:{source:root+'/media/icon_close.55fe41f1.svg',asset:'assets/synapse/mapping-close.svg',source_sha256:hash(fs.readFileSync(root+'/media/icon_close.55fe41f1.svg')),asset_sha256:hash(fs.readFileSync('assets/synapse/mapping-close.svg'))},
      local_failure:'Existing guards retain draft/pending and emit MouseMappingSaveRejected; native service submission/ack absent'},
    source_state_audit:{mapping_escape_close:false,mapping_focus_return:false,dropdown_blur:'7734 installs window blur; onWindowBlur closes open dropdown; cleanup removes handler',
      menu_height_state:{fields:['keyMappingStyle','isShowScroll'],occurrences_each:3,sites:['openFunctionsView','collapseFunctionsView','constructor state'],mounted_render_reads:0,visible_height_cap_applied:false},
      outer_click_targets:{nav:{caller:'d_ render @4315027 -> an navs-wrapper',connected:true},history:{caller:'GR render @4678183 arrow back/forward -> navigation',context_adapter_connected:true,ordering:'shell step_page_history first; dirty sets pending and mapping adapter returns; clean clears mapping before adapter no-op'},profile_option:{caller:'Ir ->7734 ->7278 raw-text option',mapping_close_exempt:true},profile_trigger:{caller:'7734 s3-dropdown ->dropdown-area; GPUI source profile uses SynapseSelect inner #input',context_adapter_connected:true,callback:'SynapseSelect trigger-click callback invokes mapping_190_document_click on the actual trigger click; keyboard open actions and detached raw-text.option are excluded',source_reason:'Kit Popover::render stops mouse-down propagation before toggle_open'}}},
    offset_units:'Zero-based UTF-16 JavaScript string code units and UTF-8 byte ranges; end exclusive', slices,
    function_chunks:chunkNames, css_rule_count:cssRules.length,
    mounted_drawer_conditions:[
      {caller:'gO render',condition:'Non-keypad branch mounts chunk652 module5035; PID190 is non-keypad',rust:'customize_190 mounts the panel in the same page'},
      {caller:'5035 updateButtons',condition:'keyProperty absent and standard layer copies all buttons; HyperShift excludes HID+disableHypershiftMapping; filter1 includes only isMapped; numeric counters sort ascending',rust:'static PID190 descriptor filtering and local mapping observation, numeric counter sort'},
      {caller:'5035 render',condition:'disabled or isSidePanelList=false rows are omitted; isKeyToggle rows follow standard/HyperShift polarity',rust:'same source descriptor predicates; source-disabled rows remain rendered with disabled styling when isEnabled=false'},
      {caller:'5035/CO clickBtn',condition:'Same active button is ignored; dirty changes save first; drawer calls updateActiveButton(key,true), image calls origin=false',rust:'Next::PanelOpen vs Next::Open, same-input early return and existing confirmation continuation'},
      {caller:'5035 updateToggleButtonPanel',condition:'Closing panel-origin mapping clears active button/mapping; image-origin mapping is retained and origin reset',rust:'Next::Panel closes panel-origin editor and reloads image-origin draft'},
      {caller:'5316 calculateKeyConfigPopupLeft',condition:'Drawer-origin popup left is drawer width; otherwise label right if room (viewport/client-width minus20), left if no room, clamp negative and drawer overlap',rust:'PopupLayer same-frame prepaint measured anchor, viewport edge flip, left clamp and230 overlap floor'},
      {caller:'5316 handleKeyConfigPopupPosition',condition:'Top-view popup body36-scrollTop; panel-origin36; window requestAnimationFrame updates measurements',rust:'PopupLayer consumes same-frame prepaint rectangles and scroll offset; header lives at bodytop-36'},
      {caller:'current CSS',condition:'Body height clamp310..570 of viewport-140; menu max-width transition200ms; sidepanel/body transition200ms',rust:'matching height clamp and gpui-kit motion Ease transition'},
      {caller:'Bm changeView/navigateBackward/navigateForward',condition:'isMappingChanged triggers setDisplaySaveAlert continuation before outer navigation',rust:'source_workspace defers page/history mutation through MouseMappingNavigation event'},
      {caller:'4355/5035 binding tooltip',condition:'Only mapped binding whose measured text exceeds client width calls hover; pass row rectangle; top=row.top+50; if top+height exceeds innerHeight subtract height+50; x61,maxwidth358,no fade',rust:'Per-row Rc rectangle and actual text-client width; deferred BindingTooltip at priority999 with measured natural height and bottom flip'},
      {caller:'7734 shared Dropdown',condition:'Opening computes min(180,25*n+2), direction by trigger.bottom+2+expectedHeight>innerHeight; down top27+margin1, up bottom28; scroll selected option; window click outside trigger/options or window blur closes',rust:'Shared DropdownLayer for panel/function/keygroup;25px source rows,180px max-height200ms ease, measured anchors and source direction, selected scroll on opening, outside release and activation observation'},
      {caller:'5316 openFunctionsView/collapse/render',condition:'t=key-mapping.height,e=body-wrapper.height; t&&e&&t+46>e stores keyMappingStyle height=e-46 and isShowScroll=true; all occurrences are handler/initializer only; mounted render never reads them; CSS hover enables overflow:auto',rust:'Measure menu/outer-body, retain calculated menu_height as source state only; no invented visible height cap; collapsed clips, hover scrolls; label remains mounted through200ms width change'},
      {caller:'5316 windowClick',condition:'Ignore inside keyMapBody; ancestor class string containing raw-text and option ignored; exact15-item whitelist tests only target/direct parent; no PID190 TwoTap branch; source close(false) prompts dirty mapping',rust:'Typed mapping_190_document_click source-class adapter, preserves existing pending navigation continuation; no blanket ancestor exemption'},
      {caller:'5316 +3466 +4230 mounted key/focus handlers',condition:'5316 mapping has no Escape/key-close handler or focus restoration;3466 prevents Shift+Escape browser shortcut only;4230 numeric input ArrowUp/Down adjusts and Enter/Escape blurs',rust:'No added Escape-close or focus-return; existing Turbo Escape blur preserved; dropdown blur separately follows7734'},
      {caller:'gO renderSaveAlert, Yt, current CSS',condition:'Backdrop stays mounted hidden: opacity100ms linear, visibility0s; main-container absolute100% viewport; save-alert fixedleft50% top50% transform(-50%,-100%),width400,padding20 30; Save has no disabled predicate; no backdrop/Escape/focus handler',rust:'Always-called persistent opacity channel; visible full-viewport backdrop and measured ConfirmationLayer x=(viewport-width)/2,y=viewport.height/2-height; source enabledSave controls/styles; guard rejection emits typed error and preserves draft/pending'},
      {caller:'Ir changeProfile',condition:'Different GUID invokes changeSelectedProfile directly; no isMappingChanged guard',rust:'existing profile restore preserved without invented confirmation'}
    ],
    drawer_resources:[['icon_sidepanel.e53fef93.svg','drawer.svg'],['icon_sidepanel_a.90d67a6e.svg','drawer-active.svg'],['icon_closepanel.86903958.svg','drawer-close.svg']].map(([source,asset])=>{
      const sourceFile=root+'/media/'+source,assetFile='assets/synapse/'+asset;
      const original=fs.readFileSync(sourceFile),output=fs.readFileSync(assetFile);
      if(!original.equals(output))throw Error('Drawer artwork differs: '+asset);
      return {source:sourceFile,asset:assetFile,source_sha256:hash(original),asset_sha256:hash(output),exact_bytes:true};
    }),
    require_synapse:{caller:'316 module5316 renderFunction/getRequireSynapse 鈫?module5169; function checkRequiredSynapse or enableSave',
      mouse:'ScrollLeft, ScrollRight, or DoubleClick with Turbo; PID190 lacks override required lists and uses source OBM fallback',
      media:'MicVolumeUp, MicVolumeDown, MuteMic, MuteAll', sensitivity:'DPI_OnTheFly', always_for_implemented_functions:['WINDOWS_SHORTCUT','TEXT_FUNCTION','LAUNCH_PROGRAM'],
      resources:['docs/re/mouse-190-requires-synapse-logo-current-source.json','docs/re/mouse-190-ai-icons-current-source.json'],
      help_resource:{source:currentHelp.file, source_sha256:currentHelp.sha256, asset:helpAsset, asset_sha256:hash(helpBytes), normalized_lf_text_equal:sameNormalized}},
    turbo:{component:'main module4230',range:[1,20], default:7, maxLength:2, repeat_ms:300,
      live:'The original live string is retained; empty and minus-only are accepted during editing.',
      blur:'parseInt; NaN becomes zero; clamp 1..20; canonical integer replaces the typed value.',
      increment:'Source value+1 preserves JavaScript concatenation for a typed string (7 as string increments to 71 then clamps to 20).',
      termination:'Mouse release/leave, min/max updates, function/control disposal clear the repeat task.'}},
  rust_implementation:{files:['crates/razer-pages/src/features/mouse_customize_190.rs','crates/razer-pages/src/features/mouse_mapping_190.rs','crates/razer-pages/src/features/mouse_products.rs','crates/razer-pages/src/features/source_workspace.rs','crates/razer-pages/src/features/product_workspace.rs'],
    implemented:['Source menu and hover expansion','14 mouse options','144 keyboard rows and 21 shifted-symbol resolutions','Keyboard recording/modifiers','Local function draft editing','Primary click preservation','HyperShift mirroring and reset behavior','Save/Don鈥檛 Save/continue-editing confirmation','Turbo live string, blur normalization, keys/wheel/hold-repeat','Require Synapse predicate/card/help and exact source logo','Mounted ButtonPanel numeric counter rows, All/Customized dropdown, descriptor filters and primary-click disabled styling','230px sidepanel/body transition and200ms function-menu expansion','Same-frame measured popup anchor flipping, drawer overlap clamp, responsive310..570 body and scroll offset','Outer tab/history pending Save/Don鈥檛 Save/continue-editing continuation'],
    newly_implemented:['Mapped binding actual overflow tooltip and row bottom flip','Shared source panel/function/keyboard-group dropdown overlay and window-blur dismissal','Measured function-menu dead-state condition and CSS hover overflow','Exact5316 document-target whitelist adapter and concrete nav bridge; no invented mapping Escape/focus action','Save alert measured400px border-box at viewport50% minus full height; full backdrop100ms linear fade and immediate hide','Source Save alert RazerF5title/close/action dimensions and300ms button opacity; no invented disabled Save or backdrop-dismiss','Typed local save rejection preserving pending confirmation and draft'],
    missing_observation:'State.missing_observation records absent reducer/producer observations separately from State.can_save', gaps},
  ui_backend_connection:{save:'Local product draft.mappings plus MouseProductChanged; native submission absent', native_submission:false, observed_device_success:false},
  runtime_acceptance:{application_executed:false,vendor_javascript_executed:false,dlls_executed:false,real_device_operations:false,status:'Not performed; pure Rust compilation handled separately by parent'}
};
fs.writeFileSync(output,JSON.stringify(receipt,null,2)+'\n');
if (process.argv.includes('--receipt-only')) {
  console.log(JSON.stringify({output,source_files:files.length,source_slices:slices.length,css_rules:cssRules.length}));
  process.exit(0);
}
const marker='## 2026-10-11 current mounted mapping drawer audit (190)';
let markdown=fs.readFileSync(page,'utf8');
if (markdown.includes(marker)) markdown=markdown.slice(0,markdown.indexOf(marker)).trimEnd()+'\n';
const blocks=[
  slices.find(s=>s.class_name==='CO'),slices.find(s=>s.class_name==='gO'),
  slices.find(s=>s.class_name==='Bm'),slices.find(s=>s.class_name==='Ir'),
  slices.find(s=>s.class_name==='an'),slices.find(s=>s.class_name==='d_'),slices.find(s=>s.class_name==='GR'),
  slices.find(s=>s.class_name==='Yt'),slices.find(s=>s.class_name==='Ft'),
  slices.find(s=>s.file===main.file&&s.webpack_module===7734),
  slices.find(s=>s.file===main.file&&s.webpack_module===7278),
  slices.find(s=>s.file===main.file&&s.webpack_module===3466),
  slices.find(s=>s.name==='Complete mounted ButtonPanelComponent'),
  slices.find(s=>s.file===shared.file&&s.webpack_module===5316),
  slices.find(s=>s.file===shared.file&&s.webpack_module===5169),
  slices.find(s=>s.file===main.file&&s.webpack_module===4230),
  slices.find(s=>s.name==='Complete mapping function MapMouse'),
  slices.find(s=>s.name==='Complete mapping function MapKeyboard')
];
const text=[marker,'',
  'This section supersedes the 2026-10-10 correction above. The seven-option claim was incomplete: the current base mouse array includes ten options and PID190 extends it by four repeated-scroll options, for fourteen. The connector hover/cycle state, function menu and mapping-save confirmation now have Rust implementations; they are not still wholly absent. The page and native chain remain incomplete for the explicit gaps below.','',
  'Acquisition, reverse-engineered behavior, Rust implementation, UI/backend connection and runtime acceptance are recorded separately in [`mouse-190-mapping-current-source.json`](../../docs/re/mouse-190-mapping-current-source.json). That durable receipt includes full current source files/hashes, exact class/module boundaries, all fifteen mapping function chunks and matched CSS rules. It replaces the temporary `.work/mapping190-current/evidence.json` as the maintained evidence. No vendor JavaScript was evaluated.','',
  'Mounted chain: current main `CO` renders the eight button label/connector callers and HyperShift action; `gO` mounts the shared KeyMappingComponent and owns pending-mapping Save/Don鈥檛 Save confirmation. Current `316` module5316 mounts the actual function menu and selected Map function with current reducers/callbacks; module5169 supplies the Require Synapse card. These are mounted body/caller implementations, not route or descriptor evidence.','',
  'The Rust branch opens the corresponding local mapping draft from the button label, captures keyboard keys/modifiers, handles fourteen mouse options, edits supported functions, preserves primary click, mirrors the HyperShift mapping, and prompts on switching input/layer or closing with unsaved edits. Cancel discards directly. Save changes local `draft.mappings` and emits `MouseProductChanged`; it does not submit to the native service or report a device write.','',
  'Turbo now follows module4230: live signed integer strings/empty/minus-only edits remain strings; Blur or Enter parses, defaults invalid input to zero, clamps to 1鈥?0 and restores a canonical number. Spinner arrows act immediately and repeat every 300ms while held. The source string-increment behavior (`"7" + 1` 鈫?`"71"` 鈫?20) is preserved. Numeric limit updates, release/leave and disposal stop repetition.','',
  'Require Synapse follows the selected-function caller predicate: Mouse ScrollLeft/ScrollRight or turbo DoubleClick; microphone volume/mute/all-mute multimedia functions; On-the-fly Sensitivity; and implemented Windows/Text/Launch functions. PID190 DeviceInfo does not export required-list overrides, so Mouse uses the actual OBM fallback rather than the unused similarly named getter. The card uses the exact current source logo, source geometry/text/help, and the existing tooltip portal. Current questionmark SVG and bundled automation questionmark match after LF normalization; their original byte hashes are retained in the receipt.','',
  'Source `canSave=false` and missing producer observations are separate states. Macro/profile/Chroma/interdevice/AI observations have not been fabricated, and the UI does not expose engineering gap prose as source content. Their missing producers and function bodies remain required work.','',
  'The next current-source checkpoint mounts the actual ButtonPanel (current chunk652 modules5035/4355): All Buttons/Customized dropdown, numeric counter order1鈥?, per-layer descriptor filters, disabled primary-click row, mapping-type/binding display, panel/image origin and source pending-save continuation. The sidepanel/body slide and function menu width use the current200ms ease transition. Popup left position reads real same-frame button rectangles, flips at the viewport edge and clamps drawer overlap; the body height follows310鈥?70px bounded viewport-minus140px. Mapped binding tooltips now use actual text client-width and each hovered row rectangle, x61/top=row+50/max-width358 and measured bottom flip without fade. Shared source dropdowns now use25px rows,180px animated max-height, trigger-based up/down placement, selected scrolling on opening, source selected text color and outside/window-blur dismissal. Dropdown option tips and source stacking-context/runtime acceptance remain explicit gaps.','',
  'The exact function menu handler compares measured menu and outer body heights: if both exist and menu+46>body it stores body-46. The current mounted render has no reads of keyMappingStyle or isShowScroll (each has exactly three occurrences: open, collapse and initialization). Rust retains this source state calculation without inventing a visible height cap or scrollable class. Current CSS alone changes overflow:hidden to overflow:auto on hover. Function labels remain mounted while the200ms width changes.','',
  'Current5316 document click ignores inside mapping and ancestor class strings containing both raw-text and option; it tests its exact fifteen whitelist names only on target/direct parent. Rust exposes the corresponding typed source-class adapter and retains pending outer navigation. There is no mapping Escape-close or focus-return handler in the mounted source: module3466 prevents Shift+Escape browser behavior, and Turbo4230 Enter/Escape blurs only its numeric input. Shared Dropdown7734 explicitly installs/removes a window blur handler and closes an open dropdown on that event.','',
  'The outer body rectangle is connected through source-product-content on_prepaint to the shared bounds handle. The concrete native nav caller supplies source d_ class nav and an direct-parent navs-wrapper to the exact document-click adapter after navigation handling. GR toolbar arrow back/forward classes use the navigation parent adapter after step_page_history with a pending guard. Profile Dropdown7734 uses the actual SynapseSelect trigger click adapter. Profile option7278 has a combined raw-text option ancestor and is source-exempt, so selecting a profile does not invent a mapping dirty confirmation. Configure Stages and icon-refresh special source callback paths remain explicit gaps.','',
  'Current gO.renderSaveAlert always mounts a backdrop and save-alert. The current Yt main-container is absolute width/height100%; the backdrop uses its full viewport containing block, while the dialog is fixed at viewport50% with translateX(-50%) translateY(-100%). Rust measures the400px border-box dialog and positions x=(viewport-width)/2,y=viewport.height/2-fullheight, with source padding20/30,border1,radius5. The persistent opacity channel starts while hidden and opens over100ms linear; close immediately hides as the source visibility0s rule requires. Current title is RazerF5 green16/19 uppercase; close20px at8px; paragraphs keep the source double-br blank line; actions match27px/min100/margin10 and localized UTF16length>9 discard padding16. Button hover0.8/active0.6 now use300ms CSS-default ease. Save alert Save has no source disabled predicate and is shown enabled; the existing local save validation remains, with typed MouseMappingSaveRejected on failure, preserving pending/draft. Native submission and acceptance remain absent. No backdrop-dismiss, Escape-close or focus-return was added.','',
  'Current outer `Bm.changeView` and back/forward navigation test `isMappingChanged` before applying navigation. Rust now defers outer page/history mutations and continues through `MouseMappingNavigation::Page/History` only after Save or Don鈥檛 Save; cancel leaves the current page/history unchanged. Current `Ir.changeProfile` calls `changeSelectedProfile` directly for a changed GUID and does not test dirty mapping state; no extra profile confirmation was invented.','',
  'Current mounted condition checklist:','',
  '| Caller | Current source condition | Rust connection |',
  '| --- | --- | --- |',
  ...receipt.reverse_engineering.mounted_drawer_conditions.map(row=>'| '+[row.caller,row.condition,row.rust].map(value=>value.replace(/\|/g,'\\|')).join(' | ')+' |'),' ',
  'Remaining gaps:','',...gaps.map(gap=>'- '+gap),'',
  'The following complete mounted code excerpts are retained directly in this page. Offsets are UTF-16 code units, end exclusive; each excerpt has both its full-file and slice SHA-256. The linked JSON additionally retains all other mapped function chunks, relevant shared modules and full matched CSS.',''
];
for(const block of blocks) text.push(`### ${block.name}`,'',`Source: \`${block.file}\`; UTF-16 [${block.utf16_range.join(', ')}). Full-file SHA-256: \`${block.file_sha256}\`. Slice SHA-256: \`${block.slice_sha256}\`.`,``,`\`\`\`javascript`,block.source,'```','');
text.push('### Current drawer/menu/Require Synapse/stepper CSS','',`Source: \`${css.file}\`; full-file SHA-256: \`${css.sha256}\`. Every rule range/hash is retained in the JSON.`, '', '```css',...cssRules.map(rule=>rule.source),'```','');
fs.writeFileSync(page,markdown.trimEnd()+'\n\n'+text.join('\n'));
console.log(JSON.stringify({output,page,source_files:files.length,source_slices:slices.length,css_rules:cssRules.length,bytes:fs.statSync(output).size,gaps:gaps.length}));
