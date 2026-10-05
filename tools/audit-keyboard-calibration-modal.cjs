// Recover the mounted 740/746 calibration component tree and CSS as data.
// Never load or evaluate vendor JavaScript. Offsets are UTF-16 JS positions.
const fs = require('fs'), path = require('path');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const records = [];
for (const pid of [740, 746]) {
  const directory = `.ref/devices/${pid}`;
  const source = Object.assign(Object.create(Source.prototype), {
    directory, modules: new Map(), texts: new Map(), parsed: new Set(),
    files: fs.readdirSync(path.join(root, directory, 'static/js'))
      .filter(f => f.endsWith('.js')).map(f => `${directory}/static/js/${f}`),
  });
  const candidates = source.files.filter(file => source.text(file).includes('KeyboardSwitchCalibrationModal_steps__'));
  if (candidates.length !== 1) throw Error(`Ambiguous calibration chunk ${pid}`);
  source.parse(candidates[0]);
  const scopes = [...source.modules.values()].filter(m => m.file === candidates[0] && m.definitions.has('Ea') && m.definitions.has('Sa'));
  if (scopes.length !== 1) throw Error(`Ambiguous calibration scope ${pid}`);
  const scope = scopes[0], id = scope.id;
  if (source.snippet(id, source.binding(id, 'Ca')) !== 'Sa') throw Error('Popup alias changed');
  const components = Object.fromEntries(['Aa', 'fa', '_a', 'Ea', 'Sa', 'ma'].map(name => [name, source.receipt(id, source.binding(id, name))]));
  const uses = [];
  walk(scope.fn.body, node => {
    if (node.type !== 'CallExpression' || !['jsx', 'jsxs'].includes(key(node.callee.property))) return;
    if (['Aa', 'fa', '_a', 'Ea', 'Ca'].includes(node.arguments[0]?.name)) uses.push(source.receipt(id, node));
  });
  // Record the root's GUID comparison; display names do not select this state.
  const factory = [];
  walk(scope.fn.body, node => {
    if (node.type === 'BinaryExpression' && node.operator === '===' &&
        node.left?.type === 'Literal' && typeof node.left.value === 'string' &&
        key(node.right?.property) === 'selectedProfileGuid') factory.push(source.receipt(id, node));
  });
  const factoryGuid = factory.find(r => r.source.includes('af06d371-861f-4b98-8d78-bfaef5cfdebf'));
  if (!factoryGuid) throw Error('Factory GUID contract changed');
  const translations = {};
  const namespace = source.binding(id, 'g').arguments[0].value;
  for (const symbol of ['mzS', 'ebo']) translations[symbol] = source.literal(namespace, source.exported(namespace, symbol));
  const shared = {};
  for (const name of ['ye', 'Me', 'De']) {
    const imported = source.binding(id, name);
    const target = imported.arguments?.[0]?.value;
    if (!Number.isInteger(target)) throw Error(`Unresolved import ${name}`);
    shared[name] = {import: source.receipt(id, imported), export: source.receipt(target, source.exported(target, 'A'))};
    const exported = source.exported(target, 'A');
    shared[name].definitions = [];
    let node = exported;
    const seen = new Set();
    while (node.type === 'Identifier' && !seen.has(node.name)) {
      seen.add(node.name);
      node = source.binding(target, node.name);
      shared[name].definitions.push(source.receipt(target, node));
    }
  }
  const cssDir = `${directory}/static/css`;
  const chunkId = path.basename(candidates[0]).split('.')[0];
  const css = fs.readdirSync(path.join(root, cssDir)).filter(f => f.endsWith('.css') && (f.startsWith('main.') || f.startsWith(`${chunkId}.`)))
    .sort((a, b) => Number(!a.startsWith('main.')) - Number(!b.startsWith('main.')))
    .map(file => {
      const name = `${cssDir}/${file}`, text = read(name);
      const rules = parseCSS(text).filter(rule => /KeyboardSwitchCalibrationModal|KeyboardSwitchCalibrationV2_(?:btn|green|calibration-content|calibrationInfo)|choose-a-mat|^\.backdrop|factory-default|warning-alert|^\.disabled\b|^\.body-widgets\b|introduction-calibration-container/.test(rule.selector));
      const keyframes = [...text.matchAll(/@keyframes KeyboardSwitchCalibrationModal_[^{]+\{/g)].map(match => {
        let end = match.index + match[0].length, depth = 1;
        while (depth && end < text.length) { if (text[end] === '{') depth++; if (text[end] === '}') depth--; end++; }
        if (depth) throw Error('Unclosed keyframes');
        return {offset: match.index, end, source: text.slice(match.index, end)};
      });
      return {path: name, sha256: hash(text), rules, keyframes};
    }).filter(item => item.rules.length || item.keyframes.length);
  records.push({product_id: pid, module: id, components, uses, shared, factory_guid: factoryGuid, translations, css});
}
const file = 'docs/re/keyboard-calibration-modal-current-evidence.json';
const output = JSON.stringify({schema_version: 1, products: records}, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (read(file) !== output) throw Error('Calibration modal evidence drifted');
} else fs.writeFileSync(path.join(root, file), output);
console.log('740/746 calibration: recovered current scoped JSX, popup alias, GUID condition, shared wrappers, CSS and complete keyframes. No vendor code executed.');
