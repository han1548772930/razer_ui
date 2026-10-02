// Parse current Dashboard exports as data. Never load or evaluate a bundle.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const folder = '.ref/applications/synapse/dashboard';
const check = process.argv.includes('--check');
const read = name => fs.readFileSync(path.join(root, name), 'utf8');
const hash = text => crypto.createHash('sha256').update(text).digest('hex');
const manifestText = read(`${folder}/asset-manifest.json`);
const manifest = JSON.parse(manifestText);
const sources = [...new Set(Object.values(manifest.files))]
  .filter(name => /\/trans-[^.]+\.[a-f0-9]+\.chunk\.js$/.test(name)).sort();
if (sources.length !== 10) throw Error(`Expected 10 locale bundles, found ${sources.length}`);
function walk(node, visit, ancestors = []) {
  if (!node || typeof node !== 'object') return;
  if (node.type) visit(node, ancestors);
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) value.forEach(child => walk(child, visit, [...ancestors, node]));
    else if (value && typeof value === 'object') walk(value, visit, [...ancestors, node]);
  }
}
const records = [];
const outputs = [];
for (const file of sources) {
  const locale = path.basename(file).match(/^trans-(.*?)\./)[1];
  const source = `${folder}/${file.replace(/^\.\//, '')}`;
  const text = read(source);
  const ast = acorn.parse(text, {ecmaVersion: 'latest'});
  let exports, body;
  walk(ast, (node, ancestors) => {
    if (node.type !== 'CallExpression' || node.callee.type !== 'MemberExpression'
        || node.callee.property.name !== 'd') return;
    const object = node.arguments[1];
    if (object?.type !== 'ObjectExpression' || object.properties.length < 1000) return;
    if (exports) throw Error(`Ambiguous translation exports: ${source}`);
    exports = object;
    body = ancestors.findLast(parent => /FunctionExpression$/.test(parent.type))?.body;
  });
  if (!exports || body?.type !== 'BlockStatement') throw Error(`Missing translations: ${source}`);
  const values = new Map();
  for (const statement of body.body) {
    if (statement.type !== 'VariableDeclaration') continue;
    for (const declaration of statement.declarations) {
      if (declaration.id.type === 'Identifier' && declaration.init) {
        if (values.has(declaration.id.name)) throw Error(`Duplicate binding in ${source}`);
        values.set(declaration.id.name, declaration.init);
      }
    }
  }
  function stringValue(name, seen = new Set()) {
    if (seen.has(name)) throw Error(`Cyclic translation binding: ${source}:${name}`);
    seen.add(name);
    const value = values.get(name);
    if (value?.type === 'Literal' && typeof value.value === 'string') return value.value;
    if (value?.type === 'Identifier') return stringValue(value.name, seen);
    throw Error(`Nonliteral translation binding: ${source}:${name}`);
  }
  const translations = {};
  for (const property of exports.properties) {
    const key = property.key.name ?? property.key.value;
    if (key === 'default') continue;
    if (property.computed || property.value.type !== 'ArrowFunctionExpression'
        || property.value.params.length || property.value.body.type !== 'Identifier'
        || !values.has(property.value.body.name)) throw Error(`Nonliteral translation: ${source}:${key}`);
    translations[key] = stringValue(property.value.body.name);
  }
  const target = `locales/${locale}.json`;
  const existing = JSON.parse(read(target));
  const changes = Object.keys(translations).filter(key => existing[key] !== translations[key]);
  if (check && changes.length) throw Error(`Stale ${target}: ${changes.join(', ')}`);
  if (!check) {
    // Keep device/settings-only keys and scoped Alexa copy, refresh current Dashboard keys.
    outputs.push([target, JSON.stringify({...existing, ...translations}, null, 2) + '\n']);
  }
  records.push({locale, source, source_sha256: hash(text), keys: Object.keys(translations).length});
  console.log(`${locale}: ${Object.keys(translations).length} static exports; ${changes.length} changed`);
}
const receipt = {
  manifest: `${folder}/asset-manifest.json`, manifest_sha256: hash(manifestText),
  method: 'Acorn AST: module-local string literals, aliases and explicit export getters; no execution',
  locales: records,
};
const receiptPath = 'docs/re/dashboard-locale-source.json';
if (check && JSON.stringify(JSON.parse(read(receiptPath))) !== JSON.stringify(receipt)) {
  throw Error('Stale Dashboard locale source receipt');
}
for (const [target, content] of outputs) fs.writeFileSync(path.join(root, target), content);
if (!check) fs.writeFileSync(path.join(root, receiptPath), JSON.stringify(receipt, null, 2) + '\n');
