// Parse current vendor source as data; never evaluate downloaded JavaScript.
const fs = require('node:fs');
const path = require('node:path');
const acorn = require('acorn');
const { Source, walk, hash } = require('./webpack-source.cjs');
const { parseCSS } = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const source = new Source('synapse/macro');
const manifestPath = `${source.directory}/asset-manifest.json`;
const manifestText = fs.readFileSync(path.join(root, manifestPath), 'utf8');
const manifest = JSON.parse(manifestText);
const file = source.files.find(file => /\/main\.[a-f0-9]+\.js$/.test(file));
const text = source.text(file);
const scopes = [];
walk(acorn.parse(text, { ecmaVersion: 'latest' }), node => {
  if (!/Function/.test(node.type) || node.body?.type !== 'BlockStatement') return;
  const scope = new Map();
  for (const statement of node.body.body) {
    if (statement.type === 'VariableDeclaration') {
      for (const declaration of statement.declarations) {
        if (declaration.id.type === 'Identifier') scope.set(declaration.id.name, declaration.init);
      }
    } else if (statement.id?.name) scope.set(statement.id.name, statement);
  }
  if (['an', 'un', 'dn', 'Ct', 'Ys', 'Zs'].every(name => scope.has(name))) scopes.push(scope);
});
if (scopes.length !== 1) throw Error('Ambiguous current Macro business closure');
const scope = scopes[0];
const bindings = Object.fromEntries(['Mn', 'Bt', 'an', 'dn', 'Gr', 'zn', 'kt', 'Ut', 'Rt', 'Ct', 'Zs'].map(name => {
  const node = scope.get(name);
  if (!node) throw Error(`Missing current Macro business binding ${name}`);
  return [name, { file, sha256: hash(text), offset: node.start, end: node.end, source: text.slice(node.start, node.end) }];
}));
const reducers = Object.fromEntries(['_G', 'i$', 'iV'].map(name => {
  let node = source.exported(25572, name);
  if (node.type === 'Identifier') node = source.binding(25572, node.name);
  return [`25572.${name}`, source.receipt(25572, node)];
}));
const actions = Object.fromEntries(['hJ', 'KK', 'O$', 'K3', 'XE', 'Mh', 'bB', 'fR', 'Yt', 'hc'].map(name => {
  let node = source.exported(4173, name);
  if (node.type === 'Identifier') node = source.binding(4173, node.name);
  return [`4173.${name}`, source.receipt(4173, node)];
}));
const css = Object.values(manifest.files)
  .filter(file => /^\.\/static\/css\/(main|8190)\..*\.css$/.test(file))
  .map(file => {
    const name = `${source.directory}/${file.slice(2)}`;
    const value = fs.readFileSync(path.join(root, name), 'utf8');
    return {
      file: name,
      sha256: hash(value),
      rules: parseCSS(value).filter(rule => /profile-bar-disabled|save-alert|save-close|profile-del|nav-wrapper|\.navbar|\.module-nav|\.rename-rect|\.navFolder/.test(rule.selector)),
    };
  });
const result = {
  route: 'synapse/macro',
  method: 'Current manifest, scoped Acorn commands/reducers and static CSS; no native/runtime/visual certification',
  manifest: { path: manifestPath, sha256: hash(manifestText) },
  bindings,
  reducers,
  actions,
  css,
};
const target = path.join(root, 'docs/re/macro-command-flow-current-evidence.json');
if (process.argv.includes('--check')) {
  if (JSON.stringify(JSON.parse(fs.readFileSync(target, 'utf8'))) !== JSON.stringify(result)) {
    throw Error('Stale current Macro command-flow evidence');
  }
} else fs.writeFileSync(target, JSON.stringify(result, null, 2) + '\n');
console.log(`Current Macro command flow: ${Object.keys(bindings).length} business bindings, ${Object.keys(reducers).length} reducers, ${Object.keys(actions).length} actions and CSS verified`);
