// Reproducible current product-691 crop and language contracts. Vendor
// JavaScript is parsed as text; no reference module, worker or DLL is run.
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const acorn = require('acorn');
const {parseCSS} = require('./css-source.cjs');
const {walk, key} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const read = name => fs.readFileSync(path.join(root, name), 'utf8');
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const page = JSON.parse(read('docs/re/keyboard-product-pages.json')).products
  .find(p => p.product_id === 691).pages.find(p => p.key === 'OLED');
const source = read(page.path);
if (hash(source) !== page.sha256) throw Error('Current product-691 OLED source changed');
acorn.parse(source, {ecmaVersion: 'latest'});
const symbols = ['be', 'xe', 'wt', 'Vt', 'jt', 'Ht', 'wi', 'Wi', 'Ui', 'Fi'];
const components = symbols.map(symbol => {
  const c = page.components.find(c => c.symbol === symbol);
  if (!c || source.slice(c.offset, c.end) !== c.source)
    throw Error(`Stale OLED ${symbol} source receipt`);
  return {symbol, path: page.path, offset: c.offset, end: c.end, sha256: hash(c.source)};
});
const component = symbol => page.components.find(c => c.symbol === symbol).source;
for (const token of ['aspectRatio:3.625', 'viewMode:0', 'cropBoxMovable:!1',
  'cropBoxResizable:!1', 'dragMode:"move"', 'minCropBoxHeight:64',
  'minCropBoxWidth:232', 'outputWidth:232,outputHeight:64',
  'new Worker(', 'originalData:i,config:n,animationDelay:y']) {
  if (!component('be').includes(token)) throw Error(`Crop contract changed: ${token}`);
}
for (const token of ['structuredClone(', '.custom=!0', '.src=e', '.size=t']) {
  for (const symbol of ['wt', 'Vt']) {
    if (!component(symbol).includes(token)) throw Error(`Isolated import changed: ${symbol}/${token}`);
  }
}
if (!component('Vt').includes('showCropperInfo:!1')
    || !component('Ht').includes('r&&d&&') || !component('jt').includes('r&&d&&'))
  throw Error('Crop info/reset conditions changed');
for (const token of ['t<127?t:255&~t', 'd===t&&d===b',
  '"progress"===e.oledLoadingReducer.oledLoading.type', 'w||j?"disabled":""',
  '6===C&&n>1&&_.current', 'SET_OLED_LANGUAGE']) {
  if (!component('wi').includes(token)) throw Error(`Language contract changed: ${token}`);
}
if (!component('Wi').includes('"language"===n')
    || !component('Ui').includes('CANCEL_OLED_LANGUAGE_UPDATE'))
  throw Error('Language progress/cancel contract changed');

const product = JSON.parse(read('crates/razer-pages/src/features/keyboard_products_data.json'))
  .find(p => p.product_id === 691);
const mainFile = product.source_files.find(file => /\/main\./.test(file.path));
const main = read(mainFile.path);
if (hash(main) !== mainFile.sha256) throw Error('Current product-691 main source changed');
const reducers = [];
walk(acorn.parse(main, {ecmaVersion: 'latest'}), node => {
  if (node.type !== 'Property' || !['oledLanguageReducer', 'oledLoadingReducer'].includes(key(node.key))
      || !/Function/.test(node.value.type)) return;
  const code = main.slice(node.value.start, node.value.end);
  reducers.push({name: key(node.key), path: mainFile.path, offset: node.value.start,
    end: node.value.end, sha256: hash(code), source: code});
});
if (!reducers.some(r => r.name === 'oledLanguageReducer'
    && r.source.includes('oledLanguageChanged:e.oledLanguageChanged+1')
    && r.source.includes('MW_SET_OLED_LANGUAGE_TO_UI')))
  throw Error('Language middleware reply reducer changed');

const cropperFile = '.ref/devices/691/static/js/3725.ac580dd5.chunk.js';
const cropper = read(cropperFile);
let zoom;
walk(acorn.parse(cropper, {ecmaVersion: 'latest'}), node => {
  if (node.type !== 'Property' || key(node.key) !== 'zoom' || !/Function/.test(node.value.type)) return;
  const code = cropper.slice(node.value.start, node.value.end);
  if (code.includes('this.canvasData') && code.includes('1/(1-A):1+A')) {
    zoom = {path: cropperFile, offset: node.value.start, end: node.value.end,
      sha256: hash(code), source: code};
  }
});
if (!zoom) throw Error('Cropper zoom ratio formula changed');
const cssFile = '.ref/devices/691/static/css/OLED.a636cf4a.chunk.css';
const css = read(cssFile);
const cropRules = parseCSS(css).filter(rule => /animation-crop/.test(rule.selector));
const native = read('crates/razer-pages/src/features/source_controls/oled_presets.rs');
const controls = read('crates/razer-pages/src/features/source_controls.rs');
for (const token of ['local_crop: Option<CropPlacement>', 'item.local_crop = Some(placement)',
  'cropped_preview(', 'restore_oled_custom_presets', 'restore_preset_selection',
  'kind.accepts_data_url(source)', 'target.normalize()']) {
  if (!native.includes(token)) throw Error(`Native crop/restoration branch missing: ${token}`);
}
if (!controls.includes('self.restore_oled_custom_presets(value)'))
  throw Error('Custom payload restore hook is not wired');
const report = {
  product_id: 691,
  source_files: [page.path, mainFile.path, cropperFile, cssFile]
    .map(path => ({path, sha256: hash(read(path))})),
  components, reducers, cropper_zoom: zoom, css_rules: cropRules,
  crop: {
    output_dimensions: [232, 64], aspect_ratio: 3.625, container_height: 190,
    crop_box_movable: false, crop_box_resizable: false, drag_mode: 'move',
    vendor_animation_processing: 'worker 8609 receives original bytes and cropper geometry',
    local_payload: 'original imported data URL and byte count, plus local_crop.canvas rectangle; legacy local_crop.zoom remains readable',
    local_restore: 'matches fixed current-source preset IDs, restores optional payload fields, normalizes selection/zoom',
    cancel_discards_isolated_draft: true,
    image_crop_info_visible: false,
    reset_requires_enabled_custom_item: true,
  },
  language: {
    source_disabled_when: ['deviceReducer.isBle', 'oledLoadingReducer.oledLoading.type === progress'],
    source_display_decode: 'raw < 127 ? raw : 255 & ~raw',
    source_unchanged_condition: 'stagedRaw === storedRaw && stagedRaw === displayedValue',
    source_reply: 'MW_SET_OLED_LANGUAGE_TO_UI increments oledLanguageChanged',
    source_progress: 'oledLoading target language and type progress',
    source_cancel_action: 'CANCEL_OLED_LANGUAGE_UPDATE',
    native_state: 'local selection/apply mirror; middleware download state remains unavailable',
  },
  limitations: [
    'Native viewMode-0 canvas/pan/zoom and media-cache receipts are in oled-canvas-current-evidence.json; rendered pixel parity remains unmeasured.',
    'GIF processing, processed output size, device transfer and language download progress are not fabricated.',
    'Pure restoration regressions are type-checked by cargo check --locked --all-targets; tests and application are not executed.',
  ],
  validation: 'Static Acorn/CSS parsing, source hashes and native wiring checks only.',
};
const target = 'docs/re/oled-crop-language-current-evidence.json';
const output = JSON.stringify(report, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (read(target) !== output) throw Error('OLED crop/language evidence differs from current source');
} else {
  fs.writeFileSync(path.join(root, target), output);
}
console.log(`Verified ${components.length} crop/language components, ${reducers.length} reducers and CropperJS zoom receipt.`);
