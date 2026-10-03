// Resolve mounted lazy module exports as AST data. Never execute vendor code.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const target = path.join(root, 'docs/re/keyboard-product-pages.json');
const receipt = JSON.parse(fs.readFileSync(target, 'utf8'));
const hash = source => crypto.createHash('sha256').update(source).digest('hex');
const key = node => node?.name ?? node?.value;
function walk(node, visit) {
  if (!node?.type || visit(node) === false) return;
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) { for (const child of value) walk(child, visit); }
    else if (value?.type) walk(value, visit);
  }
}
let count = 0;
for (const product of receipt.products) {
  for (const page of product.pages) {
    const id = /\.bind\(\w+,(\d+)\)/.exec(page.component)?.[1];
    if (!id) continue;
    const directory = path.join(root, `.ref/devices/${product.product_id}/static/js`);
    const matches = [];
    for (const file of fs.readdirSync(directory).filter(file => file.endsWith('.js'))) {
      const source = fs.readFileSync(path.join(directory, file), 'utf8');
      const head = new RegExp(`(?:^|[,{])${id}:`).exec(source);
      if (!head) continue;
      const start = head.index + head[0].length;
      const expression = acorn.parseExpressionAt(source, start, {ecmaVersion: 'latest'});
      const module = expression.type === 'SequenceExpression' ? expression.expressions[0] : expression;
      if (!/Function/.test(module.type)) continue;
      matches.push({source, module, file});
    }
    if (matches.length !== 1) throw Error(`Expected one current lazy module ${product.product_id}/${id}`);
    const {source, module, file} = matches[0], defs = new Map(), exports = new Map();
    walk(module.body, node => {
      if (node.type === 'ClassDeclaration' || node.type === 'FunctionDeclaration') { defs.set(node.id.name, node); return false; }
      if (node.type === 'VariableDeclarator' && node.id.type === 'Identifier') defs.set(node.id.name, node.init);
      if (node !== module.body && /Function/.test(node.type)) return false;
      if (node.type === 'CallExpression' && key(node.callee.property) === 'd' && node.arguments[1]?.type === 'ObjectExpression') {
        for (const property of node.arguments[1].properties) exports.set(key(property.key), property.value.body);
      }
    });
    const seen = new Set(), components = [];
    function trace(node) {
      if (!node) return;
      if (node.type === 'Identifier') {
        if (seen.has(node.name)) return;
        seen.add(node.name);
        const definition = defs.get(node.name);
        if (!definition) return;
        components.push({symbol:node.name, offset:definition.start, end:definition.end, source:source.slice(definition.start, definition.end), jsx:[]});
        trace(definition);
      } else {
        if (node.type === 'CallExpression') for (const argument of node.arguments) if (['Identifier','CallExpression'].includes(argument.type)) trace(argument);
        walk(node, child => {
          if (child.type === 'CallExpression' && child.callee.type === 'SequenceExpression' && ['jsx','jsxs'].includes(key(child.callee.expressions.at(-1)?.property))) trace(child.arguments[0]);
        });
      }
    }
    trace(exports.get('default'));
    if (!components.length) throw Error(`Unresolved default component ${product.product_id}/${id}`);
    page.nav_path = page.nav_path ?? page.path;
    page.nav_sha256 = page.nav_sha256 ?? page.sha256;
    page.path = `.ref/devices/${product.product_id}/static/js/${file}`;
    page.sha256 = hash(source);
    page.lazy_module = Number(id);
    page.components = components;
    count++;
  }
}
receipt.lazy_scanner_sha256 = hash(fs.readFileSync(__filename));
fs.writeFileSync(target, JSON.stringify(receipt, null, 2) + '\n');
console.log(`Resolved ${count} current keyboard lazy page modules.`);
