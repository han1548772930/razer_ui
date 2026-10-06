// Current layer operations and naming, parsed only; never evaluate vendor JS.
const fs = require('fs'), path = require('path');
const {source} = require('./audit-chroma-studio.cjs');
const receipts = [];
for (const [module, symbols] of [[9286, ['T','Y','w','I','G','O','b','A','z','V']], [2904,['f']], [4264,['J','Z']],[5303,['m']]]) {
  for (const symbol of symbols) receipts.push({module, symbol, ...source.receipt(module, source.binding(module, symbol))});
}
const data = JSON.stringify({method:'Current Studio static AST, UTF16 offsets and source-file SHA256; no vendor execution.', receipts}, null, 2) + '\n';
const target = path.resolve(__dirname, '../docs/re/chroma-studio-layers-source.json');
if (process.argv.includes('--check')) {
  if (fs.readFileSync(target, 'utf8') !== data) throw Error('Stale layer operation source receipts');
} else fs.writeFileSync(target, data);
console.log(`Studio layers: ${receipts.length} current AST receipts.`);
