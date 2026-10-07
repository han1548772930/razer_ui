// Re-audit receiver/dock callbacks and paired-row geometry as static source data.
// Reference JavaScript is only parsed by Acorn, never imported or evaluated.
const fs = require('fs');
const path = require('path');
const { Source, hash } = require('./webpack-source.cjs');
const { parseCSS } = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const check = process.argv.includes('--check');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
function product(id) {
  const source = Object.create(Source.prototype);
  source.directory = `.ref/devices/${id}`;
  const manifestPath = source.directory + '/asset-manifest.json';
  const manifest = JSON.parse(read(manifestPath));
  source.files = [...new Set(Object.values(manifest.files))]
    .filter(file => /^\.\/static\/js\/[^/]+\.js$/.test(file))
    .map(file => source.directory + '/' + file.slice(2));
  source.modules = new Map();
  source.texts = new Map();
  source.parsed = new Set();
  return { source, manifest, manifestPath };
}
const definitions = [
  [164, 2715, ['Pe', 'Re', 'le']],
  [241, 3746, ['bs', 'Ps', 'Es', 'xs', 'we', 'hs', 'Cs', 'ds']],
  [179, 9473, ['mE', 'Te', 'OE', 'AE', 'G', 'se']],
];
const products = definitions.map(([id, module, names]) => {
  const { source, manifest, manifestPath } = product(id);
  const components = Object.fromEntries(names.map(name => [
    name, source.receipt(module, source.binding(module, name)),
  ]));
  if (id === 164 && !components.Re.source.includes('className:`${u}`,onClick:g')) {
    throw Error('164 paired-title callback changed');
  }
  if (id === 241) {
    if (!components.Es.source.includes('null==n||0===n||e.editionId===n')) {
      throw Error('241 edition matching changed');
    }
    if (!components.Ps.source.includes('children:[Z(x,r.kBv),Z(H,r.WQi)]') ||
        !components.Ps.source.includes('children:[q(x),q(H)]')) {
      throw Error('241 mouse/keyboard ordering changed');
    }
  }
  const css = [...new Set(Object.values(manifest.files))]
    .filter(file => /^\.\/static\/css\/[^/]+\.css$/.test(file))
    .map(file => {
      const cssPath = source.directory + '/' + file.slice(2);
      const text = read(cssPath);
      const rules = parseCSS(text).filter(rule => id === 164
        ? /HyperPollingWireless_(pairInfoBox|pairedDes|pairedTitle|pairedContent)/.test(rule.selector)
        : id === 241
          ? /HyperPollingWirelessMouseDock_(pairInfoBox|pairedContentGroup|pairedDes|pairedTitle|deviceList|deviceRow|unpairButton|deviceNameLink)|Duallink_pairInfoBox/.test(rule.selector)
          : /indicator-led-container|modal_(backDrop|modal|header)|HyperPollingWirelessUma_loading/.test(rule.selector));
      return { path: cssPath, sha256: hash(text), rules };
    }).filter(file => file.rules.length);
  return {
    product_id: id,
    manifest: { path: manifestPath, sha256: hash(read(manifestPath)) },
    components,
    css,
  };
});
const nativeFiles = [
  'src/features/dock_pairing.rs',
  'src/features/dock_pairing/state.rs',
  'src/features/dock_pairing/dialog.rs',
  'src/features/dock_pairing/observation.rs',
  'src/features/source_controls/receiver.rs',
];
const result = {
  method: 'Current manifest-declared AST callbacks and CSS; no vendor execution or UI execution',
  generator_sha256: hash(fs.readFileSync(__filename)),
  products,
  native: nativeFiles.map(file => ({ path: file, sha256: hash(read(file)) })),
};
const output = 'docs/re/receiver-interactions-current-evidence.json';
const serialized = JSON.stringify(result, null, 2) + '\n';
if (check) {
  if (read(output) !== serialized) throw Error('Stale ' + output);
} else {
  fs.writeFileSync(path.join(root, output), serialized);
}
console.log(`Receiver interactions: ${products.reduce((n, p) => n + Object.keys(p.components).length, 0)} AST receipts; ${products.reduce((n, p) => n + p.css.reduce((m, f) => m + f.rules.length, 0), 0)} CSS rules.`);
