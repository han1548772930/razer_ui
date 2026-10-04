// Current Macro key-bind flow, read as AST/CSS data only. Never execute app JS.
const fs = require('node:fs');
const path = require('node:path');
const {Source, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const source = new Source('synapse/macro');
const id = 21700;
const scope = source.module(id);
const components = Object.fromEntries(['U', 'M', 'I', 'E'].map(name =>
  [name, source.receipt(id, source.binding(id, name))]));
const localeKeys = Object.fromEntries(['e3D', 'BnG', 'EmI', 'V4O', 'P6l', 'Kgv', 'xjw'].map(name =>
  [name, source.literal(37927, source.exported(37927, name))]));
const css = ['main.9ea5d7e7.css', '1700.1adda236.chunk.css'].map(name => {
  const file = `.ref/applications/synapse/macro/static/css/${name}`;
  const text = fs.readFileSync(path.join(root, file), 'utf8');
  const rules = parseCSS(text).filter(rule => /KeyBindContainer_|ProductDevice_|AddItemDropdown_|body_additem_wrapper|(?:^|,)\.?backdrop|\.dashboard.*\.box|\.box-group/.test(rule.selector));
  return {path: file, sha256: hash(text), rules};
});
const mousePath = '.ref/devices/182/static/js/main.db20a7c4.js';
const mouseText = fs.readFileSync(path.join(root, mousePath), 'utf8');
const moduleStart = mouseText.indexOf('1368:(e,E,a)=>');
if (moduleStart < 0) throw Error('Missing current 182 group module');
const moduleNode = acorn.parseExpressionAt(mouseText, moduleStart + '1368:'.length, {ecmaVersion: 'latest'}).expressions?.[0]
  ?? acorn.parseExpressionAt(mouseText, moduleStart + '1368:'.length, {ecmaVersion: 'latest'});
const groupsNode = moduleNode.body.body.find(statement => statement.type === 'VariableDeclaration')?.declarations[0]?.init;
function literal(node) {
  if (node?.type === 'Literal') return node.value;
  if (node?.type === 'UnaryExpression' && node.operator === '!') return !literal(node.argument);
  if (node?.type === 'ArrayExpression') return node.elements.map(literal);
  if (node?.type === 'ObjectExpression') return Object.fromEntries(node.properties.map(property =>
    [property.key.name ?? property.key.value, literal(property.value)]));
  throw Error(`Nonliteral 182 group data: ${node?.type}`);
}
const mouseGroups = literal(groupsNode);
if (mouseGroups[0].group.buttonList.length !== 8) throw Error('Changed current 182 physical inputs');
for (const proof of ['class KP extends', 'this.drawLine(0,this.props.buttonList[0],114,53,!0)', '"macro"===this.props.displayMode?null:', '"macro"===a)return(0,gt.jsx)(BM,{macro:t})']) {
  if (!mouseText.includes(proof)) throw Error('Changed current 182 Macro root: ' + proof);
}
const mouseReceipt = {path: mousePath, sha256: hash(mouseText), group_module: 1368,
  offset: groupsNode.start, end: groupsNode.end, groups: mouseGroups};
function productClass(name) {
  const start = mouseText.indexOf(`class ${name} extends`);
  if (start < 0) throw Error('Missing current product class: ' + name);
  const node = acorn.parseExpressionAt(mouseText, start, {ecmaVersion: 'latest'});
  return {path: mousePath, offset: node.start, end: node.end, source: mouseText.slice(node.start, node.end)};
}
const playbackStart = mouseText.indexOf('9267:') + '9267:'.length;
const playbackParsed = acorn.parseExpressionAt(mouseText, playbackStart, {ecmaVersion: 'latest'});
const playbackModule = playbackParsed.expressions?.[0] ?? playbackParsed;
const declarations = new Map(playbackModule.body.body.flatMap(statement => statement.declarations ?? []).map(d => [d.id.name, d.init]));
const playback = {normal: literal(declarations.get('je')), wheel: literal(declarations.get('Ze'))};
const playbackReceipt = {path: mousePath, module: 9267,
  arrays: Object.fromEntries(['je','Ze','xe','Xe','qe'].map(name => {
    const node = declarations.get(name);
    return [name, {offset: node.start, end: node.end, value: literal(node)}];
  }))};
const macroKeyPath = '.ref/devices/182/static/js/MapMacroKey.639e3304.chunk.js';
const macroKeyText = fs.readFileSync(path.join(root, macroKeyPath), 'utf8');
acorn.parse(macroKeyText, {ecmaVersion: 'latest'});
const productCSSPath = '.ref/devices/182/static/css/main.48c20423.css';
const productCSS = fs.readFileSync(path.join(root, productCSSPath), 'utf8');
const productRules = parseCSS(productCSS).filter(rule => /key-config|keymap-head|display-name|dropdown-playback|config-btn|config-wrapper|config-block|hyper-switch/.test(rule.selector));
const assets = [
  ['binding-more.svg', 'icon_more_g.32e1e984.svg'],
  ['binding-close.svg', 'icon_close.4f578909.svg'],
  ['../profiles-back.svg', 'icon_back_arrow.b39e4841.svg'],
  ['../profiles-glow.svg', 'background_glow.43f70cb2.svg'],
].map(([output, original]) => {
  const sourcePath = `.ref/applications/synapse/macro/static/media/${original}`;
  const outputPath = path.posix.normalize(`assets/synapse/macro/${output}`);
  const a = fs.readFileSync(path.join(root, sourcePath));
  const b = fs.readFileSync(path.join(root, outputPath));
  if (!a.equals(b)) throw Error('Macro binding asset differs: ' + outputPath);
  return {source: sourcePath, output: outputPath, sha256: hash(a), byte_equal: true,
    declaration: original.startsWith('icon_close.') ? '1700 CSS url(), not the media manifest' : 'current Macro JS/CSS'};
});
const nativeCatalogs = ['keyboard', 'mouse'].map(kind => {
  const file = `src/features/${kind}_products_data.json`;
  const bytes = fs.readFileSync(path.join(root, file));
  const products = JSON.parse(bytes);
  return {path: file, sha256: hash(bytes),
    products: products.map(product => {
      const values = kind === 'keyboard' ? product.keys :
        (product.groups ?? []).flatMap(group => (group.group ?? group).buttonList ?? []);
      const inputs = [...new Map(values.filter(value => value.inputID && Array.isArray(value.functionList))
        .map(value => [value.inputID, value])).values()];
      return {product_id: product.product_id, inputs: inputs.length,
        macro_enabled: inputs.filter(value => value.isEnabled === true && value.functionList.includes('MACRO')).length,
        shapes: product.shapes?.length ?? 0};
    })};
});
const receipt = {
  schema_version: 1,
  method: 'Module-local Acorn AST, static locale exports and CSS with enclosing media conditions; no reference code execution',
  source: {path: scope.file, sha256: hash(source.text(scope.file))},
  components, localeKeys, css, assets, native_catalogs: nativeCatalogs,
  native_boundary: 'Catalog projections are not per-product displayMode root parity. 182 has a dedicated diagram; 653 uses layout-specific shapes. Other keyboards reuse catalog shapes and other mice use a catalog-only presentation. Additional root-specific restrictions remain unaudited.',
  product_182: {...mouseReceipt,
    classes: {KP: productClass('KP'), HM: productClass('HM')}, playback: playbackReceipt,
    mapping: {path: macroKeyPath, sha256: hash(macroKeyText), module: 4369, source: macroKeyText},
    css: {path: productCSSPath, sha256: hash(productCSS), rules: productRules}},
  flow: {
    page: 'currentProfile -> macro.deviceList; null profile renders no page',
    device_source: 'commonReducer.validDevices, enriched from connectedDeviceInfo and current locale',
    device_filter: 'supportMacro OR KEYBOARD/MOUSE/MOUSEPLUSMAT/KEYPAD/SYSTEM OR productId 3907',
    popup: 'Add card -> select device -> DEVICE -> MouseBind iframe; back returns to select; close toggles panel',
    root_query: ['displayMode=macro', 'macro', 'containerId', 'deviceEditionInfo', 'serialNumber'],
    root_pid: 'allRZDevices productId/dongleId resolution',
    remove: '21700 C -> 4173 Zi, payload index and selectedMacro',
    device_disappears: 'selected serialNumber/dongleId missing from validDevices resets category to select',
    pane_width: 1020, compact_max_width: 1028, pane_top: 20,
    header_height: 36, choose_card: [300, 200], binding_card: [290, 230],
    service_boundary: 'Native local associations must not claim service discovery or hardware writes'
  }
};
const target = path.join(root, 'docs/re/macro-bindings-current-evidence.json');
const text = JSON.stringify(receipt, null, 2) + '\n';
const mouseTarget = path.join(root, 'src/features/macro_inputs_182.json');
const mouseData = JSON.stringify(mouseGroups, null, 2) + '\n';
const playbackTarget = path.join(root, 'src/shell/macro_page/bindings/playback_182.json');
const playbackData = JSON.stringify(playback, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (fs.readFileSync(target, 'utf8') !== text) throw Error('Stale Macro bindings receipt');
  if (fs.readFileSync(mouseTarget, 'utf8') !== mouseData) throw Error('Stale Macro input data');
  if (fs.readFileSync(playbackTarget, 'utf8') !== playbackData) throw Error('Stale Macro playback data');
} else { fs.writeFileSync(target, text); fs.writeFileSync(mouseTarget, mouseData); fs.writeFileSync(playbackTarget, playbackData); }
console.log('Current Macro bindings: module 21700, 4 mounted components, locale/CSS receipts');
