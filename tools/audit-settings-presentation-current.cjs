// Parse the current Settings source as data; never load its JavaScript.
const fs = require('fs');
const path = require('path');
const acorn = require('acorn');
const {Source, hash, walk} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const check = process.argv.includes('--check');
const manifestPath = '.ref/applications/settings/asset-manifest.json';
const manifest = JSON.parse(read(manifestPath));
const source = new Source('settings');
source.files = [...new Set(Object.values(manifest.files))]
  .filter(file => /^\/settings\/static\/js\/[^/]+\.js$/.test(file))
  .map(file => '.ref/applications' + file);
const assert = (condition, message) => { if (!condition) throw Error(message); };
const receipts = [[8821, 'ss'], [9302, 'ee'], [9762, 'ye'], [9762, 'x'], [9762, 'fe'],
  [3414, 'Se'], [3414, 'u'], [3414, 'be']]
  .map(([module, name]) => ({module, name, ...source.receipt(module, source.binding(module, name))}));
for (const [module, name, expected] of [[9762, 'pe', 1422], [9762, 'me', 3765], [9762, 'he', 7693],
  [3414, 'Oe', 1422], [3414, 'Ne', 3765], [3414, 'Ee', 7693]]) {
  const node = source.binding(module, name);
  assert(node.type === 'CallExpression' && node.callee.name === source.module(module).fn.params[2].name
    && node.arguments[0]?.value === expected, `Mounted ${module}/${name} import changed`);
  receipts.push({module, name: name + '-import', ...source.receipt(module, node)});
}
const resolvedExport = (module, name) => {
  let node = source.exported(module, name);
  const seen = new Set();
  while (node.type === 'Identifier') {
    assert(!seen.has(node.name), `Circular ${module}/${name}`);
    seen.add(node.name);
    node = source.binding(module, node.name);
  }
  return node;
};
for (const [module, name] of [[1422, 'widget-collection'], [3765, 'widget-column']])
  receipts.push({module, name, ...source.receipt(module, resolvedExport(module, 'A'))});
const widgetExport = source.exported(7693, 'A');
assert(widgetExport.type === 'Identifier', 'Widget export changed');
const connectedWidget = source.binding(7693, widgetExport.name);
assert(connectedWidget.type === 'CallExpression' && connectedWidget.arguments[0]?.type === 'Identifier', 'Widget connection changed');
const widgetClass = source.binding(7693, connectedWidget.arguments[0].name);
receipts.push({module: 7693, name: 'widget-export', ...source.receipt(7693, connectedWidget)},
  {module: 7693, name: 'widget-class', ...source.receipt(7693, widgetClass)});
const mainPath = '.ref/applications/settings/static/js/main.eda30dd1.js';
const main = read(mainPath), bootstraps = [];
walk(acorn.parse(main, {ecmaVersion: 'latest'}), node => {
  if (node.type !== 'CallExpression' || node.callee.type !== 'MemberExpression'
    || node.callee.property.name !== 'then') return;
  const code = main.slice(node.start, node.end);
  if (code.startsWith('Promise.all([r.e(575),r.e(97)]).then(r.bind(r,8821)).then(')) bootstraps.push(node);
});
assert(bootstraps.length === 1, 'Settings bootstrap/root chunk order changed');
receipts.push({name: 'bootstrap', path: mainPath, sha256: hash(main),
  offset: bootstraps[0].start, end: bootstraps[0].end, source: main.slice(bootstraps[0].start, bootstraps[0].end)});
const indexPath = '.ref/applications/settings/index.html';
assert(read(indexPath).includes('<link href="/settings/static/css/main.a418d266.css" rel="stylesheet">'), 'Initial CSS order changed');
assert(main.includes('r.miniCssF=e=>"static/css/"+e+"."+{97:"394f9404"'), 'Root 97 CSS mapping changed');
const selected = new Map([
  ['main.a418d266.css', ['body']],
  ['97.394f9404.chunk.css', ['body,html', '.body-widgets .widget', '.disabled']],
  ['921.08e5c8d7.chunk.css', ['.software-tab .title', '.installed-software',
    '.installed-software .title-bar', '.installed-software .title-bar>.installed-action',
    '.installed-software .title-bar>.installed-action>div', '.installed-software .action']],
  ['762.226a09dd.chunk.css', ['.settings .input-label', '.section.launcher>.apps']],
  ['414.2e30559c.chunk.css', ['.settings .input-label', '.section.about>.links',
    '.section.about>.links>a', '.section.about>.links>.divider']],
]);
const css = [];
for (const url of [...new Set(Object.values(manifest.files))].filter(file => file.endsWith('.css'))) {
  const selectors = selected.get(path.basename(url));
  if (!selectors) continue;
  const file = '.ref/applications' + url, text = read(file);
  const rules = parseCSS(text).filter(rule => selectors.includes(rule.selector));
  for (const selector of selectors) assert(rules.some(rule => rule.selector === selector), `Missing ${url} ${selector}`);
  css.push(...rules.map(rule => ({path: file, sha256: hash(text), ...rule})));
}
const declaration = (selector, property, value) => css.some(rule => rule.selector === selector
  && rule.properties.some(entry => entry.property === property && entry.value === value));
for (const [selector, property, value] of [
  ['body', 'line-height', '1.22'], ['body,html', 'font-size', '16px'],
  ['.body-widgets .widget', 'font-size', '14px'],
  ['.software-tab .title', 'font-family', 'RazerF5'],
  ['.software-tab .title', 'font-size', '22px'],
  ['.software-tab .title', 'font-weight', '300'],
  ['.software-tab .title', 'color', '#44d62c'],
  ['.software-tab .title', 'flex', '1 0 auto'],
  ['.settings .input-label', 'font-weight', '700'],
  ['.settings .input-label', 'text-transform', 'uppercase'],
  ['.section.launcher>.apps', 'width', '360px'],
  ['.section.launcher>.apps', 'height', '60px'],
  ['.section.launcher>.apps', 'border', '1px solid #707070'],
  ['.section.launcher>.apps', 'margin-bottom', '20px'],
  ['.section.about>.links>.divider', 'margin', '0 6px'],
  ['.section.about>.links>a', 'cursor', 'default'],
]) assert(declaration(selector, property, value), `Current CSS changed: ${selector} ${property}=${value}`);
const byName = name => receipts.find(receipt => receipt.name === name).source;
assert(byName('ss').includes('[we.GD6]') && byName('ss').includes('[we.n6W]') && byName('ss').includes('[we.iRj]'), 'Root tab mount changed');
assert(byName('ye').includes('className:"settings"') && byName('ye').includes('title:u.TEXT_QUICK_LAUNCHER'), 'Systray ancestry changed');
assert(byName('Se').includes('className:"settings"') && byName('Se').includes('title:a.OZV'), 'General ancestry changed');
assert(byName('widget-collection').includes('className:"body-widgets flex '), 'Widget collection ancestry changed');
assert(byName('widget-class').includes('="widget ') && byName('widget-class').includes('className:"widget-container"'), 'Visible Widget export/mount changed');
assert(byName('ee').includes('className:"title"') && byName('ee').includes('className:"action disabled"'), 'Installed header mount changed');
assert(byName('x').includes('className:"apps-".concat(L.length," apps list-unstyled"),children:L.map'), 'Empty launcher list mount changed');
assert(!byName('x').includes('L.length>0?'), 'Launcher visibility now conditional');
assert((byName('be').match(/className:"divider",children:"\|"/g) || []).length === 3, 'About dividers changed');
assert(byName('be').includes('style:{textTransform:"uppercase"}'), 'Connect With Us transform changed');
const nativePath = 'crates/razer-settings/src/settings_window.rs';
const native = read(nativePath);
for (const fact of [
  '.id("host-settings-installed-title")', '.font_family("RazerF5")',
  '.text_size(surface::css(22.))', '.font_weight(FontWeight::LIGHT)',
  '.id("host-settings-auto-update")', '.text_size(surface::css(14.))',
  '.id("host-settings-launcher-preview")', '.w(surface::css(360.))', '.h(surface::css(60.))',
  '.font_weight(FontWeight::BOLD)', '.child(text(key).to_uppercase())',
  '.child(input_label("PREVIEW"))', '.child(input_label("ORDER_FROM_LEFT_TO_RIGHT"))',
  '.child(input_label("SYSTRAY_ICON"))', '.child(input_label("LANGUAGE"))',
  '.child("|")', '.mx(surface::css(6.))', '.cursor_default()',
  '.child(text("CONNECT_WITH_US").to_uppercase())', '.line_height(relative(1.22))',
]) assert(native.includes(fact), `Native presentation missing ${fact}`);
const output = {
  schema_version: 1,
  method: 'Static Acorn/CSS source parsing only. No reference JavaScript, application, DLL or tests executed.',
  manifest: {path: manifestPath, sha256: hash(read(manifestPath))},
  css_load_order: {index: {path: indexPath, sha256: hash(read(indexPath))},
    initial: '/settings/static/css/main.a418d266.css',
    root_chunk: '97.394f9404.chunk.css before8821 renders root',
    widget_mount: '9762/ye and3414/Se ->1422/i body-widgets ->3765/i widget-col ->7693/A connected component ->widget class'},
  observations: {
    typography: 'main body retains line-height 1.22; lazy 97 body,html sets Roboto16; widgets set14',
    installed_header: 'RazerF5 22/weight300/green/flex1 0 auto; separate vertically centered disabled14px action',
    launcher_preview: 'Mounted even when filtered L=[]; 360x60/border707070/margin-bottom20',
    input_labels: 'Preview/order/systray/language use bold uppercase .settings .input-label',
    about_links: 'Three literal pipe divider nodes with6px side margins; default cursor; Connect With Us uppercase',
  },
  receipts, css,
  implementation: {path: nativePath, sha256_lf: hash(native.replace(/\r\n/g, '\n'))},
  runtime_validation: 'not_run',
  remaining: 'Installed-app/widget catalogs and launcher state publisher unavailable; window/layout/focus/pixels not runtime validated.',
};
const out = 'docs/re/settings-presentation-current-evidence.json';
const bytes = JSON.stringify(output, null, 2) + '\n';
if (check) assert(read(out).replace(/\r\n/g, '\n') === bytes, `Stale ${out}`);
else fs.writeFileSync(path.join(root, out), bytes);
console.log(`Current Settings presentation ${check ? 'checked' : 'prepared'}: ${receipts.length} AST receipts, ${css.length} CSS rules`);
