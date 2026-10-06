// Independent current 226 DPI slider source receipts. Static parsing only.
const fs = require('fs'), path = require('path');
const acorn = require('acorn');
const {Source, hash, walk} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const directory = '.ref/devices/226';
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const manifestPath = directory + '/asset-manifest.json';
const manifest = JSON.parse(read(manifestPath));
const source = Object.assign(Object.create(Source.prototype), {directory,
  files: [...new Set(Object.values(manifest.files))].filter(f => f.endsWith('.js')).map(f => directory + '/' + f.slice(2)),
  modules: new Map(), texts: new Map(), parsed: new Set()});
const chunk = directory + '/static/js/8355.3d5e573e.chunk.js';
source.parse(chunk);
const owner = [...source.modules.values()].find(m => m.definitions.has('Ft') && m.definitions.has('Wt'));
if (!owner) throw Error('Missing current slider owner');
const receipts = [];
for (const name of ['Ft','Wt','Bt','Zt','Gt','Qt','z','es','ts','ls']) {
  receipts.push({symbol: name, module: owner.id, ...source.receipt(owner.id, source.binding(owner.id, name))});
}
receipts.push({symbol:'CONFIG', module:1057, ...source.receipt(1057, source.module(1057).fn)});
const helperModule = source.binding(owner.id, 'N').arguments[0].value;
receipts.push({symbol:'range-inclusion-helper', module:helperModule,
  ...source.receipt(helperModule, source.binding(helperModule, source.exported(helperModule, 'Bt').name))});
walk(owner.fn.body, n => {
  if (n.type === 'VariableDeclarator' && n.id.type === 'ObjectPattern'
      && n.id.properties.some(p => p.key?.name === 'SENSITIVITY_RANGE_VALUES')) {
    receipts.push({symbol:'slider-config-destructure',module:owner.id,...source.receipt(owner.id,n)});
  }
});
const reactPath = '.ref/host-4.0.827/electron/assets/js/react-dom/18.2.0/umd/react-dom.production.min.js';
const reactText = read(reactPath);
walk(acorn.parse(reactText,{ecmaVersion:'latest'}),n => {
  if (n.type === 'FunctionDeclaration' && ['w','S','x'].includes(n.id.name) && n.start < 7500) {
    receipts.push({symbol:'ReactDOM-range-defaultValue:' + n.id.name,path:reactPath,
      sha256:hash(reactText),offset:n.start,end:n.end,source:reactText.slice(n.start,n.end)});
  }
});
const css = [];
for (const relative of [manifest.files['main.css'],manifest.files['static/css/8355.f94d4299.chunk.css']]) {
  const file = directory + '/' + relative.slice(2), text = read(file);
  const rules = parseCSS(text).filter(r => /(^|,)\s*(?:\.slider(?:[- .:]|$)|\.slider-container|\.thumb-tag|\.stage(?:[ .:]|$)|\.stages(?:[ .:]|$))/.test(r.selector)
    || ['body,html','div'].includes(r.selector));
  if (rules.length) css.push({path:file,sha256:hash(text),rules});
}
const out = {method:'Static Acorn/CSS parsing; UTF-16 offsets; no reference JavaScript execution',
  manifest:{path:manifestPath,sha256:hash(read(manifestPath))},configExports:[...source.module(1057).exports.keys()],receipts,css};
const destination = 'docs/re/mouse-226-dpi-slider-review-evidence.json';
const text = JSON.stringify(out,null,2) + '\n';
if (process.argv.includes('--check')) {
  if (read(destination) !== text) throw Error('Stale ' + destination);
} else fs.writeFileSync(path.join(root,destination),text);
console.log(`226 DPI slider independent receipts: ${receipts.length} AST; ${css.reduce((n,c)=>n+c.rules.length,0)} CSS`);
