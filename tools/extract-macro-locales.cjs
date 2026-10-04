// The current Macro language loader (18442) selects these specialized modules.
// Parse export getters and literal/re-export values; never execute source JS.
const fs = require('node:fs');
const path = require('node:path');
const {Source, hash} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const source = new Source('synapse/macro');
const locales = {de:81655, en:51971, es:84520, fr:93828, ja:61121, kr:70497,
  'pt-BR':81493, ru:83189, 'zh-CN':81250, 'zh-TW':67190};
const check = process.argv.includes('--check');
const receipts = [];
for (const [locale, module] of Object.entries(locales)) {
  const scope = source.module(module);
  const strings = {};
  for (const key of [...scope.exports.keys()].sort()) {
    const value = source.literal(module, source.exported(module, key));
    if (typeof value !== 'string') throw Error(`Non-string Macro translation: ${locale}:${key}`);
    strings[key] = value;
  }
  const target = path.join(root, 'locales', `${locale}.json`);
  const existing = JSON.parse(fs.readFileSync(target, 'utf8'));
  if (check) {
    if (JSON.stringify(existing.MACRO_SOURCE) !== JSON.stringify(strings)) {
      throw Error(`Stale Macro translations: ${locale}`);
    }
  } else {
    existing.MACRO_SOURCE = strings;
    fs.writeFileSync(target, JSON.stringify(existing, null, 2) + '\n');
  }
  receipts.push({locale, module, file:scope.file, sha256:hash(source.text(scope.file)), keys:Object.keys(strings).length});
  console.log(`${locale}: ${Object.keys(strings).length} Macro source strings`);
}
const receipt = {namespace:'MACRO_SOURCE', loader_module:18442,
  method:'Acorn module-local literals and explicit export getters, including common-language re-exports; no execution',
  locales:receipts};
const receiptPath = path.join(root, 'docs/re/macro-locale-source.json');
if (check) {
  if (JSON.stringify(JSON.parse(fs.readFileSync(receiptPath, 'utf8'))) !== JSON.stringify(receipt)) {
    throw Error('Stale Macro locale receipt');
  }
} else fs.writeFileSync(receiptPath, JSON.stringify(receipt, null, 2) + '\n');
