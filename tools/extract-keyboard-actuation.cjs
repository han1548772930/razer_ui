// Parse current webpack modules as data; never load or execute vendor code.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const hash = data => crypto.createHash('sha256').update(data).digest('hex');
const products = JSON.parse(read('crates/razer-pages/src/features/keyboard_products_data.json'));
const pages = JSON.parse(read('docs/re/keyboard-product-pages.json'));
const output = [];
function walk(n, visit) {
  if (!n?.type || visit(n) === false) return;
  for (const v of Object.values(n)) {
    if (Array.isArray(v)) { for (const c of v) walk(c, visit); }
    else if (v?.type) walk(v, visit);
  }
}
const key = n => n?.name ?? n?.value;
for (const product of products.filter(p => p.pages.includes('ACTUATION'))) {
  const page = pages.products.find(p => p.product_id === product.product_id).pages.find(p => p.key === 'ACTUATION');
  const source = read(page.path);
  if (hash(source) !== page.sha256) throw Error('Changed page source');
  const mounted = page.components.map(c => c.source).join('\n');
  if (!mounted.includes('onMakeActuationPointChange') || !mounted.includes('syncActuationKeys')) throw Error('Unaudited actuation page '+product.product_id);
  const file = product.source_files.find(f => /\/main\./.test(f.path));
  const main = read(file.path);
  if (hash(main) !== file.sha256) throw Error('Changed main source');
  const ast = acorn.parse(main, {ecmaVersion:'latest'});
  let excluded;
  walk(ast, node => {
    if (node.type !== 'Property' || !Number.isInteger(key(node.key)) || !/Function/.test(node.value.type)) return;
    const defs = new Map(), exports = new Map();
    walk(node.value.body, n => {
      if (n.type === 'VariableDeclarator' && n.id.type === 'Identifier') defs.set(n.id.name, n.init);
      if (n.type === 'CallExpression' && key(n.callee.property) === 'd' && n.arguments[1]?.type === 'ObjectExpression') {
        for (const p of n.arguments[1].properties) exports.set(key(p.key), p.value.body);
      }
      if (/Function|Class/.test(n.type)) return false;
    });
    if (exports.has('IOh') && exports.has('hYc')) {
      excluded = ['IOh','hYc'].flatMap(name => {
        const array = defs.get(exports.get(name).name);
        if (array?.type !== 'ArrayExpression' || array.elements.some(n => n.type !== 'Literal' || typeof n.value !== 'string')) throw Error('Nonliteral exclusion');
        return array.elements.map(n => n.value);
      });
    }
    return false;
  });
  if (!excluded) throw Error('Missing selection exclusions '+product.product_id);
  const info = product.config.DeviceInfo.analogSpecs.actuationInfo;
  for (const name of ['min','max','unit','defaultValue','minBreak']) if (typeof info[name] !== 'number') throw Error('Invalid bound');
  // Helpers shared by these current bundles retain this conversion contract.
  if (!/toSliderVal:\w+=>Math\.round\(\w+\/\w+\),toMappingVal:\w+=>\w+\*\w+/.test(main)) throw Error('Changed converter');
  excluded.push(...(product.config.OBM_PRESET_PROFILES_HOT_KEYS ?? []));
  output.push({product_id:product.product_id, info,
    excluded:[...new Set(excluded)],
    source:file.path, source_sha256:file.sha256, page:page.path, page_sha256:page.sha256,
    limitations:['Rapid Trigger, Snap Tap, live adjustment and source animations remain incomplete.']});
}
fs.writeFileSync(path.join(root,'crates/razer-pages/src/features/keyboard_actuation_data.json'), JSON.stringify(output,null,2)+'\n');
console.log(`Audited ${output.length} current actuation pages and per-product conversion bounds.`);
