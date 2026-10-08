// Current systray presentation receipts. Parse vendor JS/CSS as data only.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {walk, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const bytes = p => fs.readFileSync(path.join(root, p));
const assert = (ok, message) => { if (!ok) throw Error(message); };
const base = '.ref/applications/systray/systrayv2/';
const manifestPath = base + 'asset-manifest.json';
const manifest = JSON.parse(read(manifestPath));
const declared = name => {
  const relative = manifest.files[name];
  assert(relative?.startsWith('./static/') && !relative.includes('..'), 'Undeclared resource: ' + name);
  return base + relative.slice(2);
};
const jsPath = declared('static/js/554.2573b048.chunk.js');
const cssPath = declared('static/css/554.7cdbd936.chunk.css');
const mainCssPath = declared('main.css');
const js = read(jsPath), nodes = [];
walk(acorn.parse(js, {ecmaVersion: 'latest'}), node => {
  let symbol, expression;
  if (node.type === 'FunctionDeclaration' && ['se', 'oe', 'Re'].includes(node.id.name)) {
    symbol = node.id.name; expression = node;
  } else if (node.type === 'VariableDeclarator' && node.id.name === 'me') {
    symbol = node.id.name; expression = node.init;
  }
  if (expression) nodes.push({symbol, offset: expression.start, end: expression.end,
    source: js.slice(expression.start, expression.end)});
});
assert(nodes.length === 4, 'Ambiguous current tray components');
const component = symbol => nodes.find(node => node.symbol === symbol).source;
assert(component('Re').includes('!!e.user.item.id'), 'Account mount condition changed');
assert(component('me').includes('apps launcher list-unstyled apps-${n.length}'), 'Launcher mount changed');
const rules = [];
function carries(file, selector, expected) {
  const rule = parseCSS(read(file)).find(rule => rule.selector === selector
    && Object.entries(expected).every(([key, value]) =>
      rule.properties.some(property => property.property === key && property.value === value)));
  assert(rule, 'Current CSS changed: ' + selector);
  rules.push({path: file, selector, offset: rule.offset, declarations: rule.declarations});
}
carries(mainCssPath, 'body', {'font-size':'16px', 'line-height':'1.22', color:'#ccc'});
carries(cssPath, '.systray', {width:'360px', 'max-height':'700px', 'background-color':'#222', border:'1px solid #000', cursor:'default'});
carries(cssPath, '.systray>.header,.systray>.header-2', {height:'60px', padding:'0 20px'});
carries(cssPath, '.systray>.header-2', {'padding-bottom':'1px'});
carries(cssPath, '.systray>.apps', {height:'60px', 'border-top':'1px solid #222', 'background-color':'#111'});
carries(cssPath, '.systray>.apps>li', {height:'59px', padding:'0 10px'});
carries(cssPath, '.systray>.apps>li>.icon', {height:'32px', width:'32px', 'flex-shrink':'0'});
carries(cssPath, '.systray>.apps.apps-1>li>.icon', {'margin-right':'10px'});
carries(cssPath, '.systray>.apps>li>.title', {'font-size':'12px', 'text-overflow':'ellipsis', 'white-space':'nowrap', color:'#999'});
assert(read(cssPath).includes('.launcher{.title{text-transform:uppercase}}'), 'Launcher uppercase rule changed');
const cachePath = '.ref/host-4.0.827/electron/lib/systrayIconCache.js';
const cache = read(cachePath);
assert(cache.includes('h.light=t.createFromPath(r),h.dark=t.createFromPath(a)'), 'Theme icon loading changed');
const images = ['gear-black.png', 'gear-white.png', 'user-black.png', 'user-white.png'];
const assets = JSON.parse(read('assets/synapse/manifest.json')).entries
  .filter(entry => images.includes(path.basename(entry.source)) && entry.output.includes('/tray-'));
assert(assets.length === 4, 'Missing theme icon resources');
for (const entry of assets) {
  const png = bytes(entry.source);
  assert(png.readUInt32BE(16) === entry.width && png.readUInt32BE(20) === entry.height, 'Resized theme icon: ' + entry.output);
  assert(entry.conversion === 'exact-png' && bytes(entry.output).length === entry.width * entry.height * 4, 'Theme raster conversion changed');
  assert(hash(png) === entry.source_sha256 && hash(bytes(entry.output)) === entry.sha256, 'Stale raster receipt');
}
const tray = read('src/shell/tray.rs'), account = read('src/shell/tray/account.rs');
assert((tray.match(/\.line_height\(relative\(1\.22\)\)/g) || []).length >= 3, 'Button line height missing');
assert(account.includes('.line_height(relative(1.22))'), 'Account name line height missing');
assert(/\.id\("tray-apps"\)[\s\S]*?\.h\(surface::css\(60\.\)\)[\s\S]*?\.border_t_1\(\)[\s\S]*?Button::new\("tray-launch-synapse"\)[\s\S]*?\.h\(surface::css\(59\.\)\)/.test(tray), 'Apps border/row geometry flattened');
assert(tray.includes('.child(text("host", "RAZER_SYNAPSE").to_uppercase())') && tray.includes('.truncate()'), 'Title presentation missing');
assert(tray.includes('from_rgba(bytes.to_vec(), width, height)'), 'Menu ignores original icon dimensions');
const inputPaths = [manifestPath, jsPath, cssPath, mainCssPath, cachePath, ...assets.map(entry => entry.source)];
const evidence = {
  schema_version: 1,
  source_inputs: Object.fromEntries(inputPaths.map(file => [file, hash(bytes(file))])),
  nodes, rules, theme_icons: assets,
  changes: ['Inherited body 1.22 line height explicitly overrides Base Button defaults.',
    '60px apps container owns its border; the 59px launcher row is a separate child.',
    'Single launcher title follows uppercase and ellipsis rules.',
    'Theme menu PNGs retain official pixels and dimensions without preparation-time resizing.'],
  remaining: ['Real-window appearance, fonts, keyboard focus, DPI and hover are not run.',
    'muda 0.21.0 Windows to_hbitmap draws all custom menu icons into a 16x16 bitmap; Electron output parity is not established.',
    'Account transport, populated widgets/notifications, multi-app catalog and dynamic popup bounds remain incomplete.'],
  runtime_validation: 'not_run'
};
const output = path.join(root, 'docs/re/tray-presentation-current-evidence.json');
const serialized = JSON.stringify(evidence, null, 2) + '\n';
if (process.argv.includes('--write')) fs.writeFileSync(output, serialized);
else assert(read('docs/re/tray-presentation-current-evidence.json').replace(/\r\n/g, '\n') === serialized, 'Presentation receipts changed; inspect before --write');
console.log('Tray presentation: current AST/CSS, border ownership, typography and four native PNG receipts checked; runtime not run.');
