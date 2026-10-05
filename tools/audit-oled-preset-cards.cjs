// Current 691 preset/card UI receipts. Parse vendor syntax and CSS only.
const fs = require('fs');
const {Source, hash, walk} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const directory = '.ref/devices/691';
const manifest = JSON.parse(fs.readFileSync(`${directory}/asset-manifest.json`, 'utf8'));
const source = Object.create(Source.prototype);
source.directory = directory;
source.files = [...new Set(Object.values(manifest.files))]
  .filter(file => file.includes('/static/js/') && file.endsWith('.js'))
  .map(file => `${directory}/${file.slice(file.indexOf('static/'))}`);
source.modules = new Map(); source.texts = new Map(); source.parsed = new Set();
const bindings = ['Q', 'G', 'wt', 'Vt', 'jt', 'Ht', 'pi', '_e', 'De']
  .map(symbol => ({symbol, ...source.receipt(42553, source.binding(42553, symbol))}));
const page = JSON.parse(fs.readFileSync('docs/re/keyboard-product-pages.json', 'utf8'))
  .products.find(p => p.product_id === 691).pages.find(p => p.key === 'OLED');
if (bindings.some(binding => binding.path !== page.path || binding.sha256 !== page.sha256))
  throw Error('Current mounted OLED source hash changed');
const cssFile = `${directory}/static/css/OLED.a636cf4a.chunk.css`;
const css = fs.readFileSync(cssFile, 'utf8');
const rules = parseCSS(css).filter(rule => /CustomizeAnimation_|CustomizeImage_|DisplayWidget_/.test(rule.selector));
const localization = Object.fromEntries(['PYB', 'Qg_', 'djP', 'VLI', 'scg', 'jJm', 'vs6', 'XVF', 'De8']
  .map(symbol => [symbol, source.literal(54693, source.exported(54693, symbol))]));
for (const language of ['en', 'zh-CN']) {
  const strings = JSON.parse(fs.readFileSync(`locales/${language}.json`, 'utf8'));
  for (const key of Object.values(localization)) if (!strings[key]) throw Error(`Missing ${language}/${key}`);
}
if (process.argv.includes('--inspect')) {
  console.log(JSON.stringify({bindings: bindings.filter(b => ['_e', 'De'].includes(b.symbol)), localization,
    rules: rules.map(({selector, declarations}) => ({selector, declarations}))}, null, 2));
  process.exit(0);
}
const escape = value => String(value).replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;');
function serializeSvg(node) {
  if (!node || node.type === 'ConditionalExpression') return '';
  if (['LogicalExpression', 'AssignmentExpression'].includes(node.type)) return serializeSvg(node.right);
  if (node.type === 'Literal') return node.value == null ? '' : escape(node.value);
  if (node.type !== 'CallExpression' || node.callee.property?.name !== 'createElement') throw Error('Nonliteral SVG');
  const [tag, props, ...children] = node.arguments;
  const object = props.type === 'CallExpression' ? props.arguments[0] : props;
  let attributes = '';
  if (object.type === 'ObjectExpression') for (const p of object.properties) {
    const key = p.key.name || p.key.value;
    if (['ref', 'aria-labelledby', 'nonce'].includes(key)) continue;
    if (p.value.type !== 'Literal') throw Error('Nonliteral SVG attribute');
    const name = ({className:'class', xmlnsXlink:'xmlns:xlink', fillRule:'fill-rule', clipRule:'clip-rule', strokeWidth:'stroke-width', strokeLinecap:'stroke-linecap', strokeLinejoin:'stroke-linejoin'}[key] || key);
    attributes += ` ${name}="${escape(p.value.value)}"`;
  }
  return `<${tag.value}${attributes}>${children.map(serializeSvg).join('')}</${tag.value}>`;
}
const assets = [['_e', 'replace'], ['De', 'reset']].map(([symbol, name]) => {
  const node = source.binding(42553, symbol); let element;
  walk(node, n => { if (n.type === 'CallExpression' && n.callee.property?.name === 'createElement' && n.arguments[0]?.value === 'svg') element = n; });
  if (!element) throw Error(`Missing OLED ${name} SVG`);
  const bytes = Buffer.from(serializeSvg(element) + '\n');
  const output = `assets/synapse/oled-691-${name}.svg`;
  if (process.argv.includes('--assets')) fs.writeFileSync(output, bytes);
  else if (!fs.readFileSync(output).equals(bytes)) throw Error(`Stale ${output}`);
  const receipt = source.receipt(42553, node);
  return {source: receipt.path, output, source_sha256: receipt.sha256, sha256: hash(bytes),
    source_offset: receipt.offset, source_end: receipt.end, source_kind: 'inline_svg_literal',
    transform: 'Static literal createElement SVG serialization'};
});
const nativeFiles = ['src/features/source_controls/oled_presets.rs',
  'src/features/source_controls/oled_home_cards.rs', 'src/features/source_controls/oled_page.rs'];
const native = nativeFiles.map(path => ({path, text: fs.readFileSync(path, 'utf8')}));
for (const token of ['.grid_cols(3)', '.mt(surface::css(30.))', '.child(t(self.kind.description()))',
  '.title(t(kind.editor_title()))', 'cropped_preview_scaled(', 'if hovered { 1.1 } else { 1. }',
  'rgba(0x5f5f5f4d)', 'rgba(0x00000080)', 'cx.stop_propagation()',
  '"synapse/oled-691-replace.svg"', '"synapse/oled-691-reset.svg"',
  'item.src = None', 'item.local_crop = None', 'last_enabled && enabled']) {
  if (!native[0].text.includes(token)) throw Error(`Missing native preset contract: ${token}`);
}
for (const token of ['OLED_REQUIRE_SYNAPSE_RUNNING_TOOLTIP',
  'OLED_HOME_SCREEN_DISPLAY_TURN_OFF_BLE_MODE_TOOLTIP', 'if hovered {',
  '.on_hover(window.listener_for(&hover']) {
  if (!native[1].text.includes(token)) throw Error(`Missing native home contract: ${token}`);
}
const report = {product_id: 691, bindings, localization,
  css: {path: cssFile, sha256: hash(css), rules}, assets,
  native: native.map(({path, text}) => ({path, sha256: hash(text)})),
  corrected: ['Source animation/image dialog titles and descriptions',
    'Three-column preset grid, 20px gaps and 30px top margin',
    '232x64 content, source border cascade, 10% disabled preview and centred 1.1 hover scale',
    'Hover toolbar moves enabled switch, replace and custom-only reset into each card',
    'Last-enabled invariant, isolated drafts and real native import/crop/Apply are retained',
    'Home card body hover also updates its title; source requires-Synapse and BLE card tooltip text'],
  limitations: ['Tooltip content is verified; the shared native tooltip timing/placement is not source portal parity.',
    'Animation transfer estimates need processed size; original imported byte counts are not substituted.',
    'Emote/Banner/System editors, actual service loading/language downloads and device writes remain unavailable.',
    'No application or tests ran; runtime image pixels, hover and focus behavior remain unmeasured.']};
const target = 'docs/re/oled-preset-cards-current-evidence.json';
const output = JSON.stringify(report, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (fs.readFileSync(target, 'utf8') !== output) throw Error('Stale OLED preset/card receipt');
} else fs.writeFileSync(target, output);
console.log('Verified current 691 OLED preset/card bindings, localization and CSS.');
