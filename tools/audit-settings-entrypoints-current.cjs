// Current original presentation receipts. Parse downloaded JS as data only.
const fs = require('fs'), path = require('path');
const acorn = require('acorn');
const {Source, walk, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const assert = (value, message) => { if (!value) throw Error(message); };
const receipts = [];
function application(route) {
  const source = new Source(route);
  const manifest = JSON.parse(read(`${source.directory}/asset-manifest.json`));
  source.files = [...new Set(Object.values(manifest.files))].filter(file => file.endsWith('.js'))
    .map(file => file.startsWith('/') ? '.ref/applications' + file : `${source.directory}/${file.replace(/^\.\//, '')}`);
  return source;
}
function binding(source, module, name) {
  const receipt = {module, name, ...source.receipt(module, source.binding(module, name))};
  receipts.push(receipt);
  return receipt.source;
}
const settings = application('synapse/settings');
const synapse = binding(settings, 4914, 'ho');
const general = binding(settings, 4914, 'uo');
const migration = binding(settings, 4914, 'Ta');
const lighting = binding(settings, 4914, 'ia');
const mountedRoot = binding(settings, 4914, 'kr');
const mountedView = binding(settings, 4914, 'mo');
const navigation = binding(settings, 4914, 'po');
const navigationItems = binding(settings, 4914, '$i');
const columns = binding(settings, 4914, 'ee');
binding(settings, 4914, 'Q');
binding(settings, 4914, 'ue');
for (const name of ['Ks', 'bn', 'Ia', 'Un', 'Te', 'Fs', 'xn', 'Da', 'Aa', 'Ma', 'fa', '_a', 'Na', 'ba']) binding(settings, 4914, name);
assert(mountedRoot.includes('(po,{})') && mountedRoot.includes('(mo,{})')
  && mountedView.includes('className:"main-setting"')
  && navigation.includes('navs:$i') && navigationItems === '[{id:1,name:qi},{id:2,name:Qi}]', 'Actual Settings top navigation changed');
assert(columns.includes('className:s') && columns.includes('"widget-col col-"'), 'Actual column owner changed');
assert(![mountedRoot, mountedView, synapse, general].some(text => text.includes('side-navigation') || text.includes('setting-content')), 'Previously unused Settings sidebar became mounted; re-audit geometry');
const cssPath = '.ref/applications/synapse/settings/static/css/720.dbc9cca5.chunk.css';
const cssSource = read(cssPath);
const cssSelectors = ['.body-wrapper', '.body-widgets', '.widget-col', '.body-widgets .widget',
  'div.flex', 'div.flex>div',
  '.widget .titleRow .title', '.main-setting .widget .title', '.main-setting div.widget-col',
  '.body-widgets div.widget-col', 'div.nav-tabs', '.nav-tabs .nav', '.widget-switch',
  '.check-box', '.check-box:after', '.check-box:before', '.check-text',
  '.main-setting .widget .check-item', '.tree-checkbox .child-content .note'];
const css = parseCSS(cssSource).filter(rule => cssSelectors.includes(rule.selector))
  .map(rule => ({path: cssPath, sha256: hash(cssSource), ...rule}));
const hasCSS = (selector, property, value) => css.some(rule => rule.selector === selector
  && rule.properties.some(entry => entry.property === property && entry.value === value));
for (const [selector, property, value] of [['.body-wrapper', 'padding', '10px 20px 20px'],
  ['.body-widgets', 'max-width', '1240px'], ['.widget-col', 'width', '600px'],
  ['.body-widgets .widget', 'margin', '10px auto'], ['.body-widgets .widget', 'padding', '30px 40px'],
  ['.main-setting .widget .title', 'font-size', '18px'], ['.body-widgets div.widget-col', 'margin', '0 30px']])
  assert(hasCSS(selector, property, value), `Current layout changed: ${selector}/${property}`);
for (const [selector, property, value] of [['.check-box', 'width', '20px'],
  ['.check-box', 'height', '20px'], ['.check-box', 'border-radius', '2.4px'],
  ['.check-text', 'font-size', '14px'], ['.check-text', 'line-height', '17px'],
  ['.main-setting .widget .check-item', 'margin', '0']])
  assert(hasCSS(selector, property, value), `Current checkbox changed: ${selector}/${property}`);
assert(hasCSS('div.flex>div', 'flex', 'auto'), 'Original Settings column growth changed');
const narrowRules = css.filter(rule => ['.main-setting div.widget-col', '.body-widgets div.widget-col'].includes(rule.selector));
assert(narrowRules.length === 2 && narrowRules[1].selector === '.body-widgets div.widget-col'
  && narrowRules.every(rule => rule.conditions.includes('@media(max-width:1279px)')), 'Column media cascade order changed');
assert(synapse.includes('direction:"left"') && synapse.includes('direction:"right"')
  && ['Ks', 'bn', 'Ia', 'Un', 'Ta', 'ia'].every(name => synapse.includes(`(${name},{})`)), 'Synapse mounted columns changed');
assert(general.includes('(Te,{})') && general.includes('(Fs,{})'), 'General mounted content changed');
assert(migration.includes('"syn3-profile-migration"') && migration.includes('window.open(t,e,"policy=3,tab_visible=1,shouldFocus=1")'), 'Migration must remain a host tab');
assert(lighting.includes('extraClass:"twoway-lighting no-inner-border"') && lighting.includes('msSettings("personalization-lighting")'), 'Lighting inline settings contract changed');
const dashboard = application('synapse/dashboard');
const picker = binding(dashboard, 96776, 'E');
const header = binding(dashboard, 96776, 'We');
assert(picker.includes('className:"app-explorer"') && picker.includes('openPopUp:E') && picker.includes('f(e=>!e)'), 'Header picker popup contract changed');
assert(header.includes('url:"/synapse/settings/"'), 'Settings host navigation changed');
function declaration(file, name) {
  const source = read(file), found = [];
  walk(acorn.parse(source, {ecmaVersion: 'latest'}), node => {
    if (node.type === 'VariableDeclarator' && node.id.name === name) found.push(node);
  });
  assert(found.length === 1, `Ambiguous original ${name}`);
  const node = found[0];
  receipts.push({name, path: file, sha256: hash(source), offset: node.start, end: node.end, source: source.slice(node.start, node.end)});
  return source.slice(node.start, node.end);
}
const alexa = declaration('.ref/applications/synapse/alexa/static/js/main.05f102d2.js', 'ch');
const migrator = declaration('.ref/profile-migration/static/js/main.512f18b6.js', 'OD');
assert(alexa.includes('document.title="ALEXA"') && alexa.includes('(QE,{})'), 'Alexa application root changed');
assert(migrator.includes('document.title="PROFILE MIGRATION"') && migrator.includes('tabNavigations:[]'), 'Migration application root changed');
// Check product presentation against maintained exact original component slices.
const productEvidence = ['hue-current-evidence', 'aether-strip-current-evidence',
  'wired-argb-current-evidence', 'wireless-argb-current-evidence', 'automation-current-evidence'];
const productReceipts = productEvidence.map(name => {
  const file = `docs/re/${name}.json`, document = JSON.parse(read(file));
  let slices = 0;
  function verify(value) {
    if (!value || typeof value !== 'object') return;
    if (typeof value.path === 'string' && value.path.startsWith('.ref/devices/')
        && Number.isInteger(value.offset) && Number.isInteger(value.end) && typeof value.source === 'string') {
      const original = read(value.path);
      assert(original.slice(value.offset, value.end) === value.source, `Stale original slice in ${file}`);
      slices++;
    }
    for (const item of Object.values(value)) if (typeof item === 'object') verify(item);
  }
  verify(document);
  assert(slices > 0, `No verified product components in ${file}`);
  return {path: file, sha256: hash(read(file)), verified_slices: slices};
});
const page = read('crates/razer-settings/src/settings_page.rs');
const shell = read('crates/razer-shell/src/shell.rs');
const eventStart = shell.indexOf('settings_page::SettingsEvent::Changed =>');
const eventEnd = shell.indexOf('settings_page::SettingsEvent::ResetTutorials =>', eventStart);
assert(eventStart >= 0 && eventEnd > eventStart, 'Settings dispatch boundary changed');
assert(!page.includes('::open_preview(') && !page.includes('open_calibration_preview(')
  && !shell.slice(eventStart, eventEnd).includes('open_preview('), 'Settings still opens a sample wrapper');
assert(!page.includes('Checkbox::new(') && page.includes('surface::check_item_with_style(')
  && page.includes('tick_bottom_origin: (0.8, 10.2)'), 'Settings no longer uses original checkbox geometry');
assert((page.match(/\.flex_grow\(1\.\)/g) || []).length >= 4
  && page.includes('.flex_basis(surface::css(600.))'), 'Settings columns lost source flex:auto growth');
const result = {schema_version: 1, method: 'Acorn mounted roots and byte-exact current product slices; no vendor execution',
  generator_sha256: hash(fs.readFileSync(__filename)), receipts, css, product_evidence: productReceipts,
  implementation: ['crates/razer-settings/src/settings_page.rs', 'crates/razer-widgets/src/surface.rs', 'crates/razer-shell/src/shell.rs', 'crates/razer-app-pages/src/app_picker.rs']
    .map(file => ({path: file, sha256: hash(fs.readFileSync(path.join(root, file)))})),
  scope: 'Current Settings mounted Synapse/General navigation, column/card geometry and recommendation switch; user-authorized Connection diagnostics opens actual page owners and is separate from original coverage',
  remaining: ['Actual page details and backend coverage remain tracked individually', 'No application, visual, focus or hardware runtime acceptance'], runtime_acceptance: 'not_run'};
const output = path.join(root, 'docs/re/settings-entrypoints-current-evidence.json');
const text = JSON.stringify(result, null, 2) + '\n';
if (process.argv.includes('--check')) assert(fs.readFileSync(output, 'utf8') === text, 'Settings entrypoint evidence is stale');
else fs.writeFileSync(output, text);
console.log(`Settings entrypoints: ${receipts.length} original roots/actions and ${productReceipts.reduce((n, row) => n + row.verified_slices, 0)} product slices verified`);
