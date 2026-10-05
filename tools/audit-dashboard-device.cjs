// Static current Dashboard device-card contracts. Never execute reference JS.
const fs = require('fs'), path = require('path');
const {Source, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const source = new Source('synapse/dashboard');
const contracts = ['z','G','V','K','Pi','xi','H'].map(name => ({module:22534,name,...source.receipt(22534,source.binding(22534,name))}));
const enums = ['pV','Dh'].map(name => ({module:29228,name,value:source.literal(29228,source.exported(29228,name)),...source.receipt(29228,source.binding(29228,source.exported(29228,name).name))}));
const keys = ['uEA','FHr','Uxl','wch','ZdF','jA5','Z55','CwW','CnU','wWZ','fc0'];
const labels = Object.fromEntries(keys.map(name => [name,source.literal(54693,source.exported(54693,name))]));
const cssPath = `${source.directory}/static/css/55.4e8559cb.chunk.css`;
const cssText = fs.readFileSync(path.join(root,cssPath),'utf8');
const rules = parseCSS(cssText).filter(r => /box-item.*(?:name-tag|batt|display-number)|^\.low-batt$|^\.disabled$|count-icon/.test(r.selector));
const manifest = JSON.parse(fs.readFileSync(path.join(root,'assets/synapse/manifest.json'),'utf8'));
const assets = manifest.entries.filter(e => /\/battery-(?!(?:disconnected))/.test(e.output)).map(entry => {
  // Current Dashboard declares these exact hashed URLs. Where its own media
  // cache is incomplete, current product 182 carries the same hashed asset.
  const filename = path.basename(entry.source);
  if (!rules.some(r => r.declarations.includes(filename))) throw Error(`No Dashboard selector for ${filename}`);
  const sourceBytes = fs.readFileSync(path.join(root,entry.source));
  const outputBytes = fs.readFileSync(path.join(root,entry.output));
  if (hash(sourceBytes)!==entry.source_sha256 || hash(outputBytes)!==entry.sha256) throw Error(`Resource drift ${entry.output}`);
  return {...entry, dashboard_reference:filename, checked_source_sha256:hash(sourceBytes), checked_output_sha256:hash(outputBytes)};
});
const missing = ['icon_device_power_state_off.6b8c9694.svg','xbox-icon.11f09412.svg','ps-icon.54354df0.svg']
  .map(name=>({path:`${source.directory}/static/media/${name}`,present:fs.existsSync(path.join(root,source.directory,'static/media',name))}));
const result = {verification:'Current module-local AST/CSS and resource hash parsing only; no app, build or tests executed.',contracts,enums,labels,css:{path:cssPath,sha256:hash(cssText),rules},assets,missing,
  scope:'Source names/edition/profile conditions; ordinary battery levels, unknown/charging/off/paused states and 2-second delay. Current standby power-off and console resources are now present and mounted. Card-state trees and interaction changes have a separate dashboard-card-state-current-evidence.json receipt; source service transport and runtime visual parity remain incomplete.'};
const json = JSON.stringify(result,null,2)+'\n', output=path.join(root,'docs/re/dashboard-device-current-evidence.json');
if(process.argv.includes('--check')) {
  if(fs.readFileSync(output,'utf8')!==json) throw Error('Dashboard device receipt drifted');
} else fs.writeFileSync(output,json);
console.log(JSON.stringify({contracts:contracts.length,enums:enums.length,css:rules.length,assets:assets.length,missing:missing.filter(x=>!x.present).length}));
