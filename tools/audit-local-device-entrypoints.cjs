// Current source is parsed as data; neither vendor JS nor Rust is executed.
const fs = require('fs'), acorn = require('acorn');
const {Source, walk, hash, key} = require('./webpack-source.cjs');
const dashboard = new Source('synapse/dashboard');
const setup = dashboard.literal(29228, dashboard.exported(29228, 'Dh'));
const card = dashboard.binding(22534, 'z');
let statusLabels;
walk(card, node => {
  if (node.type !== 'ObjectExpression') return;
  const props = new Map(node.properties.filter(p => p.type === 'Property' && p.computed
    && p.key.type === 'MemberExpression' && key(p.key.object?.property) === 'Dh')
    .map(p => [key(p.key.property), p]));
  if (!props.has('ERROR') || !props.has('INSTALL_CANCELED') || !props.has('DOWNLOADING')) return;
  const values = Object.fromEntries([...props].map(([name, p]) => [name, dashboard.literal(22534, p.value)]));
  if (values.ERROR !== 'INSTALLATION_FAILED' || values.INSTALL_CANCELED !== values.ERROR)
    throw Error('ERROR is no longer the current installation-failed label');
  statusLabels = {values, ...dashboard.receipt(22534, node)};
});
if (!statusLabels) throw Error('Missing current Dashboard setup label map');

const picker = new Source('rz-app-menu');
const manifest = JSON.parse(fs.readFileSync(`${picker.directory}/asset-manifest.json`, 'utf8'));
const pickerPath = `${picker.directory}/${manifest.files['Root.js'].slice(2)}`;
const pickerText = picker.text(pickerPath);
let installerError, readyFilter;
walk(acorn.parse(pickerText, {ecmaVersion: 'latest'}), node => {
  if (node.type === 'SwitchCase' && node.test?.type === 'TemplateLiteral'
    && node.test.quasis.some(part => part.value.raw.endsWith('_installer-error'))) {
    const source = pickerText.slice(node.start, node.end);
    if (source.includes('deviceSetup/setupStatus') && source.includes('payload:"error"'))
      installerError = {path: pickerPath, sha256: hash(pickerText), offset: node.start, end: node.end, source};
  }
  if (node.type === 'ArrowFunctionExpression') {
    const source = pickerText.slice(node.start, node.end);
    if (source.length < 600 && source.includes('deviceContainerId')
      && source.includes('setupStatus') && source.includes('MIXER_SYSTEM_CHECK_FAILED')
      && source.includes('powerStatus'))
      readyFilter = {path: pickerPath, sha256: hash(pickerText), offset: node.start, end: node.end, source};
  }
});
if (!installerError || !readyFilter) throw Error('Missing current picker install-error/readiness source');

const requirements = {
  'crates/razer-pages/src/features/product_workspace.rs': ['pub(crate) fn has_local_page', 'Body::Source(workspace) => workspace.read(cx).has_local_page()'],
  'crates/razer-pages/src/features/source_workspace.rs': ['pub(crate) fn has_local_page', 'page.role() == ProductPageRole::Help', 'FamilyBody::Pending => false', 'self.dock_pairing.is_some()', 'self.supplement.is_some()', 'super::audio_products::supports_page', 'super::accessory_system_products::supports_page'],
  'crates/razer-dashboard/src/lib.rs': ['fn local_installation_entry', 'SetupStatus::Waiting', 'SetupStatus::Downloading', 'SetupStatus::Installing', 'SetupStatus::Syncing', 'SetupStatus::InstallCanceled', 'SetupStatus::Error', 'device.dashboard.no_alive_sign != Some(true)', 'dashboard_device::can_focus(device)'],
  'crates/razer-shell/src/shell/main_pages/dashboard_cards.rs': ['workspace.has_local_page(cx)', 'open_without_installation: local_installation_entry(device, supported)', 'if restart', 'Location::Device(key.clone())'],
  'crates/razer-dashboard/src/dashboard_device.rs': ['fn can_focus', '!min_firmware(device)', '!preset_loading(device)', '!power_off(device)', 'mixer_system_check_failed', 'SetupStatus::Updating | SetupStatus::RestartRequired'],
  'crates/razer-dashboard/src/dashboard_device_card.rs': ['let installation_gate = !self.open_without_installation', 'source_spinner && !retry && installation_gate', '!ready && installation_gate', 'fields.no_alive_sign == Some(true)', 'let ready = device.setup_status == SetupStatus::Ready'],
  'crates/razer-app-pages/src/app_picker.rs': ['(self.ready || self.open_without_installation)', 'open_without_installation: false', '&& !self.mixer_failed', '&& !self.powered_off'],
  'crates/razer-shell/src/shell/app_picker_host.rs': ['workspace.has_local_page(cx)', '.ready(device.setup_status == SetupStatus::Ready)', '.mixer_failed(', 'super::main_pages::local_installation_entry('],
  'crates/razer-shell/src/shell.rs': ['let mut local_page_devices = std::collections::BTreeSet::new()', 'workspace.has_local_page(cx)', 'local_page_devices.insert(', 'page.sync_local_devices(&devices, local_page_devices, cx)', 'workspace.has_local_page(cx))'],
  'crates/razer-app-pages/src/devices_modules_catalog.rs': ['local_page_devices: BTreeSet<(u32, String, String)>', 'local_page_devices: BTreeSet::new()', 'self.local_page_devices = local_page_devices'],
  'crates/razer-app-pages/src/module_service_rows.rs': ['fn local_service_device', 'fn local_page_service_device', 'self.local_page_devices', '.contains(&identity)', '.then_some(device)', 'if let Some(device) = self.local_page_service_device(row)', 'let local = self.local_service_device(row).cloned()'],
};
const implementation = Object.entries(requirements).map(([path, tokens]) => {
  const source = fs.readFileSync(path, 'utf8');
  for (const token of tokens) if (!source.includes(token)) throw Error(`${path}: missing ${token}`);
  if (/setup_status\s*=\s*SetupStatus::Ready\b/.test(source)) throw Error(`Fabricated readiness in ${path}`);
  return {path, sha256: hash(source)};
});
const result = {method:'Current Acorn AST and native static contract checks; no application, build or test execution.',
  source:{setup,status_labels:statusLabels,installer_error:installerError,picker_ready_filter:readyFilter},
  local_policy:{phases:['waiting','downloading','installing','syncing','install_canceled','error'],
    capability:'Retained product renderer and a non-help page; Pending and help-only source bodies do not qualify.',
    guards:'Original Dashboard can_focus plus noAliveSign != true. Updating, restart-required, firmware minimum, preset loading, console mode, mixer failure and off/standby do not gain a new local opener.',
    projection:'Card installer presentation and picker visibility only. Raw setup, firmware, battery, WDL and runtime fields remain untouched.',
    service_catalog:'New-device Open requires an identity-matched live workspace with a retained non-help page; firmware associations retain the full local device snapshot.',
    preview:'PickerDevice defaults the separate local override to false; standalone source-state previews preserve the readiness filter.'},
  implementation};
const output='docs/re/local-device-entrypoints-current-evidence.json', rendered=JSON.stringify(result,null,2)+'\n';
if(process.argv.includes('--check')) {
  if(fs.readFileSync(output,'utf8')!==rendered) throw Error('Local device entrypoint evidence is stale');
} else fs.writeFileSync(output,rendered);
console.log('Local device entrypoints: current install-error semantics, retained renderers and unchanged runtime guards verified.');
