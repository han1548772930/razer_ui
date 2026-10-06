// Current mounted 3334/3337 branches. Parse vendor source only; never evaluate it.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {Source, walk, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), check = process.argv.includes('--check');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
function output(file, value) {
  const bytes = Buffer.isBuffer(value) ? value : Buffer.from(value);
  if (check) {
    if (!fs.readFileSync(path.join(root, file)).equals(bytes)) throw Error('Stale ' + file);
  } else fs.writeFileSync(path.join(root, file), bytes);
}
const products = [];
for (const pid of [3334, 3337]) {
  const directory = `.ref/devices/${pid}`, manifestPath = directory + '/asset-manifest.json';
  const manifest = JSON.parse(read(manifestPath));
  const main = directory + '/' + manifest.files['main.js'].replace(/^\.\//, '');
  const source = read(main), sha256 = hash(source), receipts = [];
  const s = Object.assign(Object.create(Source.prototype), {directory,
    files: [...new Set(Object.values(manifest.files))].filter(f => f.endsWith('.js')).map(f => directory + '/' + f.replace(/^\.\//, '')),
    modules: new Map(), texts: new Map(), parsed: new Set()});
  walk(acorn.parse(source, {ecmaVersion: 'latest'}), node => {
    if (node.type !== 'VariableDeclarator' || node.start < 3900000 || !['Wn', 'Yn', 'yU', 'Jm'].includes(node.id?.name)) return;
    receipts.push({symbol: node.id.name, path: main, sha256, offset: node.start, end: node.end, source: source.slice(node.start, node.end)});
  });
  for (const name of ['Wn', 'Yn', 'yU', 'Jm']) if (receipts.filter(r => r.symbol === name).length !== 1) throw Error(`Missing ${pid}:${name}`);
  const mounted = receipts.find(r => r.symbol === 'yU').source;
  for (const text of ['activeStreamMixerStatus:e.streamMixerReducer.activeStreamMixerStatus',
    'playbackDevices:e.streamMixerReducer.playbackDevices', 'playbackMixDevice:e.streamMixerReducer.playbackMixDevice',
    'toggleSwitch:()=>C?E(!C):void(A?B(!0):E(!C))', 'E.push({name:P,disabled:!0})',
    'f&&!C?', 'E(!0),B(!1)', 'height:"164px",width:"620px"']) {
    if (!mounted.includes(text)) throw Error(`Re-audit mounted ${pid}: ${text}`);
  }
  const labels = {};
  for (const symbol of ['Ec2', 'nlF', 'bOp', 'iRy', 'YHN', 'Js1', 'G$0']) {
    labels[symbol] = s.literal(4693, s.exported(4693, symbol));
    receipts.push({symbol: 'label:' + symbol, ...s.receipt(4693, s.binding(4693, s.exported(4693, symbol).name))});
  }
  const actions = {};
  for (const symbol of ['QsH', 'JKI', 'Rax']) {
    actions[symbol] = s.literal(3254, s.exported(3254, symbol));
    receipts.push({symbol: 'action:' + symbol, ...s.receipt(3254, s.binding(3254, s.exported(3254, symbol).name))});
  }
  const css = [];
  for (const relative of [...new Set(Object.values(manifest.files))].filter(f => f.endsWith('.css'))) {
    const file = directory + '/' + relative.replace(/^\.\//, ''), text = read(file);
    const rules = parseCSS(text).filter(r => /^(\.remove-alert|\.warning-alert|\.style-icon|\.backdrop(?:[,. ]|$))/.test(r.selector));
    if (rules.length) css.push({path: file, sha256: hash(text), rules});
  }
  const assets = [];
  for (const name of ['warning', 'warning-icon']) {
    const file = directory + '/' + manifest.files[`static/media/${name}.svg`].replace(/^\.\//, '');
    const bytes = fs.readFileSync(path.join(root, file)), dest = `assets/synapse/stream-mixer-${name}.svg`;
    if (pid === 3334) output(dest, bytes);
    else if (!bytes.equals(fs.readFileSync(path.join(root, dest)))) throw Error('Product asset differs: ' + file);
    assets.push({source: file, output: dest, sha256: hash(bytes)});
  }
  products.push({product_id: pid, manifest: {path: manifestPath, sha256: hash(read(manifestPath))}, receipts, labels, actions, css, assets});
}
output('assets/synapse/stream-mixer-embedded.rs', '&[\n' + ['warning', 'warning-icon'].map(name =>
  `    ("synapse/stream-mixer-${name}.svg", include_bytes!("stream-mixer-${name}.svg") as &[u8]),`).join('\n') + '\n]\n');
output('docs/re/stream-mixer-current-evidence.json', JSON.stringify({method: 'Static AST, CSS and asset comparison only', products}, null, 2) + '\n');
console.log(`Stream Mixer: ${products.length} products, ${products.reduce((n,p)=>n+p.receipts.length,0)} AST receipts, 4 asset comparisons`);
