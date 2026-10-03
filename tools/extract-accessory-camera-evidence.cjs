// Source is parsed as Acorn syntax and literal data; never evaluated.
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const { inspect } = require('./extract-audio-evidence.cjs');
const root = path.resolve(__dirname, '..');
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const family = process.argv.includes('--camera') ? 'camera' : 'accessory';
const source = JSON.parse(fs.readFileSync(path.join(root, 'docs/re/source-product-configs.json'), 'utf8'));
const candidates = source.products.filter(p => family === 'camera' ? p.family === 'camera' : ['other', 'iot_hue'].includes(p.family));
const products = candidates.filter(p => p.config).map(p => { console.log(p.product_id); return inspect(p); });
fs.writeFileSync(path.join(root, `docs/re/${family}-product-evidence.json`), JSON.stringify({
  schema_version: 1,
  scanner_sha256: hash(fs.readFileSync(__filename)),
  parser_sha256: hash(fs.readFileSync(path.join(__dirname, 'extract-audio-evidence.cjs'))),
  unresolved_configs: candidates.filter(p => !p.config).map(p => p.product_id),
  products,
}, null, 2) + '\n');
