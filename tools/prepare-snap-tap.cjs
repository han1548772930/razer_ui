// Current 515 Snap Tap: static AST/CSS/data extraction. Never evaluate vendor JS.
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
async function main() {
  const directory = '.ref/devices/515', manifestPath = directory + '/asset-manifest.json';
  const manifest = JSON.parse(read(manifestPath));
  const file = directory + '/' + manifest.files['main.js'].slice(2), text = read(file);
  const source = Object.assign(Object.create(Source.prototype), {directory,
    files: [...new Set(Object.values(manifest.files))].filter(f => f.endsWith('.js')).map(f => directory + '/' + f.slice(2)),
    modules: new Map(), texts: new Map(), parsed: new Set()});
  const names = ['ha', 'jc', 'Xc', 'Zc', 'qc', 'wc', 'zc', 'kc', 'tA', 'EA', 'Vl', 'wl', 'zl', 'kl', 'xl', 'Xl', 'jl', 'Zl', 'ql', 'Ql', 'Jl', '$l'];
  const nodes = new Map(), receipts = [];
  walk(acorn.parse(text, {ecmaVersion: 'latest'}), n => {
    if (['VariableDeclarator', 'FunctionDeclaration', 'ClassDeclaration'].includes(n.type) && n.start > 6800000 && names.includes(n.id?.name)) {
      if (nodes.has(n.id.name)) throw Error('Ambiguous ' + n.id.name);
      nodes.set(n.id.name, n.init || n);
      receipts.push({symbol: n.id.name, path: file, sha256: hash(text), offset: n.start, end: n.end, source: text.slice(n.start, n.end)});
    }
    if (n.type === 'FunctionDeclaration' && n.id?.name === 'TE' && n.start > 6800000 || n.type === 'Property' && n.key?.name === 'snapTapReducer') {
      receipts.push({symbol: n.key?.name || n.id.name, path: file, sha256: hash(text), offset: n.start, end: n.end, source: text.slice(n.start, n.end)});
    }
  });
  for (const name of names) if (!nodes.has(name)) throw Error('Missing ' + name);
  // Only this finite expression grammar is interpreted as data.
  function literal(n) {
    if (n.type === 'Literal') return n.value;
    if (n.type === 'UnaryExpression' && n.operator === '!') return !literal(n.argument);
    if (n.type === 'ArrayExpression') return n.elements.map(literal);
    if (n.type === 'ObjectExpression') return Object.fromEntries(n.properties.map(p => [p.key.name ?? p.key.value, literal(p.value)]));
    if (n.type === 'CallExpression' && n.callee.name === 'parseInt' && n.arguments.length === 2 && n.arguments[1].value === 16) return parseInt(literal(n.arguments[0]), 16);
    throw Error('Nonliteral ' + n.type);
  }
  const keyNode = source.binding(46114, 'n'), layoutNode = source.binding(90857, 'n');
  receipts.push({symbol: 'keyboard-input-table', ...source.receipt(46114, keyNode)},
    {symbol: 'layout-labels', ...source.receipt(90857, layoutNode)},
    {symbol: 'layout-label-function', ...source.receipt(90857, source.binding(90857, 'o'))});
  const labels = {};
  for (const name of ['Fqe', 'qI5', 'f_m', 'PVl', 'Yjp', 'x4F', 'Ol3']) {
    labels[name] = source.literal(54693, source.exported(54693, name));
    receipts.push({symbol: 'label:' + name, ...source.receipt(54693, source.binding(54693, source.exported(54693, name).name))});
  }
  const css = [];
  for (const relative of [...new Set(Object.values(manifest.files))].filter(f => f.endsWith('.css'))) {
    const file = directory + '/' + relative.slice(2), text = read(file);
    const rules = parseCSS(text).filter(r => /snap-tap|key-record-item|blink-active|blink-warning|^\.disabled$|warning-alert|backdrop|create-snaptap-message/.test(r.selector));
    const keyframes = [...text.matchAll(/@keyframes blinker-(?:active|warning)\{(?:[^{}]*\{[^{}]*\})+\}/g)].map(m => ({offset: m.index, source: m[0]}));
    if (rules.length || keyframes.length) css.push({path: file, sha256: hash(text), rules, keyframes});
  }
  const assets = [];
  const base = JSON.parse(read(manifestPath + '.http.json')).source_url;
  for (const name of ['icon_delete', 'icon_delete_snap', 'warning']) {
    const relative = manifest.files[`static/media/${name}.svg`], file = directory + '/' + relative.slice(2);
    if (!fs.existsSync(path.join(root, file))) {
      if (check) throw Error('Missing ' + file);
      const url = new URL(relative, base), response = await fetch(url);
      if (!response.ok) throw Error('Fetch failed ' + url);
      const bytes = Buffer.from(await response.arrayBuffer());
      if (!bytes.toString('utf8').includes('<svg')) throw Error('Not SVG');
      fs.mkdirSync(path.dirname(path.join(root, file)), {recursive: true});
      output(file, bytes);
      output(file + '.http.json', JSON.stringify({source_url: url.href, final_url: response.url, http_status: response.status,
        fetched_at_utc: new Date().toISOString(), sha256: hash(bytes), bytes: bytes.length}, null, 2) + '\n');
    }
    const bytes = fs.readFileSync(path.join(root, file)), dest = `assets/synapse/snap-tap-${name}.svg`;
    output(dest, bytes); assets.push({source: file, output: dest, sha256: hash(bytes)});
  }
  output('assets/synapse/snap-tap-embedded.rs', '&[\n' + assets.map(a =>
    `    ("synapse/${path.basename(a.output)}", include_bytes!("${path.basename(a.output)}") as &[u8]),`).join('\n') + '\n]\n');
  output('crates/razer-pages/src/features/keyboard_snap_tap_data.json', JSON.stringify({defaults: literal(nodes.get('ha')), forbidden: literal(nodes.get('xl')),
    inputs: literal(keyNode), razer_keys: literal(nodes.get('wc')), layouts: source.literal(90857, layoutNode)}, null, 2) + '\n');
  output('docs/re/snap-tap-current-evidence.json', JSON.stringify({method: 'Static parsing only; offsets are UTF-16', product_id: 515,
    manifest: {path: manifestPath, sha256: hash(read(manifestPath))}, receipts, labels, css, assets}, null, 2) + '\n');
  console.log(`515 Snap Tap: ${receipts.length} AST receipts, ${css.reduce((n,c)=>n+c.rules.length,0)} CSS rules, ${assets.length} assets`);
}
main().catch(e => {console.error(e); process.exitCode = 1;});
