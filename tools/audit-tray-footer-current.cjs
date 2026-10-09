// Current systray footer/launcher evidence. Parse source only; never run JS.
const fs = require('fs');
const path = require('path');
const { Source, walk, hash } = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const bytes = file => fs.readFileSync(path.join(root, file));
const assert = (value, message) => { if (!value) throw Error(message); };
const source = new Source('systray/systrayv2');
const receipts = [];
function receipt(file, symbol, node, purpose) {
  const text = read(file);
  const item = { path: file, sha256: hash(bytes(file)), symbol,
    start: node.start, end: node.end, source: text.slice(node.start, node.end), purpose };
  receipts.push(item);
  return item;
}
const main = source.module(5596);
const version = (() => {
  const found = [];
  walk(main.fn, node => {
    if (node.type === 'VariableDeclarator' && node.id.name === 'i' && node.init?.type === 'Literal') found.push(node);
  });
  return found.find(node => node.init.value === '1.1.97');
})();
assert(version, 'Current source version literal changed');
receipt(main.file, 'main:version-1.1.97', version, 'Displayed by ee as v + M.hl when REACT_APP_SHOW_VERSION is enabled');
const versionFlag = source.binding(5596, 'T');
let declaredVersionFlag = false;
walk(versionFlag, node => {
  if (node.type === 'Property' && (node.key.name ?? node.key.value) === 'REACT_APP_SHOW_VERSION') declaredVersionFlag = true;
});
assert(!declaredVersionFlag, 'SHOW_VERSION environment changed; re-audit its truth value');
receipt(main.file, 'main:SHOW_VERSION flag T', versionFlag,
  'Missing REACT_APP_SHOW_VERSION gives !undefined = true; current component is enabled');
const showVersion = (() => {
  const found = [];
  walk(source.module(2554).fn, node => {
    if (node.type === 'VariableDeclarator' && node.id.name === 'ee') found.push(node);
  });
  return found[0];
})();
assert(showVersion, 'Version component changed');
receipt(source.module(2554).file, '554:component ee', showVersion, 'Fixed app-version footer component');
receipt(source.module(2554).file, '554:component me', source.binding(2554, 'me'),
  'Launcher list receives app.launchers; one item renders title');
receipt(source.module(2554).file, '554:component Re', source.binding(2554, 'Re'),
  'Account branch and body mount condition');
receipt(source.module(597).file, '597:resize c', source.binding(597, 'c'),
  'Window height: header/apps initial height, account children +153 clamp');
receipt(source.module(5492).file, '492:synapse widget Y', source.binding(5492, 'Y'),
  'Body child section title + synapse devices container');
receipt(source.module(5492).file, '492:synapse device list b', source.binding(5492, 'b'),
  'Device count effect uses 60*count and title allowance TH=27');
receipt(source.module(9001).file, '9001:stable Synapse logo u', source.binding(9001, 'u'),
  'Default launcher stable Synapse image comes from systray media logo_synapse, not the host menu icon');
const manifest = JSON.parse(read(source.directory + '/asset-manifest.json'));
const cssRelative = Object.values(manifest.files).find(file => file.endsWith('/554.7cdbd936.chunk.css'));
const css = cssRelative && source.directory + '/' + cssRelative.slice(2);
assert(css, 'Current tray CSS missing');
const cssText = read(css);
for (const [selector, snippets, purpose] of [
  ['.systray>.apps', ['height:60px', 'border-top:1px solid #222', 'background-color:#111'], 'Launcher container is exactly 60px'],
  ['.systray>.apps>li', ['height:59px', 'padding:0 10px', 'justify-content:center'], 'Launcher row geometry'],
  ['.systray>.apps>li>.icon', ['height:32px', 'width:32px', 'flex-shrink:0'], 'Launcher icon geometry'],
  ['.systray>.apps.apps-1>li>.icon', ['margin-right:10px'], 'Single launcher icon/title gap'],
  ['.systray>.apps>li>.title', ['font-size:12px', 'text-overflow:ellipsis', 'white-space:nowrap', 'color:#999'], 'Single launcher title typography'],
  ['.app-version', ['position:fixed', 'bottom:0', 'right:0', 'font-size:12px', 'color:#888', 'padding:2px 5px'], 'Version footer fixed viewport placement'],
  ['.systray .app-section-title', ['font-size:10px', 'padding:6px 0', 'border-top:2px solid #111'], 'Section label height: 10*1.22+12+2=26.2 CSS px'],
]) {
  const offset = cssText.indexOf(selector);
  assert(offset >= 0 && snippets.every(snippet => cssText.slice(offset, offset + 700).includes(snippet)), `CSS changed: ${selector}`);
  receipt(css, `css:${selector}`, {start: offset, end: cssText.indexOf('}', offset) + 1}, purpose);
}
const output = {
  schema_version: 1,
  method: 'Acorn/static text only; no reference JS, app, host or DLL execution',
  source_inputs: Object.fromEntries([...new Set([source.directory + '/asset-manifest.json', ...receipts.map(item => item.path)])]
    .map(file => [file, hash(bytes(file))])),
  receipts,
  source_values: {
    version: '1.1.97',
    version_display: 'v1.1.97',
    launcher_container_height: 60,
    launcher_item_height: 59,
    launcher_icon: { width: 32, height: 32 },
    launcher_title: { font_size: 12, uppercase: true },
    version_footer: { position: 'fixed', bottom: 0, right: 0, padding: '2px 5px' },
  },
  height_calculation: {
    signed_out: { header_2: 60, apps: 60, requested_window_height: 60,
      note: 'Source queries .app.list-unstyled (singular) but me renders .apps (plural), so apps height is not added.' },
    account_formula: 'sum(systrayBody children offsetHeight) + 153; clamp to 400..700; body maxHeight = requested - 153',
    example_two_device_rows: { device_rows: 120, section_title_css_height: 26.2,
      section_title_offset_height: 26, children_offset_height: 146, plus_153: 299,
      clamped_window_height: 400, body_max_height: 247,
      source_device_count_effect_allowance: 27,
      note: 'For one Synapse section with exactly two visible normal 60px rows; offsetHeight is integer-rounded. DPR/font/layout observations are not executed.' },
    note: 'Two launcher items share the fixed 60px apps container; they do not create a 700px window. The account formula applies to systrayBody children, not launcher count.',
  },
  implementation_boundary: {
    launcher_component: 'src/shell/tray/launcher.rs accepts observed launchers only; no fixed Synapse fixture.',
    version: 'Caller must pass audited source version; Cargo package version is not substituted.',
    runtime_validation: 'not_run',
  },
};
const target = 'docs/re/tray-footer-current-evidence.json';
const serialized = JSON.stringify(output, null, 2) + '\n';
if (process.argv.includes('--check')) assert(read(target).replace(/\r\n/g, '\n') === serialized, 'Tray footer evidence stale');
else fs.writeFileSync(path.join(root, target), serialized);
console.log(`Tray footer: ${receipts.length} source receipts and exact 2-row height calculation checked.`);
