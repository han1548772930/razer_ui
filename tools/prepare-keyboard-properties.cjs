// 515 ordinary Customize → MA/LA. Static parsing; no vendor code execution.
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
  const manifest = JSON.parse(read(manifestPath)), file = directory + '/' + manifest.files['main.js'].slice(2), text = read(file);
  const source = Object.assign(Object.create(Source.prototype), {directory,
    files: [...new Set(Object.values(manifest.files))].filter(f => f.endsWith('.js')).map(f => directory + '/' + f.slice(2)),
    modules: new Map(), texts: new Map(), parsed: new Set()});
  const names = ['MA', 'LA', 'mA', 'pA', '$l', 'fr', 'vr'], receipts = [];
  walk(acorn.parse(text, {ecmaVersion: 'latest'}), n => {
    const symbol = n.id?.name;
    const declaration = names.includes(symbol) && n.start > 7000000 && ['ClassDeclaration', 'VariableDeclarator'].includes(n.type);
    const wrapperName = n.left?.property?.name;
    const wrapper = n.type === 'AssignmentExpression' && ['OpenKeyboardProperties', 'getWindowVersion'].includes(wrapperName)
      && n.start > 1700000 && n.start < 1800000;
    if (declaration || wrapper) receipts.push({symbol: declaration ? symbol : wrapperName, path: file, sha256: hash(text), offset: n.start, end: n.end, source: text.slice(n.start, n.end)});
  });
  for (const name of names) if (receipts.filter(r => r.symbol === name).length !== 1) throw Error('Re-audit ' + name);
  for (const name of ['OpenKeyboardProperties', 'getWindowVersion']) if (receipts.filter(r => r.symbol === name).length !== 1) throw Error('Re-audit wrapper ' + name);
  const labels = {};
  for (const symbol of ['SwD', 'MxL', 'Tpt']) {
    labels[symbol] = source.literal(54693, source.exported(54693, symbol));
    receipts.push({symbol: 'label:' + symbol, ...source.receipt(54693, source.binding(54693, source.exported(54693, symbol).name))});
  }
  const css = [];
  for (const relative of [...new Set(Object.values(manifest.files))].filter(f => f.endsWith('.css'))) {
    const path = directory + '/' + relative.slice(2), text = read(path);
    const rules = parseCSS(text).filter(r => /img-text|windows-11|\.windows(?:[, .:]|$)|body-widgets|\.widget .help|\.widget .title/.test(r.selector));
    if (rules.length) css.push({path, sha256: hash(text), rules});
  }
  const assets = [];
  const base = JSON.parse(read(manifestPath + '.http.json')).source_url;
  for (const [name, suffix] of [['common-windows-11', 'win11'], ['windows_logo', 'legacy']]) {
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
    const bytes = fs.readFileSync(path.join(root, file)), dest = `assets/synapse/keyboard-properties-${suffix}.svg`;
    output(dest, bytes); assets.push({source: file, output: dest, sha256: hash(bytes)});
  }
  output('assets/synapse/keyboard-properties-embedded.rs', '&[\n' + assets.map(a =>
    `    ("synapse/${path.basename(a.output)}", include_bytes!("${path.basename(a.output)}") as &[u8]),`).join('\n') + '\n]\n');
  output('docs/re/keyboard-properties-current-evidence.json', JSON.stringify({method: 'Static parsing only; offsets are UTF-16', product_id: 515,
    manifest: {path: manifestPath, sha256: hash(read(manifestPath))}, receipts, labels, css, assets}, null, 2) + '\n');
  console.log(`515 Keyboard Properties: ${receipts.length} AST receipts, ${css.reduce((n,c)=>n+c.rules.length,0)} CSS rules, ${assets.length} assets`);
}
main().catch(e => {console.error(e); process.exitCode = 1;});
