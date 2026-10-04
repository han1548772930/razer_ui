// Independently recheck current mounted transfer receipts as syntax and CSS.
// --check is strictly read-only. Never imports or executes reference scripts.
const fs = require('node:fs'), path = require('node:path'), acorn = require('acorn');
const {walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const fact = (condition, message) => { if (!condition) throw Error(message); };
const expectedProducts = [164, 241, 778, 784, 3871, 3884, 3886, 3946];
const upstream = JSON.parse(read('docs/re/source-profile-menu-current-evidence.json'));
const data = JSON.parse(read('src/features/source_profile_menu_data.json'));
const texts = new Map();
function currentText(file) {
  fact(/^\.ref\/devices\/\d+\/static\/(js|css)\//.test(file), `Unexpected reference path: ${file}`);
  if (!texts.has(file)) texts.set(file, read(file));
  return texts.get(file);
}
function checkedReceipt(receipt) {
  const source = currentText(receipt.path);
  fact(source.slice(receipt.offset, receipt.end) === receipt.source, `Stale current source receipt: ${receipt.path}:${receipt.offset}`);
  return {...receipt, sha256: hash(source)};
}
function node(receipt) {
  checkedReceipt(receipt);
  return acorn.parse(`(${receipt.source})`, {ecmaVersion: 'latest'});
}
function prop(node, name) {
  return node?.type === 'ObjectExpression' ? node.properties.find(p => key(p.key) === name)?.value : undefined;
}
function one(ast, predicate, label) {
  const matches = [];
  walk(ast, node => { if (predicate(node)) matches.push(node); });
  fact(matches.length === 1, `Expected one ${label}; got ${matches.length}`);
  return matches[0];
}
function classProperty(node, name) {
  const value = prop(node, 'className');
  return value?.type === 'MemberExpression' && key(value.property) === name;
}
function cssValue(rule, name) { return rule?.properties.findLast(p => p.property === name)?.value; }
const products = expectedProducts.map(pid => {
  const sourceProduct = upstream.products.find(p => p.product_id === pid);
  const geometry = data.products.find(p => p.product_id === pid)?.import_export;
  fact(sourceProduct?.import_export && geometry, `Missing current ${pid} transfer`);
  const transfer = sourceProduct.import_export, component = node(transfer.component);
  const selected = prop(one(component, n => prop(n, 'isProfileSelected'), `${pid} initial profile selection`), 'isProfileSelected');
  fact(pid === 3886
    ? selected.type === 'UnaryExpression' && selected.operator === '!' && selected.argument.value === 0
    : selected.type === 'BinaryExpression' && selected.operator === '===' && key(selected.left.property) === 'selectedProfileGuid' && key(selected.right.property) === 'guid',
    `${pid} initial export selection changed`);
  fact(/deselectAll:!1/.test(transfer.component.source), `${pid} initial select-all action changed`);
  const browse = transfer.children.find(c => c.component.source.includes('uploadFile'));
  const cloud = transfer.children.find(c => c.strings.some(s => s.value === 'COMPATIBLE_CLOUD_PROFILES'));
  const profile = transfer.children.find(c => c.component.source.includes('isProfileSelected') || c.component.source.includes('this.props.isSelected'));
  fact(browse && cloud && profile, `Missing ${pid} mounted transfer child`);
  const browseAst = node(browse.component), cloudAst = node(cloud.component);
  const fileInput = one(browseAst, n => prop(n, 'type')?.value === 'file', `${pid} file input`);
  const fileIntro = one(browseAst, n => classProperty(n, 'fileIntro'), `${pid} browse row`);
  const dropdown = one(cloudAst, n => prop(n, 'dataSet') && prop(n, 'disable'), `${pid} cloud dropdown`);
  const disabled = prop(dropdown, 'disable');
  fact(disabled.type === 'UnaryExpression' && disabled.operator === '!', `${pid} cloud disabled value is no longer static`);
  const cloudDisabled = !disabled.argument.value;
  const fileAccept = prop(fileInput, 'accept')?.value ?? null;
  const wholeRowClick = !!prop(fileIntro, 'onClick');
  fact(cloudDisabled === (pid !== 3886), `${pid} Cloud availability changed`);
  fact(fileAccept === (pid === 3886 ? null : '.synapse4'), `${pid} file accept changed`);
  fact(wholeRowClick === (pid !== 3886), `${pid} browse target changed`);
  if (!wholeRowClick) one(fileIntro, n => prop(n, 'src') && prop(n, 'onClick'), `${pid} folder click target`);
  const backdrop = one(component, n => classProperty(n, 'backDrop'), `${pid} backdrop`);
  fact(!prop(backdrop, 'onClick'), `${pid} now dismisses on outside click`);
  fact(!/onKeyDown|keydown|Escape/.test(transfer.component.source), `${pid} dialog keyboard handling changed`);
  const files = [...new Set(sourceProduct.css.rules.filter(r => /ImportExportModal_|cloud-switch|import-profile-btn-group/.test(r.selector)).map(r => r.source_path))];
  fact(files.length > 0, `${pid} CSS missing`);
  const css = files.flatMap(file => parseCSS(currentText(file))
    .filter(rule => /ImportExportModal_|cloud-switch|import-profile-btn-group|^\.thx-btn/.test(rule.selector))
    .map(rule => ({source_path: file, ...rule})));
  const modal = css.find(rule => /^\.ImportExportModal_modal__\S+$/.test(rule.selector));
  fact(modal, `${pid} modal rule missing`);
  for (const [name, value] of Object.entries({width:'602px',height:'481px',top:'104px',left:'calc(50% - 300px)',
    'background-color':'#111',border:'1px solid #515151','border-radius':'5px'})) {
    fact(cssValue(modal, name) === value, `${pid} modal ${name} changed`);
  }
  for (const [name, value] of Object.entries({width:602,height:481,top:104,left:'calc(50% - 300px)'})) {
    fact(geometry[name] === value, `${pid} native geometry ${name} differs from source`);
  }
  const header = css.find(r => /^\.ImportExportModal_modal__\S+ \.ImportExportModal_header__\S+$/.test(r.selector));
  const browseRule = css.find(r => /^\.ImportExportModal_modal__\S+ \.ImportExportModal_browse__\S+$/.test(r.selector));
  fact(cssValue(header, 'height') === '36px' && cssValue(browseRule, 'height') === '47px', `${pid} header/browse dimensions changed`);
  fact(cssValue(css.find(r => r.selector === '.cloud-switch'), 'height') === '49px', `${pid} Local/Cloud height changed`);
  const manifestPath = `.ref/devices/${pid}/asset-manifest.json`, manifest = JSON.parse(read(manifestPath));
  for (const file of [transfer.component.path, ...files]) fact(Object.values(manifest.files).some(value => file.endsWith(value.replace(/^\.\//,''))), `${file} absent from current manifest`);
  const strings = [...new Set([transfer, ...transfer.children].flatMap(c => c.strings.map(s => s.value)))];
  for (const locale of ['en', 'zh-CN']) {
    const localeData = JSON.parse(read(`locales/${locale}.json`));
    for (const label of geometry.locale_keys.filter(value => !['ACCESSORY','MOUSE','MAINBOARD','IOT','STRIP','ARGB_CONTROLLER'].includes(value))) {
      fact(Object.hasOwn(localeData, label), `Missing existing ${locale} transfer label: ${label}`);
    }
  }
  return {product_id:pid, manifest:{path:manifestPath,sha256:hash(read(manifestPath))},
    call:checkedReceipt(transfer.call), component:checkedReceipt(transfer.component),
    children:[profile,cloud,browse].map(c=>({call:checkedReceipt(c.call),component:checkedReceipt(c.component)})),
    geometry, source_behavior:{cloud_disabled:cloudDisabled,file_accept:fileAccept,browse_click_target:wholeRowClick?'whole filename row':'folder image',
      backdrop_dismiss:false,keyboard_dismiss:false,initial_profile_selection:pid===3886?'all':'active',select_all_initial_action:'deselect'}, strings, css};
});
const currentFile = products.find(p=>p.product_id===3946).component.path, current = currentText(currentFile);
const assetDeclarations = [
  {offset:6351138, symbol:'Lc', filename:'icon_close.55fe41f1.svg', target:'assets/synapse/profiles-close.svg'},
  {offset:6320671, symbol:'X_', filename:'icon_folder_grey.220b24b0.svg', target:'assets/synapse/source-transfer-folder.svg'},
];
const assets = assetDeclarations.map(asset => {
  let expression = acorn.parseExpressionAt(current, asset.offset, {ecmaVersion:'latest'});
  if (expression.type === 'SequenceExpression') expression = expression.expressions[0];
  fact(expression.type === 'AssignmentExpression' && expression.left.name === asset.symbol, `Asset binding changed: ${asset.symbol}`);
  fact(expression.right.type === 'BinaryExpression' && expression.right.right.value === `static/media/${asset.filename}`, `Actual image path changed: ${asset.symbol}`);
  return {...asset, reference:`.ref/devices/3946/static/media/${asset.filename}`, declaration:{path:currentFile,offset:expression.start,end:expression.end,source:current.slice(expression.start,expression.end)}};
});
const coneRule = products.find(p=>p.product_id===3946).css.find(r=>/^\.ImportExportModal_cone-icon__/.test(r.selector));
fact(cssValue(coneRule,'background-image') === 'url(../../static/media/icon_cone.7bf8041f.svg)', 'Actual cloud cone changed');
assets.push({filename:'icon_cone.7bf8041f.svg',target:'assets/synapse/source-transfer-cone.svg',reference:'.ref/devices/3946/static/media/icon_cone.7bf8041f.svg',declaration:coneRule});
for (const asset of assets) {
  const source = fs.readFileSync(path.join(root, asset.reference));
  if (process.argv.includes('--check') || asset.target.endsWith('/profiles-close.svg')) {
    fact(source.equals(fs.readFileSync(path.join(root, asset.target))), `Native image differs from current source: ${asset.target}`);
  }
  asset.sha256 = hash(source);
}
const nativePath='src/features/source_workspace/profile_transfer.rs', native=read(nativePath), compact=native.replace(/\s+/g,'');
for (const fragment of ['profiles.into_iter()', 'selected: pid == 3886 || profile.id == active', 'select_all: false',
  'row.selected = this.select_all', 'this.select_all = !this.select_all', 'row.selected = *checked',
  'cx.prompt_for_paths(PathPromptOptions', 'multiple: false', 'this.pid == 3886 ||', 'extension.eq_ignore_ascii_case("synapse4")',
  'Ok(Ok(None)) => {}', 'this.path = Some(path.clone())', 'this.picker_generation == generation',
  'self.pid == 3886 && !self.cloud', 'BaseButton::new("source-transfer-browse-icon")',
  '"DISABLED_FEATURE_DESC"', '.disabled(primary)', '.on_cancel(|_, _, _| false)', '.on_ok(|_, _, _| false)',
  '.close_on_backdrop_press(false)', 'DialogPopup::new()', 'cx.emit(SourceProfileTransferClosed)',
  '.w(surface::css(geometry.width))', '.h(surface::css(geometry.height))', '.top(surface::css(geometry.top))',
  'window.viewport_size().width / 2. - surface::css(300.).to_pixels(window.rem_size())']) {
  fact(compact.includes(fragment.replace(/\s+/g,'')), `Missing native transfer contract: ${fragment}`);
}
fact(!native.includes('razer-ui-profile') && !/std::fs|File::|process::|serde_json::to/.test(native), 'Transfer must not substitute local JSON or execute/read user-selected files');
const actions=read('src/features/source_workspace/profile_actions.rs'), workspace=read('src/features/source_workspace.rs');
fact(actions.includes('SourceProfileTransfer::new') && actions.includes('cx.subscribe_in(') && actions.includes('SourceProfileTransferClosed'), 'Actual profile menu route missing');
fact(workspace.includes('mod profile_transfer;') && workspace.includes('.children(self.profile_transfer.clone())'), 'Transfer entity is not mounted');
const report={schema_version:1, method:'Current per-product mounted receipts re-bound to source; Acorn syntax and current manifest CSS parsed without execution.',
  products, assets, native:{path:nativePath,sha256:hash(native),
    state:'Retained entity, task, focus and one-shot close event. Export rows use actual supplied profiles. Seven products initially select the active id; 3886 initially selects all. All preserve the exact select-all toggle semantics.',
    files:'Actual native single-file prompt; cancellation preserves the previous path. Seven products validate .synapse4, while 3886 permits any extension and opens from its folder icon only. Selection never reads file contents.',
    cloud:'Seven products retain the source-disabled cloud notice. 3886 source enables compatible-device selection; the native dropdown is disabled with a local unavailable message because connected cloud device data is absent.',
    limitations:['Final Import and Export stay disabled: no vendor codec/device service has been implemented.',
      'No factory-preset identity is present in the native Profile model; no user profile is excluded by matching a name.',
      'No fabricated macro children, imported profiles, cloud profiles or transfer success.',
      'Export product heading uses the actual PID registry name. Local explanatory text adds height within the fixed modal.',
      'OS picker has no extension-filter API, so the seven source accept filters are enforced only after selection.',
      'Pointer/keyboard dispatch, focus restoration, display scaling, motion and pixels were not run.']}};
const markdown = [
  '# Current source product profile transfer', '',
  '2026-10-04. Audited products: 164, 241, 778, 784, 3871, 3884, 3886 and 3946.', '',
  'Each current mounted ImportExportModal and its row, cloud and local-browse children is bound to its own manifest-declared source. All eight use a 602×481 modal at top 104 and left calc(50% − 300px), a 36px header, 49px Local/Cloud area and 47px browse area.', '',
  'The profile menu now mounts a retained native dialog. Export shows actual local profile names; seven products initially select the active profile, while 3886 initially selects all. The source’s initially Deselect all link toggles independently from individual selections. No preset is inferred from a user-editable profile name.', '',
  'Import opens a real single-file OS picker, preserves the previous path on cancellation, and guards late results after switching Local/Cloud or closing. Seven products request .synapse4 and open from the whole filename row. Current 3886 has no accept filter and opens only from the folder image; these differences are retained.', '',
  'Current 3886 also enables its compatible-device cloud dropdown. Without connected compatible cloud devices the native dropdown remains disabled and states that cloud profiles are unavailable. The other seven retain the actual source-disabled cloud notice and cone resource.', '',
  'The final Import/Export buttons remain disabled because vendor .synapse4 encoding/decoding and device services are unavailable. Selected files are not read or executed. The unrelated razer-ui-profile JSON format is never substituted. No imported profile, remote data, macro row or transfer success is fabricated.', '',
  'The close, folder and cone SVGs match actual current 3946 resources byte for byte. The folder and cone were fetched from their exact production manifest paths using the maintained snapshot helper; HTTP receipts remain next to those source resources.', '',
  'The source has no Escape, Enter or backdrop-close handlers. Base Dialog/DialogPopup consume those actions, trap focus and occlude underlying content; explicit close/cancel emits one retained close event and the owner restores profile focus.', '',
  'Run `node tools/audit-source-profile-transfer.cjs --check` for a read-only source/asset/native-route audit. Formatting and parent-owned cargo check are permitted; applications, builds, tests, downloaded JavaScript and DLLs were not executed. Pixel, animation and interactive behavior have not been run.', '',
  'See [source-profile-transfer-current-evidence.json](source-profile-transfer-current-evidence.json) for complete current receipts and explicit native limitations.', '',
].join('\n');
const outputs=[['docs/re/source-profile-transfer-current-evidence.json',JSON.stringify(report,null,2)+'\n'],['docs/re/source-profile-transfer-current-audit.md',markdown]];
const prepared = assets.filter(asset => !asset.target.endsWith('/profiles-close.svg'));
outputs.push(['assets/synapse/source-profile-transfer-manifest.json',JSON.stringify(prepared.map(asset=>({
  source:asset.reference, output:asset.target, source_sha256:asset.sha256, output_sha256:asset.sha256,
  url:`https://apps.razer.com/synapse/products/3946/ui/static/media/${asset.filename}`,
})),null,2)+'\n']);
for(const asset of prepared) outputs.push([asset.target,read(asset.reference)]);
if(process.argv.includes('--check')) {
  for(const [file,value] of outputs) fact(read(file)===value,`Stale transfer audit: ${file}`);
  console.log('Current eight-product transfer source, differences, exact resources and native route verified; no files written.');
} else {
  for(const [file,value] of outputs) fs.writeFileSync(path.join(root,file),value);
  console.log('Wrote current eight-product profile transfer evidence.');
}
