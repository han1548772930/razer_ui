// Current-source UI review receipts. Parse reference JavaScript/CSS as data only.
// Never import, require, evaluate, or execute production application modules.
const fs = require('fs');
const path = require('path');
const {Source, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const source = new Source('synapse/dashboard');
const contracts = [];
for (const [moduleId, symbols] of [
  [35378, ['x']],
  [22534, ['Pi', 'Li', 'Ti', 'xi', 'bi', 'xe', 'be', 'z', 'Y', 'G', 'V', 'K', 'X', 'Q', 'ee', 'te', 'de', 'ce', 'ji', 'Ai', 'gi']],
  [1285, ['n', 'o', 'a']],
  [44442, ['ae', 'H', 'w', 'O', 'L', 'z', 'ne', 'te', 'ee', 'ie', 'u']],
  [96689, ['c']],
  [19388, ['He', 'Oe', 'je', 'ze', 'ie', 'o', 'a', 'l', 'j', 'b', 'u', 're', 'fe', 'S', 'Pe']],
  [94608, ['Fe', 'Ve', 'Oe', 'Te', 'be', 'ie', 'se']],
]) {
  for (const symbol of symbols) {
    const node = source.binding(moduleId, symbol);
    contracts.push({module_id: moduleId, symbol, ...source.receipt(moduleId, node)});
  }
}
const available = source.exported(22431, 'AVAILABLE_MODULES');
contracts.push({module_id: 22431, symbol: available.name,
  ...source.receipt(22431, source.binding(22431, available.name))});
const locales = {};
for (const symbol of ['CAr','MMV','Qu2','zV8','QBW','kEL','Rav','BSg','iwS','fUK','D58','jLf','QgG','hmK','uMo','Axi','HYh','qST','d8g']) {
  locales[symbol] = source.literal(54693, source.exported(54693, symbol));
}
const css = [];
for (const filename of ['55.4e8559cb.chunk.css','7861.a49b4dc6.chunk.css','App.5fe618fc.chunk.css','6505.9782778c.chunk.css','4608.cb053604.chunk.css']) {
  const relative = `${source.directory}/static/css/${filename}`;
  const text = fs.readFileSync(path.join(root, relative), 'utf8');
  const rules = parseCSS(text).filter(r => /dashboard|\.box\b|box-item|box-no-device|movableBox|introduction-banner|collapse-action|body-wrapper|^body(?:,|$)|^html(?:,|$)|^\.items|#gamerRoom|gamer-room-tutorial|shortcut|box_img_2/.test(r.selector));
  css.push({path: relative, sha256: hash(text), rules});
}
// Local receipt anchors are deliberately refreshed on each invocation: other
// authorized work may change these files while this independent review runs.
const local = [
  'src/shell.rs', 'src/shell/main_pages.rs', 'src/shell/main_pages/dashboard_cards.rs',
  'src/shell/main_pages/dashboard_grid.rs', 'src/shell/main_pages/dashboard_group.rs',
  'src/shell/main_pages/dashboard_tutorial.rs', 'src/shell/service_pages.rs',
  'src/shell/gamer_room.rs', 'src/shell/gamer_room_devices.rs',
  'src/shell/gamer_room_presentation.rs',
  'src/shell/gamer_room_hotspot.rs',
  'src/shell/module_preview.rs',
  'src/features/shortcuts.rs',
  'src/shell/chroma_page.rs',
  'src/model.rs', 'src/main.rs', 'src/ui/theme.rs',
].map(file => ({path: file, sha256: hash(fs.readFileSync(path.join(root, file)))}));
// Chroma's current manifest uses application-absolute paths. Normalize only
// that exact manifest prefix, without widening Source's allow-list to disk.
const chroma = new Source('chroma-app/dashboard');
const chromaManifest = JSON.parse(fs.readFileSync(path.join(root, chroma.directory, 'asset-manifest.json'), 'utf8'));
chroma.files = [...new Set(Object.values(chromaManifest.files))]
  .filter(file => /^\/chroma-app\/dashboard\/static\/js\/[^/]+\.js$/.test(file))
  .map(file => `.ref/applications${file}`);
const chromaContracts = [];
for (const [moduleId, symbols] of [
  [23322, ['Ks', 'Fs', 'nt']],
  [62296, ['Wn', 'Pn', 'Kt', 'Qt', 'Ss', 'Cs', 'bs', 'ue', 'Qe', 'et']],
  [12093, ['B', 'd']],
  [40554, ['K', 'Z']],
]) for (const symbol of symbols) {
  chromaContracts.push({module_id: moduleId, symbol,
    ...chroma.receipt(moduleId, chroma.binding(moduleId, symbol))});
}
const chromaCss = [...new Set(Object.values(chromaManifest.files))]
  .filter(file => /^\/chroma-app\/dashboard\/static\/css\/(?:332|6883|9700|6570)\.[a-f0-9]+\.chunk\.css$/.test(file))
  .map(file => file.split('/').pop())
  .map(filename => {
    const relative = `${chroma.directory}/static/css/${filename}`;
    const text = fs.readFileSync(path.join(root, relative), 'utf8');
    return {path: relative, sha256: hash(text), rules: parseCSS(text).filter(r =>
      /introduction-banner|chroma-app-dashboard-wrapper|nav-tabs|^\.nav\.active|^\.items/.test(r.selector))};
  });
const receipt = {
  review_date: '2026-10-05',
  verification: 'Static Acorn/CSS parsing only; no application, build, tests, downloaded JS or DLL executed.',
  offset_unit: 'JavaScript UTF-16 code units; end is exclusive. CSS offsets are in the same unit.',
  route: 'App module 35378 x -> DASHBOARD_HEADER lazy chunk 7861 module 22534 -> Ti -> Li -> Pi',
  contracts, locales, css, local,
  chroma: {route: '23322/Ks -> 62296/Wn (Dashboard); 40554/K (Devices & Modules)',
    contracts: chromaContracts, css: chromaCss},
};
const output = 'docs/re/ui-source-review-2026-10-05-evidence.json';
fs.writeFileSync(path.join(root, output), JSON.stringify(receipt, null, 2) + '\n');
console.log(JSON.stringify({output, contracts: contracts.length, css_rules: css.reduce((n,x) => n+x.rules.length,0), local_files: local.length,
  chroma_contracts: chromaContracts.length, chroma_css_rules: chromaCss.reduce((n,x) => n+x.rules.length,0)}));
