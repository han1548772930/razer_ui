// Current 226 Performance integer editor: AST/CSS/asset parsing only.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {Source, walk, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), directory = '.ref/devices/226';
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const manifestPath = directory + '/asset-manifest.json', manifest = JSON.parse(read(manifestPath));
const source = Object.assign(Object.create(Source.prototype), {directory,
  files: [...new Set(Object.values(manifest.files))].filter(f => f.endsWith('.js')).map(f => directory + '/' + f.slice(2)),
  modules: new Map(), texts: new Map(), parsed: new Set()});
// Performance loads 4230 through 8901; the manifest also contains a MapKeyboard
// copy. Pin the mounted source instead of accepting the first module-id match.
const numberPath = source.files.find(file => /\/8901\.[^/]+\.js$/.test(file));
if (!numberPath) throw Error('Missing current Performance number chunk');
source.parse(numberPath);
if (source.module(4230).file !== numberPath) throw Error('Wrong numeric editor source');
const pages = JSON.parse(read('docs/re/mouse-page-source.json')).products.find(p => p.product_id === 226).pages;
const page = pages.find(p => p.key === 'TAB_PERFORMANCE');
const text = read(page.path);
if (hash(text) !== page.sha256) throw Error('Re-audit current Performance source');
if (!text.slice(page.nav_offset, page.nav_offset + 100).includes(page.component)) throw Error('Stale navigation mount');
const symbols = ['es', 'ts', 'ls', 'cs', 'Ms', 'ys', 'wi', 'Ri'];
const receipts = symbols.map(symbol => {
  const found = page.components.find(c => c.symbol === symbol);
  if (!found || text.slice(found.offset, found.end) !== found.source) throw Error('Stale mounted receipt: ' + symbol);
  return {symbol, path: page.path, sha256: hash(text), offset: found.offset, end: found.end, source: found.source};
});
for (const module of [4230, 1057]) receipts.push({symbol: 'module:' + module, ...source.receipt(module, source.module(module).fn)});
walk(acorn.parse(text, {ecmaVersion: 'latest'}), node => {
  if (node.type === 'VariableDeclarator' && ['Yt', 'zt', 'Xt', 'Jt', 'Kt'].includes(node.id?.name)
      && node.start > 139000 && node.start < 140000) {
    receipts.push({symbol: node.id.name, path: page.path, sha256: hash(text), offset: node.start, end: node.end, source: text.slice(node.start, node.end)});
  }
});
for (const name of ['Yt', 'zt', 'Xt', 'Jt', 'Kt']) if (receipts.filter(r => r.symbol === name).length !== 1) throw Error('Missing parameter ' + name);
const mainPath = directory + '/' + manifest.files['main.js'].slice(2), mainText = read(mainPath);
walk(acorn.parse(mainText, {ecmaVersion: 'latest'}), node => {
  if (node.type !== 'Property' || (node.key?.name ?? node.key?.value) !== 'default') return;
  const snippet = mainText.slice(node.start, node.end);
  if (snippet.includes('n.e(8901),n.e(8355)') && snippet.includes('n.bind(n,4125)')) {
    receipts.push({symbol: 'default-root-loader', path: mainPath, sha256: hash(mainText), offset: node.start, end: node.end, source: snippet});
  }
});
if (receipts.filter(r => r.symbol === 'default-root-loader').length !== 1) throw Error('Re-audit ordinary root chunk loading');
const stepper = receipts.find(r => r.symbol === 'module:4230').source;
for (const value of ['maxLength:6,maxValue:zt,minValue:Xt,allowDecimals:!1,stepValue:Kt']) {
  if (!receipts.find(r => r.symbol === 'es').source.includes(value)) throw Error('Re-audit numeric parameters');
}
for (const value of ['this.props.setParentState(e,this.isRegisterEvent)', 'Number.isNaN(e)&&(e=0)',
  'this.isKeyInput=!0,this.sendToParent(s)', '13!==e.keyCode&&27!==e.keyCode||this.inputDom.current.blur()',
  't+this.props.stepValue', '},300)']) if (!stepper.includes(value)) throw Error('Re-audit stepper behavior: ' + value);
const css = [];
for (const relative of [...new Set(Object.values(manifest.files))].filter(f => f.endsWith('.css'))) {
  const file = directory + '/' + relative.slice(2), text = read(file);
  const rules = parseCSS(text).filter(r => /stepper|spinner/.test(r.selector));
  if (rules.length) css.push({path: file, sha256: hash(text), rules});
}
const assets = ['up', 'down'].map(direction => {
  const input = directory + '/' + manifest.files[`static/media/stepper_${direction}.svg`].slice(2);
  const output = `assets/synapse/stepper-${direction}.svg`;
  const bytes = fs.readFileSync(path.join(root, input));
  if (!bytes.equals(fs.readFileSync(path.join(root, output)))) throw Error('Shared arrow differs: ' + output);
  return {source: input, source_url: 'https://apps.razer.com/synapse/products/226/ui/' + input.slice(directory.length + 1), output, sha256: hash(bytes)};
});
const result = {product_id: 226, method: 'Static AST/CSS and byte comparisons only; UTF-16 offsets',
  manifest: {path: manifestPath, sha256: hash(read(manifestPath))},
  mount: {path: page.path, sha256: page.sha256, offset: page.nav_offset, source: page.component},
  receipts, css, assets};
const file = 'docs/re/mouse-226-dpi-number-current-evidence.json', output = JSON.stringify(result, null, 2) + '\n';
if (process.argv.includes('--check')) { if (read(file) !== output) throw Error('Stale 226 DPI number receipts'); }
else fs.writeFileSync(path.join(root, file), output);
console.log(`226 DPI number: ${receipts.length} AST receipts, ${css.reduce((n,c)=>n+c.rules.length,0)} CSS rules, ${assets.length} asset comparisons`);
