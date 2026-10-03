// Audit remaining actual navigation pages; source is only parsed as Acorn data.
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const {inspect} = require('./source-help-ast.cjs');
const root = path.resolve(__dirname, '..');
const read = file => JSON.parse(fs.readFileSync(path.join(root,file),'utf8'));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const catalog = read('docs/re/unimplemented-products.json').products;
const selected = process.argv.slice(2).filter(x=>/^\d+$/.test(x)).map(Number);
const pending = read('docs/re/native-product-coverage.json').products
  .filter(p=>(!selected.length || selected.includes(p.product_id)) && p.pages.some(page=>page.status==='pending'));
for (const record of pending) {
  const product = catalog.find(p=>p.product_id===record.product_id);
  const keys = record.pages.filter(p=>p.status==='pending').map(p=>p.key);
  const nav = product.navigation.find(n=>n.items.some(i=>keys.includes(i.name?.value)));
  const evidence = inspect({...product,config:{path:nav.source,sha256:nav.sha256}},keys);
  fs.writeFileSync(path.join(root,`docs/re/pending-product-${product.product_id}-source.json`),JSON.stringify({
    method:'Acorn lexical resolution; vendor code not executed',
    generator_sha256:hash(fs.readFileSync(__filename)),
    resolver_sha256:hash(fs.readFileSync(path.join(__dirname,'source-help-ast.cjs'))),
    ...evidence,
  },null,2)+'\n');
  console.log(`Resolved pending product ${product.product_id}: ${keys.join(', ')}`);
}
