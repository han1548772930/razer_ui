// Parse current Macro event selection/toolbar contracts. Never execute source JS.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), s = new Source('synapse/macro');
const contracts = ['br', 'va', 'ha', 'ke', 'be'].map(name => ({
  module: 58190, name, ...s.receipt(58190, s.binding(58190, name)),
}));
contracts.push({module: 25572, name: 'M', ...s.receipt(25572, s.binding(25572, 'M'))});
const main = s.files.find(f => /\/main\.[^/]+\.js$/.test(f)), mainText = s.text(main);
const reducers = [];
walk(acorn.parse(mainText, {ecmaVersion: 'latest'}), n => {
  if (n.type !== 'SwitchCase' || n.test?.type !== 'MemberExpression'
      || n.test.object.name !== 'h' || !['tn', 'wL', 'mk', 'V7', 'FZ'].includes(key(n.test.property))) return;
  const text = mainText.slice(n.start, n.end);
  if (!text.includes('Ps(e)')) return;
  reducers.push({action: s.literal(4173, s.exported(4173, key(n.test.property))), path: main,
    sha256: hash(mainText), offset: n.start, end: n.end, source: text});
});
if (reducers.length !== 5) throw Error('Expected all five current selection reducers');
const manifest = JSON.parse(fs.readFileSync(path.join(root, s.directory, 'asset-manifest.json'), 'utf8'));
const css = Object.values(manifest.files).filter(f => /\/static\/css\/(main|8190)\..*\.css$/.test(f)).map(f => {
  const file = `${s.directory}/${f.replace(/^\.\//, '')}`, text = fs.readFileSync(path.join(root, file), 'utf8');
  return {path: file, sha256: hash(text), rules: parseCSS(text).filter(r =>
    /MacroItem_|\.check-box|\.check-item|ticktop|tickbottom/.test(r.selector))};
});
const nativePaths = ['src/shell/macro_page.rs', 'src/shell/macro_page/body.rs', 'src/shell/macro_page/selection.rs', 'src/shell/macro_page/row_actions.rs', 'src/shell/macro_page/row_view.rs'];
const native = nativePaths.map(file => ({path: file, sha256: hash(fs.readFileSync(path.join(root, file)))}));
const body = fs.readFileSync(path.join(root, nativePaths[1]), 'utf8');
const rowView = fs.readFileSync(path.join(root, nativePaths[4]), 'utf8');
const selection = fs.readFileSync(path.join(root, nativePaths[2]), 'utf8');
for (const token of ['macro-select-all', 'toggle_all_actions', 'macro-delete-selected-actions',
    'rgba(0x44d62c33)', 'selection::history_icon', 'action_pair_highlighted']) {
  if (!body.includes(token) && !rowView.includes(token)) throw Error('Missing native hookup: ' + token);
}
if (/macro-selected-actions-bar|child\("✓"\)|rgb\(0x44d62c33\)/.test(body)) throw Error('Obsolete selection UI remains');
if (!selection.includes('row_actions::pair_id(item) == Some(pair)') || !selection.includes('row_actions::event_state(item) == Some(opposite)')) {
  throw Error('Counterpart must use kind, identity and source highlight state');
}
const resourceManifest = JSON.parse(fs.readFileSync(path.join(root, 'assets/synapse/manifest.json'), 'utf8'));
const assetNames = ['delete', 'delete-hover', 'undo', 'undo-enable', 'undo-hover', 'redo', 'redo-enable', 'redo-hover'];
const embedded = fs.readFileSync(path.join(root, 'assets/synapse/embedded.rs'), 'utf8');
const assets = assetNames.map(name => {
  const file = `assets/synapse/macro/${name}.svg`;
  const entry = resourceManifest.entries.find(e => e.output === file);
  if (!entry || !entry.source.startsWith(s.directory + '/static/media/')) throw Error('Missing current asset: ' + file);
  const original = fs.readFileSync(path.join(root, entry.source)), output = fs.readFileSync(path.join(root, file));
  if (!original.equals(output) || hash(output) !== entry.sha256
      || !embedded.includes(`"synapse/macro/${name}.svg"`)) throw Error('Unverified asset: ' + file);
  return entry;
});
const result = {generator_sha256: hash(fs.readFileSync(__filename)), method: 'Acorn only; native hookup guards and asset hashes are static checks, not runtime verification',
  contracts, reducers, css, native, assets,
  remaining: ['Pairing lines', 'Source 200ms leading/trailing selection debounce',
    'Phased grouping and phase drag targets', 'Toolbar tooltips and runtime geometry/keyboard verification']};
const file = path.join(root, 'docs/re/macro-selection-current-evidence.json'), output = JSON.stringify(result, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (fs.readFileSync(file, 'utf8') !== output) throw Error('Stale Macro selection evidence');
} else fs.writeFileSync(file, output);
console.log(`Macro selection: ${contracts.length} scoped components, ${reducers.length} reducers, ${assets.length} exact assets; native checks passed.`);
