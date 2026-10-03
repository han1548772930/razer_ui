// Static AST/CSS receipts for mounted profile bars. Never evaluates source JS.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const read = file => fs.readFileSync(path.join(root, file));
const products = [];
for (const pid of [164, 241, 778, 784, 3871, 3884, 3886, 3946]) {
  const dir = `.ref/devices/${pid}`;
  const manifest = JSON.parse(read(`${dir}/asset-manifest.json`));
  const file = pid === 241 ? `${dir}/static/js/11.219fb515.chunk.js` : `${dir}/${manifest.files['main.js'].replace(/^\.\//, '')}`;
  const bytes = read(file), source = bytes.toString('utf8'), ast = acorn.parse(source, {ecmaVersion:'latest'});
  const receipts = [];
  const receipt = (kind, node) => receipts.push({kind, offset:node.start, end:node.end, source:source.slice(node.start,node.end)});
  function walk(node) {
    if (!node?.type) return;
    if (node.type === 'AssignmentExpression' && node.left.type === 'MemberExpression' && ['renderProfileBarIcon', 'renderProfileBar', 'enableSwitchProfile'].includes(node.left.property.name)) receipt(node.left.property.name, node);
    if (node.type === 'Property' && ['renderProfileBar', 'isEnableProfileBar'].includes(node.key.name)) receipt(node.key.name, node);
    if (node.type === 'CallExpression' && node.callee.type === 'MemberExpression' && node.callee.property.name === 'setProfileDropdownState') receipt('setProfileDropdownState', node);
    for (const value of Object.values(node)) {
      if (Array.isArray(value)) value.forEach(walk);
      else if (value?.type) walk(value);
    }
  }
  walk(ast);
  if (!receipts.some(r => r.kind === 'renderProfileBarIcon') || !receipts.some(r => r.kind === 'isEnableProfileBar')) throw Error(`Missing profile consumer/root: ${pid}`);
  const cssFile = `${dir}/${manifest.files['main.css'].replace(/^\.\//, '')}`;
  const cssBytes = read(cssFile), css = cssBytes.toString('utf8');
  const cssRules = [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
    .filter(m => /(?:profile-bar|profile-wrapper|rename-rect|\.loader(?:\.|\{|\s)|navs-wrapper)/.test(m[0]))
    .map(m => ({offset:m.index, source:m[0]}));
  products.push({product_id:pid, file, sha256:hash(bytes), receipts, css_file:cssFile, css_sha256:hash(cssBytes), css_rules:cssRules});
}
const result = {method:'Acorn UTF-16 node offsets and static CSS receipts; no vendor JavaScript execution', generator_sha256:hash(fs.readFileSync(__filename)), products};
const target = path.join(root,'docs/re/product-profile-bars-current-evidence.json');
const output = JSON.stringify(result,null,2)+'\n';
if (process.argv.includes('--check')) {
  if (fs.readFileSync(target,'utf8') !== output) throw Error('Stale profile bar evidence');
} else fs.writeFileSync(target,output);
console.log(`Verified ${products.length} current product profile-bar roots and consumers.`);
