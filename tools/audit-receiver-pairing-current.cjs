// Current 179 pairing tree, product-owned labels and CSS. No vendor execution.
const fs = require('fs');
const path = require('path');
const { Source, walk, key, hash } = require('./webpack-source.cjs');
const { parseCSS } = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const check = process.argv.includes('--check');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const source = Object.create(Source.prototype);
source.directory = '.ref/devices/179';
const manifestPath = source.directory + '/asset-manifest.json';
const manifest = JSON.parse(read(manifestPath));
source.files = [...new Set(Object.values(manifest.files))]
  .filter(file => /^\.\/static\/js\/[^/]+\.js$/.test(file))
  .map(file => source.directory + '/' + file.slice(2));
source.modules = new Map(); source.texts = new Map(); source.parsed = new Set();
const names = ['W', 'x', 'J', 'd', 'SE', 'ae', '_e', 'te', 'ie', 'oe', 'ne', 're', 'se', 'Te', 'mE'];
const components = Object.fromEntries(names.map(name => [name, source.receipt(9473, source.binding(9473, name))]));
if (!components.ne.source.includes('I?"":"disabled"') ||
    !components.oe.source.includes('a===W.UNPAIRING?') ||
    !components.se.source.includes('status:W.JUSTUNPAIR') ||
    !components.se.source.includes('1===_.length?')) throw Error('Pairing branch contract changed');
const labelExports = ['PXm','PRj','owr','VcX','XBA','PSG','DxM','kTs','T8n','_AH','j04','bOp','Mh0','kiV','BuT','kEL','rQd','lwW','ca6','evT','RFv','Thf','zt5','o1A','CRt','ssI','UVv'];
const labels = Object.fromEntries(labelExports.map(name => [name, source.literal(4693, source.exported(4693, name))]));
const needed = new Set([...Object.values(labels), 'MOUSE', 'KEYBOARD', 'LOADING', 'CLOSE']);
const localeModule = source.module(5955), localeMap = source.binding(5955, 't5O');
const translations = {};
const localeGetters = [];
walk(localeModule.fn, node => {
  if (node.type === 'CallExpression' && node.callee.property?.name === 'd' &&
      node.arguments[0]?.type === 'Identifier' && node.arguments[1]?.type === 'ObjectExpression') {
    localeGetters.push(node);
  }
});
for (const entry of localeMap.properties) {
  const locale = entry.computed ? source.literal(5955, entry.key) : key(entry.key);
  const exports = localeGetters.find(node => node.arguments[0].name === entry.value.name);
  if (!exports) throw Error('Missing product locale namespace: ' + locale);
  translations[locale] = Object.fromEntries(exports.arguments[1].properties
    .filter(property => needed.has(key(property.key)))
    .map(property => [key(property.key), source.literal(5955, property.value.body)]));
}
for (const [locale, values] of Object.entries(translations)) {
  for (const label of Object.values(labels)) if (typeof values[label] !== 'string') throw Error(`Missing ${locale}/${label}`);
}
const cssPath = source.directory + '/static/css/main.c778525e.css';
const css = read(cssPath);
const rules = parseCSS(css).filter(rule => /^(\.hyperpolling-device-content|\.nospecificdevice|\.HyperPollingWirelessUma_|\.radio-item|\[type=radio\])/.test(rule.selector) ||
  rule.conditions.some(condition => condition.includes('HyperPollingWirelessUma_myanimation')));
const assets = JSON.parse(read('assets/synapse/receiver-pairing-manifest.json'));
for (const asset of assets) {
  if (hash(fs.readFileSync(path.join(root, asset.source))) !== asset.source_sha256 ||
      hash(fs.readFileSync(path.join(root, asset.output))) !== asset.sha256) throw Error('Changed pairing asset ' + asset.output);
}
const nativePaths = ['receiver.rs', 'receiver_pairing_state.rs', 'receiver_pairing_view.rs', 'receiver_pairing_state_tests.rs']
  .map(file => 'src/features/source_controls/' + file);
const data = { translations };
const evidence = {
  product_id: 179,
  method: 'Acorn current mounted state tree, product-owned 10-locale getter namespaces, CSS and independent asset hashes; no execution of vendor code',
  generator_sha256: hash(fs.readFileSync(__filename)),
  manifest: { path: manifestPath, sha256: hash(read(manifestPath)) },
  components, labels,
  locale_map: source.receipt(5955, localeMap),
  css: { path: cssPath, sha256: hash(css), rules },
  assets,
  native: nativePaths.map(file => ({ path: file, sha256: hash(read(file)) })),
  test_support_ids: [...read('src/features/source_controls/receiver_pairing_view.rs').matchAll(/(?:\.id|BaseButton::new)\("(receiver-[^"]+)"\)/g)].map(match => match[1]),
};
for (const [file, value] of [
  ['src/features/source_controls/receiver_pairing_data.json', data],
  ['docs/re/receiver-pairing-current-evidence.json', evidence],
]) {
  const content = JSON.stringify(value, null, 2) + '\n';
  if (check) { if (read(file) !== content) throw Error('Stale ' + file); }
  else fs.writeFileSync(path.join(root, file), content);
}
console.log(`179 pairing: ${names.length} AST nodes, ${rules.length} CSS rules, ${Object.keys(translations).length} source locale maps, ${assets.length} assets.`);
