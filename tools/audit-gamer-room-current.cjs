// Current-source receipts only: Acorn/CSS parsing, no vendor JS execution.
const fs = require('fs');
const path = require('path');
const { Source, hash, walk } = require('./webpack-source.cjs');
const { parseCSS } = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const source = new Source('synapse/dashboard');
const symbols = ['He', 'Oe', 'je', 'oe', 'ze', 'ue', 'fe', 'ne', 'S', 'Q', 'o', 'a', 'b', 'u', 'ie', 'ce', 'le', 'w', 'ee', 'te', 'U', 'A', 'E', 'Pe', 'l', 'j', '_', '$'];
const contracts = symbols.map(symbol => ({ module_id: 19388, symbol, ...source.receipt(19388, source.binding(19388, symbol)) }));
contracts.push({ module_id: 50527, symbol: 'v', ...source.receipt(50527, source.binding(50527, 'v')) });
contracts.push({ module_id: 26079, symbol: 'module', ...source.receipt(26079, source.module(26079).fn) });
const cssFile = '.ref/applications/synapse/dashboard/static/css/55.4e8559cb.chunk.css';
const css = fs.readFileSync(path.join(root, cssFile), 'utf8');
const rules = parseCSS(css).filter(rule =>
  /#gamerRoom|gamer-room-tutorial|^\.help(?:\b|\s)|^\.switch(?:\b|\s)|^\.box-item$|^\.box-item \.disabled$|^\.disabled$|\.dashboard \.box-group.*(?:content|collapse|title)|^\.collapse-action/.test(rule.selector));
const at = css.indexOf('@keyframes expand{');
if (at < 0) throw Error('Missing current expand keyframe');
let end = css.indexOf('{', at) + 1, nesting = 1;
while (nesting) { if (css[end] === '{') nesting++; else if (css[end] === '}') nesting--; end++; }
const locales = {};
for (const name of ['d7M', 'js6', 'Huw', 'ArX', 'O29', 'G1D', 'QIe', 'avl', 'lLO', 'ypT', 'wn3', 'uEA', 'FHr', 'Uxl', 'wch', 'ZdF', 'jA5']) {
  locales[name] = source.literal(54693, source.exported(54693, name));
}
const setup = source.literal(29228, source.exported(29228, 'Dh'));
const imageBase = source.literal(69937, source.exported(69937, 'sKg'));
if (imageBase !== 'https://app-assets.razer.com/files/synapse/') throw Error('Changed device image fallback root');

// Prove the external-link path was extracted from o's inline JSX, not a similar
// host icon. Compare semantic SVG attributes, allowing XML serialization syntax.
let externalPath;
walk(source.binding(19388, 'o'), node => {
  if (node.type !== 'CallExpression' || node.arguments[0]?.value !== 'path') return;
  const properties = node.arguments[1]?.properties;
  const d = properties?.find(property => property.key.name === 'd')?.value;
  if (d?.type === 'Literal') externalPath = d.value;
});
if (!externalPath) throw Error('Missing current inline link path');
const assetNames = ['gr-external-link.svg', 'gr-help.svg', 'gr-device-synapse.svg', 'gr-device-app.svg',
  'gr-popup-bottom.svg', 'gr-popup-top.svg', 'gr-device-power.svg', 'gr-device-power-active.svg',
  'gr-device-offline.svg', 'gr-device-offline-description.svg'];
const manifest = JSON.parse(fs.readFileSync(path.join(root, 'assets/synapse/manifest.json'), 'utf8'));
const assets = assetNames.map(name => {
  const file = `assets/synapse/${name}`;
  const bytes = fs.readFileSync(path.join(root, file));
  const record = manifest.entries.find(record => record.output === file);
  if (!record || hash(bytes) !== record.sha256 || hash(fs.readFileSync(path.join(root, record.source))) !== record.source_sha256)
    throw Error(`Missing or stale resource provenance: ${name}`);
  if (name === 'gr-external-link.svg' && (!bytes.toString().includes(`d="${externalPath}"`) || !bytes.toString().includes('viewBox="0 0 21 20"')))
    throw Error('External-link SVG diverges from current inline path or dimensions');
  if (name === 'gr-help.svg' && !bytes.equals(fs.readFileSync(path.join(root,
      '.ref/applications/synapse/dashboard/static/media/tooltip_questionmark.96138d2f.svg'))))
    throw Error('Help glyph diverges from current CSS asset');
  return { path: file, sha256: hash(bytes), source: record.source, source_sha256: record.source_sha256 };
});
const hotspotPath = 'docs/re/gamer-room-hotspot-current-evidence.json';
const hotspot = JSON.parse(fs.readFileSync(path.join(root, hotspotPath), 'utf8'));
if (hash(fs.readFileSync(path.join(root, hotspot.source.path))) !== hotspot.source.sha256)
  throw Error('Recovered current hotspot source changed');
const hotspotData = 'crates/razer-app-pages/src/gamer_room_hotspot_data.json';
if (JSON.stringify(JSON.parse(fs.readFileSync(path.join(root, hotspotData), 'utf8'))) !== JSON.stringify(hotspot.data))
  throw Error('Hotspot timeline no longer matches its XML receipt');
const applicationManifest = JSON.parse(fs.readFileSync(path.join(root, source.directory, 'asset-manifest.json'), 'utf8'));
const pulseCss = [...new Set(Object.values(applicationManifest.files))].filter(file => file.endsWith('.css')).map(file => {
  const relative = source.directory + '/' + file.replace(/^\.\//, '');
  const text = fs.readFileSync(path.join(root, relative), 'utf8');
  const rules = parseCSS(text).filter(rule => /\.pulse\b|gr-banner-content__button img/.test(rule.selector));
  if (rules.some(rule => /\.pulse\b/.test(rule.selector))) throw Error('New pulse CSS requires a fresh intrinsic-size audit');
  return {path: relative, sha256: hash(text), rules};
});
const receipt = {
  source_date: '2026-10-05', verification: 'Static source parsing and resource hash validation only. No application, build, tests, vendor JS or DLL executed.',
  offset_unit: 'JavaScript UTF-16 code units; end is exclusive.', contracts,
  css: {path: cssFile, sha256: hash(css), rules, expand: {offset: at, end, source: css.slice(at, end)}},
  locales, setup, imageBase, assets,
  hotspot: {path: hotspotPath, sha256: hash(fs.readFileSync(path.join(root, hotspotPath))),
    data: hotspotData, data_sha256: hash(fs.readFileSync(path.join(root, hotspotData))),
    recovery: hotspot.recovery, pulse_css: pulseCss},
  consumers: ['crates/razer-app-pages/src/gamer_room.rs', 'crates/razer-app-pages/src/gamer_room_devices.rs', 'crates/razer-app-pages/src/gamer_room_presentation.rs', 'crates/razer-app-pages/src/gamer_room_hotspot.rs'],
  boundaries: [
    'No source backdrop-filter primitive in the locked GPUI renderer: 5px hotspot and 30px marketing blur remain unresolved.',
    'Hotspot SVG was recovered using its current manifest content fingerprint and parsed as XML. This is not an independent live-download byte comparison.',
    'Only explicit Device.sub_devices observations populate inventory. Catalog membership, profile settings and device request timers never create IoT observations.',
    'No IoT transport is connected. DeviceCommand submits a request and leaves observed state unchanged.',
    'Numeric title ordering is retained; locale-aware non-ASCII Intl collation is not implemented.',
    'Native installedDevices/noAliveSignPage service metadata, srcSet selection and persistent group collapse state remain separate follow-ups.',
  ],
};
const output = path.join(root, 'docs/re/gamer-room-current-evidence.json');
if (process.argv.includes('--check')) {
  if (!fs.existsSync(output) || fs.readFileSync(output, 'utf8') !== JSON.stringify(receipt, null, 2) + '\n')
    throw Error('Gamer Room source receipt changed; inspect before refreshing');
} else fs.writeFileSync(output, JSON.stringify(receipt, null, 2) + '\n');
console.log(JSON.stringify({contracts: contracts.length, css_rules: rules.length, assets: assets.length, output: path.relative(root, output), check: process.argv.includes('--check')}));
