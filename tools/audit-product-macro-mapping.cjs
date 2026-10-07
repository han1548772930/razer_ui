// Current product MapMacro data only. Never evaluate downloaded JavaScript.
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const {Source, hash, walk} = require('./webpack-source.cjs');
const acorn = require('acorn');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const check = process.argv.includes('--check');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
function productSource(pid) {
  const source = Object.create(Source.prototype);
  source.directory = `.ref/devices/${pid}`;
  const manifest = JSON.parse(read(`${source.directory}/asset-manifest.json`));
  source.files = [...new Set(Object.values(manifest.files))]
    .filter(file => /static\/js\/[^/]+\.js$/.test(file))
    .map(file => `${source.directory}/${file.replace(/^\.?\//, '')}`);
  source.modules = new Map();
  source.texts = new Map();
  source.parsed = new Set();
  return {source, manifest};
}
const specs = [
  {product_id: 182, component: 2508, playback: 9267, constants: 9937, info: 1057},
  {product_id: 653, component: 82508, playback: 29267, constants: 69937, info: 78193},
];
const products = [], evidence = [];
let playback;
for (const spec of specs) {
  const {source, manifest} = productSource(spec.product_id);
  const receipts = {};
  function exported(id, name) {
    let node = source.exported(id, name);
    const value = source.literal(id, node);
    while (node.type === 'Identifier') node = source.binding(id, node.name);
    receipts[`${id}/${name}`] = {...source.receipt(id, node), value};
    return value;
  }
  const sets = Object.fromEntries(Object.entries({standard: 'gJ', wheel: 'kL', sequence: 'cv', phased: 'kJ'})
    .map(([name, symbol]) => [name, exported(spec.playback, symbol)]));
  assert.deepEqual(sets.standard.map(item => item.id), ['Once', 'NTimes', 'ContinuousToggle', 'ContinuousHeld', 'Queue']);
  assert.deepEqual(sets.wheel.map(item => item.id), ['Once', 'NTimes', 'Queue']);
  assert.deepEqual(sets.sequence.map(item => item.id), ['Sequence']);
  assert.deepEqual(sets.phased.map(item => item.id), ['Phased']);
  if (playback) assert.deepEqual(sets, playback); else playback = sets;
  const info = source.literal(spec.info, source.exported(spec.info, 'DeviceInfo'));
  // These two actual products do not enable the extra single-profile branch.
  assert.equal(info.singleProfileDevice, undefined);
  const wheel_inputs = exported(spec.playback, 'cJ');
  const held_inputs = exported(spec.constants, 'LL4');
  const held_products = exported(spec.constants, 'pVC');
  const component = source.receipt(spec.component, source.binding(spec.component, 'P'));
  let sortNode = source.exported(3466, 'eh');
  while (sortNode.type === 'Identifier') sortNode = source.binding(3466, sortNode.name);
  const sortRank = [...source.module(3466).definitions.values()].find(node =>
    node.type === 'Literal' && node.value === '*!@_.()#^&%-=+01234567989abcdefghijklmnopqrstuvwxyz');
  assert(sortRank, 'Changed source name sort rank');
  const name_sort = {sort: source.receipt(3466, sortNode), rank: source.receipt(3466, sortRank),
    fallback: source.receipt(3466, source.binding(3466, 'r'))};
  const routes = [];
  for (const file of source.files) {
    const text = source.text(file);
    if (!text.includes('.e(8997)')) continue;
    walk(acorn.parse(text, {ecmaVersion: 'latest'}), node => {
      if (node.type !== 'ArrowFunctionExpression') return;
      const excerpt = text.slice(node.start, node.end);
      if (new RegExp(`^\\(\\)=>[a-zA-Z_$]+\\.e\\(8997\\)\\.then\\([a-zA-Z_$]+\\.bind\\([a-zA-Z_$]+,${spec.component}\\)\\)$`).test(excerpt)) {
        routes.push({path: file, sha256: hash(text), offset: node.start, end: node.end, source: excerpt});
      }
    });
  }
  assert.equal(routes.length, 1, 'Expected one actual lazy MapMacro route');
  for (const token of ['getPlayBackSet', 'd.cJ.includes(this.props.activeButton.inputID)', 'singleProfileDevice',
    'this.props.isGlobalShortcut', 'getMappingData', 'repeatCount:e===r.i_6?this.state.playbackTimes:2',
    'maxLength:2,maxValue:99,minValue:1', 'allowLiveUpdate:!0', 'macroSet:e,playBackSet:a',
    'this.props.isMacroInstalled', 'window.open("/synapse/macro/"']) {
    assert(component.source.includes(token), `Changed ${spec.product_id} component: ${token}`);
  }
  const css = [...new Set(Object.values(manifest.files))].filter(file => /static\/css\/[^/]+\.css$/.test(file))
    .map(file => `${source.directory}/${file.replace(/^\.?\//, '')}`)
    .map(file => ({path: file, sha256: hash(read(file)), rules: parseCSS(read(file))
      .filter(rule => /map-macro__playback-options|custom-keymapping-stepper/.test(rule.selector))}))
    .filter(file => file.rules.length);
  assert(css.length > 0, `Missing macro CSS ${spec.product_id}`);
  const inputAliases = spec.product_id === 182 ? {CycleUpSensitivityStages: 'DKM_SB_03'} : {};
  products.push({product_id: spec.product_id, wheel_inputs, no_held_inputs: held_products.includes(spec.product_id) ? held_inputs : [], input_aliases: inputAliases});
  evidence.push({product_id: spec.product_id,
    manifest: {path: `${source.directory}/asset-manifest.json`, sha256: hash(read(`${source.directory}/asset-manifest.json`))},
    component, routes, name_sort, exports: receipts, device_info: {path: source.module(spec.info).file, sha256: hash(source.text(source.module(spec.info).file)), single_profile_device: false}, css});
}
const output = {schema_version: 1, playback, products};
const receipt = {schema_version: 1, method: 'Manifest-selected module-local Acorn AST and CSS; no vendor code execution',
  products: evidence, local_identity: 'macro_id is a local MacroLibrary entry identity, never a native guid',
  source_limitations: ['Component update uses stale indices and can lose restricted playback sets; local adaptation preserves document identity and reapplies current input eligibility.',
    'The source AX$.SYNAPSE.SEQUENCE typo is not adopted; local MacroType chooses the independently verified final playback set.']};
for (const [file, data] of [['src/features/mapping_macro_data.json', output], ['docs/re/product-macro-mapping-current-evidence.json', receipt]]) {
  const bytes = JSON.stringify(data, null, 2) + '\n';
  if (check) assert.equal(read(file).replace(/\r\n/g, '\n'), bytes, `Stale ${file}`);
  else fs.writeFileSync(path.join(root, file), bytes);
}
console.log(`Product Macro mapping ${check ? 'checked' : 'prepared'}: ${products.length} products, 4 playback sets`);
