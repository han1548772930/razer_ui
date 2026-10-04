// Extract the current Feedback application's dedicated locale modules.
// This reads Webpack AST literals and export getters only; it never executes
// the downloaded application bundle.
const fs = require('node:fs');
const path = require('node:path');
const {Source, hash, walk, key} = require('./webpack-source.cjs');

const root = path.resolve(__dirname, '..');
const source = new Source('feedback');
const locales = {
  en: 7693,
  es: 3208,
  de: 5816,
  fr: 4364,
  ja: 5814,
  kr: 8469,
  ru: 4415,
  'pt-BR': 3077,
  'zh-CN': 9064,
  'zh-TW': 2371,
};
const check = process.argv.includes('--check');
const receipts = [];
const loader = source.binding(3272, 't');
const actualModules = {};
for (const property of loader.properties) {
  const locale = property.computed ? source.literal(3272, property.key) : key(property.key);
  const moduleIds = [];
  walk(property.value, node => {
    if (node.type === 'CallExpression' && node.callee.type === 'MemberExpression'
        && node.callee.object.name === 'd' && key(node.callee.property) === 'bind'
        && node.arguments[0]?.name === 'd' && Number.isInteger(node.arguments[1]?.value)) {
      moduleIds.push(node.arguments[1].value);
    }
  });
  if (moduleIds.length !== 1) throw Error(`Unresolved Feedback locale loader: ${locale}`);
  actualModules[locale] = moduleIds[0];
}
if (JSON.stringify(actualModules) !== JSON.stringify(locales)) throw Error('Feedback locale loading map changed');

for (const [locale, module] of Object.entries(locales)) {
  const scope = source.module(module);
  const strings = {};
  for (const key of [...scope.exports.keys()].sort()) {
    const value = source.literal(module, scope.exports.get(key));
    if (typeof value !== 'string') throw Error(`Non-string Feedback translation: ${locale}:${key}`);
    strings[key] = value;
  }
  const target = path.join(root, 'locales', `${locale}.json`);
  const existing = JSON.parse(fs.readFileSync(target, 'utf8'));
  if (check) {
    if (JSON.stringify(existing.FEEDBACK_SOURCE) !== JSON.stringify(strings)) {
      throw Error(`Stale Feedback translations: ${locale}`);
    }
  } else {
    existing.FEEDBACK_SOURCE = strings;
    fs.writeFileSync(target, JSON.stringify(existing, null, 2) + '\n');
  }
  receipts.push({locale, module, file: scope.file, sha256: hash(source.text(scope.file)), keys: Object.keys(strings).length});
  console.log(`${locale}: ${Object.keys(strings).length} Feedback source strings`);
}

const receipt = {
  namespace: 'FEEDBACK_SOURCE',
  loader_module: 3272,
  constants_module: 4166,
  loader: source.receipt(3272, loader),
  method: 'Acorn module-local literals and explicit export getters; no execution',
  locales: receipts,
};
const receiptPath = path.join(root, 'docs/re/feedback-locale-source.json');
if (check) {
  if (JSON.stringify(JSON.parse(fs.readFileSync(receiptPath, 'utf8'))) !== JSON.stringify(receipt)) {
    throw Error('Stale Feedback locale receipt');
  }
} else {
  fs.writeFileSync(receiptPath, JSON.stringify(receipt, null, 2) + '\n');
}
