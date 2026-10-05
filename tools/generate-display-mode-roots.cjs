// Derive the product ids that carry a root-level `displayMode` branch.
//
// Input is the audited static scan (docs/re/display-mode-audit.json), which was
// produced from the current product bundles by tools/audit-display-modes.cjs.
// No downloaded JavaScript is executed here: this only reshapes an existing
// receipt into a table the Rust side can include.
//
//   node tools/generate-display-mode-roots.cjs           # write the table
//   node tools/generate-display-mode-roots.cjs --check   # fail if stale
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const root = path.resolve(__dirname, '..');
const auditPath = 'docs/re/display-mode-audit.json';
const outputPath = 'src/features/display-mode-roots.json';
const check = process.argv.includes('--check');

const bytes = fs.readFileSync(path.join(root, auditPath));
const audit = JSON.parse(bytes.toString('utf8'));

const modes = {};
for (const product of audit.products) {
  for (const mode of product.modes) {
    if (mode.kind !== 'root') continue;
    (modes[mode.mode] ??= []).push(product.product_id);
  }
}
for (const key of Object.keys(modes)) modes[key] = [...new Set(modes[key])].sort((a, b) => a - b);

const table = {
  schema_version: 1,
  source: auditPath,
  source_sha256: crypto.createHash('sha256').update(bytes).digest('hex'),
  generator_sha256: crypto.createHash('sha256').update(fs.readFileSync(__filename)).digest('hex'),
  method:
    'Root branches audited by tools/audit-display-modes.cjs: the product bundle selects this component ' +
    'in the same ternary chain that reads `searchParams.get("displayMode")`. Products are listed by id only; ' +
    'the branch content, its opener window and its services are recorded in docs/re/display-mode-audit.md.',
  modes,
};
const text = JSON.stringify(table, null, 2) + '\n';

if (check) {
  const current = fs.existsSync(path.join(root, outputPath))
    ? fs.readFileSync(path.join(root, outputPath), 'utf8')
    : '';
  if (current !== text) throw Error(`${outputPath} is stale; rerun tools/generate-display-mode-roots.cjs`);
  if (table.source_sha256 !== crypto.createHash('sha256').update(bytes).digest('hex')) {
    throw Error('display mode audit changed without regeneration');
  }
  console.log(`display-mode roots up to date: ${Object.entries(modes).map(([k, v]) => `${k}=${v.length}`).join(' ')}`);
} else {
  fs.writeFileSync(path.join(root, outputPath), text);
  console.log(`wrote ${outputPath}: ${Object.entries(modes).map(([k, v]) => `${k}=${v.length}`).join(' ')}`);
}
