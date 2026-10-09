// Current 226 ordinary Customize scroll wheel. AST/CSS only, no vendor execution.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {Source, walk, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), check = process.argv.includes('--check');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
function output(file, value) {
  if (check) { if (read(file) !== value) throw Error('Stale ' + file); }
  else fs.writeFileSync(path.join(root, file), value);
}
const directory = '.ref/devices/226', manifestPath = directory + '/asset-manifest.json';
const manifest = JSON.parse(read(manifestPath));
const source = Object.assign(Object.create(Source.prototype), {directory,
  files: [...new Set(Object.values(manifest.files))].filter(f => f.endsWith('.js')).map(f => directory + '/' + f.slice(2)),
  modules: new Map(), texts: new Map(), parsed: new Set()});
const receipts = [];
for (const name of ['fe', 'Se', 'ge', 'Ne', 'de', '_e', 'Ee', 'pe', 'ue', 'Ke', 'qt', 'es', 'Es', 'ps', 'us', 'Ks']) {
  receipts.push({symbol: name, ...source.receipt(42, source.binding(42, name))});
}
for (const module of [130, 527, 9686, 7837, 9769, 6079]) {
  receipts.push({symbol: 'module:' + module, ...source.receipt(module, source.module(module).fn)});
}
const mounted = receipts.find(r => r.symbol === 'fe').source;
for (const snippet of ['s.length>=2&&!s.includes(t)', 'const n=[...o,i]', 's(t[0].id),e.setScrollMode(t[0].id)', 'forceDisabled:r', 'setScrollMode(t)},300)']) {
  if (!mounted.includes(snippet)) throw Error('Re-audit fe: ' + snippet);
}
const profileDefaults = [];
for (const relative of [manifest.files['main.js'], Object.values(manifest.files).find(f => /\/8355\..*\.js$/.test(f))]) {
  const file = directory + '/' + relative.slice(2), text = read(file);
  walk(acorn.parse(text, {ecmaVersion: 'latest'}), n => {
    let symbol;
    if (n.type === 'VariableDeclarator' && ['Gt', 'bt'].includes(n.id?.name) && n.start > 200000 && n.start < 215000) symbol = n.id.name;
    if (n.type === 'AssignmentExpression' && n.left?.property?.name === 'loadScrollWheelSettings') symbol = 'loadScrollWheelSettings';
    if (n.type === 'Property' && n.key?.name === 'scrollWheel' && n.value?.type === 'ObjectExpression'
        && n.value.properties.some(p => p.key?.name === 'scrollMode')) {
      const value = source.literal(42, n.value);
      if (value.scrollMode === 'FreeSpin' && Array.isArray(value.disabledModes)) {
        symbol = 'DEFAULTPROFILE.scrollWheel'; profileDefaults.push(value);
      }
    }
    if (symbol) receipts.push({symbol, path: file, sha256: hash(text), offset: n.start, end: n.end, source: text.slice(n.start, n.end)});
  });
}
if (profileDefaults.length !== 1) throw Error('Ambiguous profile default: ' + profileDefaults.length);
for (const name of ['Gt', 'bt', 'loadScrollWheelSettings']) if (receipts.filter(r => r.symbol === name).length !== 1) throw Error('Missing ' + name);
const modes = source.literal(42, source.binding(42, 'ge'));
const levelControls = [];
walk(source.binding(42, 'Se'), node => {
  if (node.type !== 'ObjectExpression') return;
  const properties = new Map(node.properties.map(p => [p.key?.name, p.value]));
  if (['min', 'max', 'step', 'minTag', 'maxTag'].every(key => properties.has(key))) {
    levelControls.push(Object.fromEntries(['min', 'max', 'step'].map(key => [key, source.literal(42, properties.get(key))])));
  }
});
if (levelControls.length !== 1) throw Error('Ambiguous scroll level control');
const limits = [];
walk(source.binding(42, 'fe'), node => {
  if (node.type === 'BinaryExpression' && node.operator === '>=' &&
      node.left.property?.name === 'length' && node.right.type === 'Literal') limits.push(node.right.value);
});
if (limits.length !== 2 || limits[0] !== limits[1] || limits[0] >= modes.length) throw Error('Changed disabled-mode limit');
const wheelSpec = {
  product_id: Number(directory.split('/').at(-1)), profile_key: 'scrollWheel',
  defaults: profileDefaults[0], modes,
  level_min: levelControls[0].min, level_max: levelControls[0].max, level_step: levelControls[0].step,
  max_disabled_modes: limits[0], locking_mode: source.literal(42, source.binding(42, 'Ne')),
};
const labels = {};
for (const symbol of ['wtv','dC4','KnW','X5L','EJu','NYz','MhB','b_8','X3g','a3B','bYt','$LP','E_p','RYm']) {
  labels[symbol] = source.literal(4693, source.exported(4693, symbol));
  receipts.push({symbol: 'label:' + symbol, ...source.receipt(4693, source.binding(4693, source.exported(4693, symbol).name))});
}
const actions = {};
for (const symbol of ['g7A','lQo','OZG','Rzo','coV','BF_','CXk','RHj','u7X','tdg','Jgj','lcf']) {
  actions[symbol] = source.literal(3254, source.exported(3254, symbol));
  receipts.push({symbol: 'action:' + symbol, ...source.receipt(3254, source.binding(3254, source.exported(3254, symbol).name))});
}
const css = [];
for (const relative of [...new Set(Object.values(manifest.files))].filter(f => f.endsWith('.css'))) {
  const file = directory + '/' + relative.slice(2), text = read(file);
  const rules = parseCSS(text).filter(r => /swtm|^\.slider(?:[, .:]|$)|^\.slider-container|^\.check-item|^\.check-box|^\.check-text|^\.switch|^\.mt10$|^\.mb20$/.test(r.selector));
  if (rules.length) css.push({path: file, sha256: hash(text), rules});
}
const otherSpecs = JSON.parse(read('crates/razer-pages/src/features/mouse_scroll_wheel_data.json')).filter(spec => spec.product_id !== wheelSpec.product_id);
output('crates/razer-pages/src/features/mouse_scroll_wheel_data.json', JSON.stringify([...otherSpecs, wheelSpec].sort((a,b) => a.product_id - b.product_id), null, 2) + '\n');
output('docs/re/mouse-226-scroll-current-evidence.json', JSON.stringify({method: 'Static AST and CSS only; UTF-16 offsets', product_id: 226,
  manifest: {path: manifestPath, sha256: hash(read(manifestPath))}, receipts, labels, actions, css}, null, 2) + '\n');
console.log(`226 scroll: ${receipts.length} AST receipts, ${css.reduce((n,c)=>n+c.rules.length,0)} CSS rules, ${modes.length} modes`);
