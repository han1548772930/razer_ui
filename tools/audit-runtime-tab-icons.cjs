// Static DOM favicon selectors and mounted overrides in current application JS.
// Parse reference bytes as data only; never import or execute those scripts.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const routes = ['synapse/dashboard', 'synapse/macro', 'synapse/armory', 'synapse/alexa',
  'synapse/profiles', 'synapse/introduction-tour', 'feedback', 'chroma-app/dashboard'];
const documents = new Map(), applications = [];
function parse(file, text) {
  const ast = acorn.parse(text, {ecmaVersion: 'latest'}), parents = new Map();
  walk(ast, node => {
    for (const value of Object.values(node)) for (const child of Array.isArray(value) ? value : [value]) {
      if (child?.type) parents.set(child, node);
    }
  });
  const document = {ast, parents, text, file};
  documents.set(file, document);
  return document;
}
function receipt(document, node) {
  return {path: document.file, sha256: hash(document.text), offset: node.start, end: node.end,
    source: document.text.slice(node.start, node.end)};
}
function one(document, predicate) {
  const nodes = []; walk(document.ast, node => { if (predicate(node)) nodes.push(node); });
  if (nodes.length !== 1) throw Error(`Ambiguous current runtime contract: ${document.file}: ${nodes.length}`);
  return nodes[0];
}
for (const route of routes) {
  const directory = `.ref/applications/${route}`, manifestFile = `${directory}/asset-manifest.json`;
  const manifestText = read(manifestFile), manifest = JSON.parse(manifestText);
  const files = [...new Set(Object.values(manifest.files))].filter(file => /static\/js\/[^/]+\.js$/.test(file))
    .map(file => `${directory}/${file.slice(file.indexOf('static/js/'))}`).sort();
  const selectors = [], fingerprints = [];
  for (const file of files) {
    const text = read(file);
    fingerprints.push([file, hash(text)]);
    // Both conditions are necessary for a literal icon selector. This is not
    // a claim about computed selectors or reference runtime execution.
    if (!text.includes('querySelector') || !text.includes('icon')) continue;
    const document = parse(file, text);
    walk(document.ast, node => {
      if (node.type !== 'CallExpression' || !['querySelector', 'querySelectorAll'].includes(key(node.callee.property))) return;
      const selector = node.arguments[0]?.value;
      if (typeof selector !== 'string' || !selector.includes('link') || !selector.includes('icon')) return;
      let owner = document.parents.get(node);
      while (owner && !/Function/.test(owner.type)) owner = document.parents.get(owner);
      if (!owner) throw Error('Unscoped favicon mutation');
      const record = {selector, query_offset: node.start, ...receipt(document, owner)};
      record.behavior = record.source.includes('readAsDataURL') ? 'preserve-image-bytes-as-data-url' : 'runtime-icon-selection';
      selectors.push(record);
    });
  }
  applications.push({route, manifest: manifestFile, manifest_sha256: hash(manifestText),
    script_count: files.length, scripts_sha256: hash(JSON.stringify(fingerprints)), selectors});
}
const expectedCounts = [3, 2, 3, 0, 1, 1, 1, 2];
applications.forEach((app, i) => { if (app.selectors.length !== expectedCounts[i]) throw Error(`Changed favicon selectors: ${app.route}`); });
const macro = documents.get('.ref/applications/synapse/macro/static/js/main.3f4b9604.js');
const macroComponent = one(macro, n => n.type === 'VariableDeclarator' && n.id.name === 'xr'
  && macro.text.slice(n.start, n.end).includes('activeRecord'));
const macroMount = one(macro, n => n.type === 'CallExpression' && n.arguments[0]?.name === 'xr');
const macroAssets = ['Lr', 'Kr'].map(name => receipt(macro,
  one(macro, n => n.type === 'VariableDeclarator' && n.id.name === name
    && macro.text.slice(n.start, n.end).includes('static/media/'))));
if (!macroAssets[0].source.includes('macro.eba30c35.ico') || !macroAssets[1].source.includes('tabicon_recording.d215dc73.svg')) throw Error('Macro icon resources changed');
const dashboard = documents.get('.ref/applications/synapse/dashboard/static/js/7861.1b0e99a4.chunk.js');
const unusedRoot = one(dashboard, n => n.type === 'ClassDeclaration' && n.id.name === 'Ft'
  && dashboard.text.slice(n.start, n.end).includes('_restoreFavicon'));
let scope = dashboard.parents.get(unusedRoot);
while (scope && !/Function/.test(scope.type)) scope = dashboard.parents.get(scope);
const references = [];
walk(scope, n => { if (n.type === 'Identifier' && n.name === 'Ft') references.push(n); });
if (references.length !== 2 || references[0] !== unusedRoot.id) throw Error('Dashboard pairing root reference count changed');
const connect = dashboard.parents.get(references[1]);
if (connect.type !== 'CallExpression' || dashboard.parents.get(connect)?.type !== 'ExpressionStatement') throw Error('Dashboard pairing wrapper is now used');
const armory = new Source('synapse/armory');
const featureHooks = [77989, 60094].map(id => ({module: id, ...armory.receipt(id, armory.module(id).fn)}));
const result = {
  generator_sha256: hash(fs.readFileSync(__filename)),
  method: 'Current manifest JS literal DOM icon selectors; Acorn ownership, explicit mounted Macro component and unused Dashboard pairing wrapper; no reference execution',
  applications,
  macro: {component: receipt(macro, macroComponent), mount: receipt(macro, macroMount), assets: macroAssets,
    native_state: 'normal ICO; real activeRecord/recording transport is not connected'},
  dashboard_pairing_override: {class: receipt(dashboard, unusedRoot), unused_connect: receipt(dashboard, connect),
    conclusion: 'The Ft class is only declared and passed to a connect call whose result is discarded. It is not mounted by this module; do not apply its HyperPolling icon to the main Dashboard.'},
  armory_feature_hooks: featureHooks,
  final_icons: {dashboard: './synapse.svg (production)', macro: 'static/media/macro.eba30c35.ico (normal)',
    armory: '/synapse/assets/imgs/apps/logo_armory_workshop.svg (unobserved feature hook starts false)',
    armory_observed_exchange: '/synapse/assets/imgs/apps/logo_armory_exchange.svg',
    chroma: '/chroma-app/dashboard/icon.svg (production)', alexa: 'HTML favicon.ico', profiles: 'HTML favicon.svg',
    tour: 'HTML favicon.svg', feedback: 'HTML favicon.svg'},
  limitations: ['Static literal selector scan is not a browser event-order, cache, computed-selector or network test.',
    'Product HTML icon mapping is audited separately; product-specific alternate-root icon overrides are not covered by this application receipt.',
    'Armory feature/account transport and Macro recording are not fabricated; alternate observed states require their real service data.'],
};
const target = path.join(root, 'docs/re/runtime-tab-icons-current-evidence.json');
const output = JSON.stringify(result, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (fs.readFileSync(target, 'utf8') !== output) throw Error('Stale runtime favicon receipt');
} else fs.writeFileSync(target, output);
console.log(`Verified ${applications.length} application runtime favicon scopes and Macro mount; Dashboard unused pairing override excluded.`);
