// Parse the mounted current calibration pages as data. Never evaluate vendor JS.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const check = process.argv.includes('--check');
const key = node => node?.name ?? node?.value;
function walk(node, visit) {
  if (!node?.type || visit(node) === false) return;
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) value.forEach(child => walk(child, visit));
    else if (value?.type) walk(value, visit);
  }
}
function stringExports(source) {
  const modules = [];
  walk(acorn.parse(source, {ecmaVersion: 'latest'}), node => {
    if (node.type !== 'Property' || !Number.isInteger(key(node.key)) || !/Function/.test(node.value.type)) return;
    const defs = new Map(), exports = new Map();
    for (const statement of node.value.body.body) {
      if (statement.type === 'VariableDeclaration') for (const declaration of statement.declarations) {
        if (declaration.id.type === 'Identifier') defs.set(declaration.id.name, declaration.init);
      }
    }
    walk(node.value.body, child => {
      if (child.type === 'CallExpression' && key(child.callee.property) === 'd' && child.arguments[1]?.type === 'ObjectExpression') {
        for (const property of child.arguments[1].properties) exports.set(key(property.key), property.value.body);
      }
      if (/Function|Class/.test(child.type)) return false;
    });
    function literal(value, seen = new Set()) {
      if (!value || seen.has(value)) return;
      if (value.type === 'Literal' && typeof value.value === 'string') return value.value;
      if (value.type === 'Identifier') return literal(defs.get(value.name), new Set([...seen, value]));
    }
    const values = Object.fromEntries([...exports].map(([name, value]) => [name, literal(value)]).filter(([, value]) => value !== undefined));
    modules.push({id: key(node.key), values});
    return false;
  });
  return modules;
}
const pages = JSON.parse(read('docs/re/keyboard-product-pages.json')).products;
const products = JSON.parse(read('src/features/keyboard_products_data.json'));
const records = [];
const assets = new Map();
const symbols = {title:'CBp', introduction:'Q_2', description:'e30', start:'KBw', note:'SWf',
  mode:'OEj', select_key:'opb', press_key:'v9s', release_key:'a$L', success:'KUr', failure:'Jv7',
  restart:'uqI', calibrating:'$3v', next:'K29', done:'DHM', cancel:'bOp'};
for (const pid of [740, 746]) {
  const product = products.find(p => p.product_id === pid);
  const page = pages.find(p => p.product_id === pid).pages.find(p => p.key === 'TAB_CALIBRATION');
  const source = read(page.path);
  if (hash(source) !== page.sha256) throw Error('Changed mounted calibration page');
  for (const component of page.components) {
    if (source.slice(component.offset, component.end) !== component.source) throw Error('Stale component offset');
  }
  const modal = page.components.find(c => c.symbol === 'Ea');
  for (const fragment of ['[1,2,3].map', 'maxWidth:"850px"', 'initCalibration', 'calibrateBottom', 'verifyBottom', 'calibrateTop', 'verifyTop', 'stopCalibration', 't>15e3', 'startMonitorForegroundWindow', 'disableMapping', 'enableMapping']) {
    if (!modal?.source.includes(fragment)) throw Error('Changed calibration contract: ' + fragment);
  }
  for (const component of page.components.filter(c => ['Ea', '_a'].includes(c.symbol))) {
    const ast = acorn.parse('(' + component.source + ')', {ecmaVersion:'latest'});
    walk(ast, node => {
      if (node.type !== 'CallExpression' || node.arguments[0]?.value !== 'svg') return;
      const props = new Map(node.arguments[1].properties.map(p => [key(p.key), p.value]));
      const child = props.get('children');
      if (child?.type !== 'CallExpression' || child.arguments[0]?.value !== 'path') throw Error('Nonliteral calibration SVG');
      const attrs = Object.fromEntries(child.arguments[1].properties.map(p => [key(p.key), p.value.value]));
      const name = component.symbol === '_a' ? 'info' : attrs.fill === '#44D62C' ? 'success' : 'failure';
      const svg = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="${props.get('viewBox').value}"><path d="${attrs.d}" fill="${attrs.fill}"/></svg>\n`;
      const output = `assets/synapse/keyboard-calibration-${name}.svg`;
      if (assets.has(output) && assets.get(output).svg !== svg) throw Error('Product calibration SVG differs');
      if (!assets.has(output)) assets.set(output, {svg, source:page.path, source_sha256:page.sha256, output,
        output_sha256:hash(svg), offset:component.offset + node.start - 1, end:component.offset + node.end - 1,
        width:Number(props.get('width').value), height:Number(props.get('height').value)});
    });
  }
  const mainFile = product.source_files.find(f => /\/main\./.test(f.path));
  const main = read(mainFile.path);
  if (hash(main) !== mainFile.sha256) throw Error('Changed calibration main');
  const namespaces = stringExports(main).filter(m => m.values.CBp === 'KEYBOARD_SWITCH_CALIBRATION');
  if (namespaces.length !== 1) throw Error('Ambiguous calibration string namespace');
  const labels = Object.fromEntries(Object.entries(symbols).map(([name, symbol]) => {
    const value = namespaces[0].values[symbol];
    if (!value) throw Error('Missing calibration label ' + symbol);
    return [name, value];
  }));
  const localeReceipts = [];
  const directory = path.posix.dirname(page.path);
  for (const file of fs.readdirSync(path.join(root, directory)).filter(f => /^trans-.*\.js$/.test(f))) {
    const locale = file.match(/^trans-(.*?)\./)[1];
    const text = read(directory + '/' + file);
    const translations = Object.assign({}, ...stringExports(text).map(m => m.values));
    const local = JSON.parse(read(`locales/${locale}.json`));
    for (const label of Object.values(labels)) {
      if (!translations[label] || translations[label] !== local[label]) throw Error(`Calibration locale differs: ${pid}/${locale}/${label}`);
    }
    localeReceipts.push({path:directory + '/' + file, sha256:hash(text)});
  }
  if (localeReceipts.length !== 10) throw Error('Missing calibration locales');
  const cssDirectory = directory.replace('/js', '/css');
  const cssSources = fs.readdirSync(path.join(root, cssDirectory)).filter(f => f.endsWith('.css')).map(f => {
    const file = cssDirectory + '/' + f, css = read(file);
    return {path:file, sha256:hash(css), rules:css.split('}').filter(rule => /KeyboardSwitchCalibration|introduction-calibration|\.choose-a-mat|\.calibration[ >{]|\.backdrop/.test(rule)).map(rule => rule + '}')};
  }).filter(file => file.rules.length);
  records.push({product_id:pid, labels, modal_width:850, sources:[mainFile, {path:page.path, sha256:page.sha256}],
    components:page.components.map(({symbol,offset,end})=>({symbol,offset,end})), css:cssSources, locales:localeReceipts,
    service_contract:['initCalibration','calibrateBottom','verifyBottom','calibrateTop','verifyTop','stopCalibration'],
    idle_error_timeout_ms:15000});
}
const output = 'src/features/keyboard_calibration_data.json';
const content = JSON.stringify(records, null, 2) + '\n';
function emit(output, content) {
  if (check) {
    if (read(output) !== content) throw Error('Stale calibration evidence: ' + output);
  } else fs.writeFileSync(path.join(root, output), content);
}
if (assets.size !== 3) throw Error('Missing calibration SVGs');
emit(output, content);
for (const asset of assets.values()) emit(asset.output, asset.svg);
emit('assets/synapse/keyboard-calibration-manifest.json', JSON.stringify([...assets.values()].map(({svg,...record})=>record), null, 2) + '\n');
console.log(`Verified ${records.length} mounted calibration pages, source CSS and 20 locale bundles; vendor code was not executed.`);
