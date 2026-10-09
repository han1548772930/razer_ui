// Static extraction only: never evaluate a downloaded webpack module.
const fs = require('fs'), path = require('path');
const {Source, walk, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const source = new Source('synapse/dashboard');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const contracts = ['z','E','Z','k','T','U','B','F','W','H','xi','xe'].map(name => ({
  module:22534,name,...source.receipt(22534,source.binding(22534,name)),
}));
const keys = ['pAm','a58','Km','PPh','p6C','Aw1','j7H','HhA','uKP','AAO','pfH','f03','Uef','nmq','Sx7','OdY'];
const labels = Object.fromEntries(keys.map(name=>[name,source.literal(54693,source.exported(54693,name))]));
const cssPath = `${source.directory}/static/css/55.4e8559cb.chunk.css`;
const css = read(cssPath);
const rules = parseCSS(css).filter(rule => /^(?:\.box-item|\.boxFirmwareIcon|\.count-icon|\.windows-dynamic-lighting|\.device-state|\.firmwareNoti|\.flip-left\.firmwareNoti|\.btn(?:\b|[.: ])|\.warning(?::before)?$)/.test(rule.selector));
const start = css.indexOf('@keyframes threeDotLoading');
if(start<0) throw Error('Missing current preset-loading animation');
let end = start, depth = 0, began = false;
for(;end<css.length;end++) { if(css[end]==='{'){depth++;began=true;} if(css[end]==='}'&&--depth===0&&began){end++;break;} }
const keyframes=css.slice(start,end);
const manifest=JSON.parse(read('assets/synapse/manifest.json'));
const assets=manifest.entries.filter(entry=>entry.output.startsWith('assets/synapse/dashboard-card/'));
if(assets.length!==17) throw Error(`Expected 17 current Dashboard state resources, got ${assets.length}`);
for(const asset of assets) {
  const original=fs.readFileSync(path.join(root,asset.source)), output=fs.readFileSync(path.join(root,asset.output));
  if(hash(original)!==asset.source_sha256||hash(output)!==asset.sha256) throw Error(`Resource drift: ${asset.output}`);
  if(asset.output.endsWith('.svg')&&hash(original)!==hash(output)) throw Error(`SVG was altered: ${asset.output}`);
}
const batteryMasks = [
  ['assets/synapse/battery-off.svg', null, null, '0 0 20 20', 26],
  ['assets/synapse/dashboard-card/xbox-icon.svg', '26', '26', '0 0 26 26', 26],
  ['assets/synapse/dashboard-card/ps-icon.svg', '24', '24', '0 0 24 24', 24],
].map(([output, width, height, viewBox, paintedSize]) => {
  const entry = manifest.entries.find(entry => entry.output === output);
  if (!entry || !css.includes(path.basename(entry.source))) throw Error(`Missing current CSS mask: ${output}`);
  const original = fs.readFileSync(path.join(root, entry.source)), bytes = fs.readFileSync(path.join(root, output));
  if (hash(original) !== entry.source_sha256 || hash(bytes) !== entry.sha256 || hash(original) !== hash(bytes)) throw Error(`Mask drift: ${output}`);
  const rootTag = bytes.toString('utf8').match(/<svg\b[^>]*>/)?.[0];
  if (!rootTag) throw Error(`Missing SVG root: ${output}`);
  const attr = name => rootTag.match(new RegExp(`\\b${name}="([^"]*)"`))?.[1] ?? null;
  if (attr('width') !== width || attr('height') !== height || attr('viewBox') !== viewBox) throw Error(`Mask dimensions changed: ${output}`);
  return {...entry, intrinsic:{width,height,viewBox}, cssBox:26, paintedSize,
    placement:'CSS mask-size:auto, mask-position:0% 0%; background-size:20px does not apply to the mask.'};
});
const batteryRules = rules.filter(rule=>/batt-icon/.test(rule.selector));
if(batteryRules.some(rule=>/(?:^|;)\s*(?:-webkit-)?mask(?:-size|-position|-repeat)?\s*:/.test(rule.declarations))) throw Error('Battery mask defaults require re-audit');
const deviceClass = source.binding(22534,'z');
const powerLifecycle = ['componentDidMount','componentDidUpdate'].map(name => {
  const method=deviceClass.body.body.find(method=>method.key?.name===name);
  if(!method) throw Error(`Missing battery lifecycle: ${name}`);
  const receipt=source.receipt(22534,method);
  if(!receipt.source.includes('[y.Dh.READY,y.Dh.WAITING].includes(') || !receipt.source.includes('generateBatteryData')) throw Error(`Battery acceptance guard changed: ${name}`);
  return {name,...receipt};
});
const powerStateWriters = [];
walk(deviceClass, node => {
  if(node.type==='AssignmentExpression' && node.left?.object?.type==='ThisExpression'
    && ['checkStandbyState','generateBatteryData','onWindowStorageChange'].includes(node.left.property?.name)) {
    powerStateWriters.push({name:node.left.property.name,...source.receipt(22534,node)});
  }
});
if(powerStateWriters.length!==3) throw Error('Missing scoped standby/battery writers');
const powerNative=read('crates/razer-dashboard/src/dashboard_device.rs');
for(const token of ['previous_support', 'previous_state', 'previous_hide', 'previous_show',
  'state.displayed.accept_standby(fields)', 'self.observed.with_battery', 'self.observed.icon.clone()'])
  if(!powerNative.includes(token)) throw Error('Missing sticky power projection '+token);
const spinnerPath=`${source.directory}/static/media/spinner.ef2d0235.svg`;
const spinner=read(spinnerPath);
for(const literal of ['stroke-linecap="square"','values="0 50 50;180 50 50;720 50 50"','keyTimes="0;0.5;1"']) {
  if(!spinner.includes(literal)) throw Error(`Spinner contract changed: ${literal}`);
}
const implementation = [
  'crates/razer-dashboard/src/dashboard_device_card.rs', 'crates/razer-dashboard/src/dashboard_device.rs',
  'crates/razer-shell/src/shell/main_pages/dashboard_cards.rs', 'crates/razer-dashboard/src/dashboard_grid.rs',
].map(file=>({path:file,sha256:hash(read(file))}));
const report={verification:'Current module-local AST, final CSS, maintained resource preparation and hash checks. No application, build, test or reference JavaScript was executed.',
  contracts,labels,css:{path:cssPath,sha256:hash(css),rules,keyframes},spinner:{path:spinnerPath,sha256:hash(spinner),source:spinner},assets,batteryMasks,powerLifecycle,powerStateWriters,implementation,
  local_entrypoints:'docs/re/local-device-entrypoints-current-evidence.json',
  implemented:[
    'Separate source image-container disabled and non-ready artwork blurred opacity; console name opacity and PlayStation artwork exception.',
    'Known controller/headset/earbuds/firmware art, retry/offline, actual spinner curve and square caps, restart warning, WDL and firmware affordances.',
    'Observed presetLoading value only, 100ms ease-in width and exact 600ms dot keyframes with 150/300ms initial delays; mixer init overlay.',
    'Source short-drag overrideAction for nested mouse controls; native keyboard activation stops card bubbling.',
    'Source can-focus short-circuit conditions and actual firmware-update inventory matching; recovery/install/restart-notification requests report missing transport without synthetic success.',
    'User-requested local entry override suppresses only installer spinner/retry/status/blur for held non-help renderers in six explicit setup phases, retaining can-focus and noAliveSign guards. Source device fields, readiness and battery observations are unchanged.',
    'Recovered standby-off and console glyphs; battery no-spinner gate, restart suppression, console text and edge-placement branches.',
    'Ready/waiting acceptance of actual power observations, retained display state in other setup states, raw power used independently for spinner, source 2-second display-only timers.',
    'Mask sizing separated from 20px background images: 26px viewBox-only power mask, intrinsic 26px Xbox and 24px PlayStation at the default top-left mask origin.',
    'Persistent isOff/isStandby/withBattery/icon/hide/show projection follows generateBatteryData, prop overrides and checkStandbyState ordering. Unknown states and unsupported branches retain prior fields as the current class does.',
    'Battery tooltip anchor follows actual flex height; source audio category and console generic-off exclusion, normal versus pre-line text whitespace.',
  ],
  unresolved:[
    'No runtime or pixel comparison is permitted; native layout/text rasterization and overlapping tooltip hit regions are not visually certified.',
    'CSS normal line-height, CJK fallback and pre-wrap/shrink-to-fit equivalence remain open as recorded in the shell consistency audit.',
    'Firmware navigates to Devices & Modules; source sessionStorage flag and smooth scroll to its first firmware row are not yet implemented.',
    'Service transport and continuous reducer/storage updates for retryInstall, resuscitate, notification restart, noAliveSign and dynamic-lighting are not connected; metadata is an observed projection only.',
    'Missing prepared ordinary artwork retains the source image slot; no Synapse-logo substitution or fabricated image-load failure fallback is used.',
    'Local snapshots do not carry source powerStatus object identity/revision: battery acceptance detects actual level/status changes, not replacement with an equal-valued JavaScript object. Sticky prop-driven standby transitions are projected locally; storage-only updates/removal/clear still need the real reducer adapter.',
  ]};
const output=JSON.stringify(report,null,2)+'\n',target=path.join(root,'docs/re/dashboard-card-state-current-evidence.json');
if(process.argv.includes('--check')) { if(fs.readFileSync(target,'utf8')!==output) throw Error('Dashboard card state receipt drifted'); }
else fs.writeFileSync(target,output);
console.log(JSON.stringify({contracts:contracts.length,css:rules.length,assets:assets.length,implementation:implementation.length}));
