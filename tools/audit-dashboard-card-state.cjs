// Static extraction only: never evaluate a downloaded webpack module.
const fs = require('fs'), path = require('path');
const {Source, hash} = require('./webpack-source.cjs');
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
const spinnerPath=`${source.directory}/static/media/spinner.ef2d0235.svg`;
const spinner=read(spinnerPath);
for(const literal of ['stroke-linecap="square"','values="0 50 50;180 50 50;720 50 50"','keyTimes="0;0.5;1"']) {
  if(!spinner.includes(literal)) throw Error(`Spinner contract changed: ${literal}`);
}
const implementation = [
  'src/shell/main_pages/dashboard_device_card.rs', 'src/shell/main_pages/dashboard_device.rs',
  'src/shell/main_pages/dashboard_cards.rs', 'src/shell/main_pages/dashboard_grid.rs',
].map(file=>({path:file,sha256:hash(read(file))}));
const report={verification:'Current module-local AST, final CSS, maintained resource preparation and hash checks. No application, build, test or reference JavaScript was executed.',
  contracts,labels,css:{path:cssPath,sha256:hash(css),rules,keyframes},spinner:{path:spinnerPath,sha256:hash(spinner),source:spinner},assets,implementation,
  implemented:[
    'Separate source image-container disabled and non-ready artwork blurred opacity; console name opacity and PlayStation artwork exception.',
    'Known controller/headset/earbuds/firmware art, retry/offline, actual spinner curve and square caps, restart warning, WDL and firmware affordances.',
    'Observed presetLoading value only, 100ms ease-in width and exact 600ms dot keyframes with 150/300ms initial delays; mixer init overlay.',
    'Source short-drag overrideAction for nested mouse controls; native keyboard activation stops card bubbling.',
    'Source can-focus short-circuit conditions and actual firmware-update inventory matching; recovery/install/restart-notification requests report missing transport without synthetic success.',
    'Recovered standby-off and console glyphs; battery no-spinner gate, restart suppression, console text and edge-placement branches.',
  ],
  unresolved:[
    'No runtime or pixel comparison is permitted; native layout/text rasterization and overlapping tooltip hit regions are not visually certified.',
    'CSS normal line-height, CJK fallback and pre-wrap/shrink-to-fit equivalence remain open as recorded in the shell consistency audit.',
    'Firmware navigates to Devices & Modules; source sessionStorage flag and smooth scroll to its first firmware row are not yet implemented.',
    'Service transport and continuous reducer/storage updates for retryInstall, resuscitate, notification restart, noAliveSign and dynamic-lighting are not connected; metadata is an observed projection only.',
    'Missing prepared ordinary artwork retains the source image slot; no Synapse-logo substitution or fabricated image-load failure fallback is used.',
  ]};
const output=JSON.stringify(report,null,2)+'\n',target=path.join(root,'docs/re/dashboard-card-state-current-evidence.json');
if(process.argv.includes('--check')) { if(fs.readFileSync(target,'utf8')!==output) throw Error('Dashboard card state receipt drifted'); }
else fs.writeFileSync(target,output);
console.log(JSON.stringify({contracts:contracts.length,css:rules.length,assets:assets.length,implementation:implementation.length}));
