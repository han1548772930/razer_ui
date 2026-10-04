// Static product-691 GIF worker dependency and message-contract audit.
// Reference JS is parsed with Acorn; no reference module or WASM is executed.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const {isDeepStrictEqual} = require('util');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file));
const json = file => JSON.parse(read(file));
const resources = json('docs/re/oled-worker-resources-current-evidence.json');
const prefix = '/synapse/products/691/ui/';
const base = '.ref/devices/691/';
const required = new Map([
  ['gifWorker.js', 'js'], ['static/js/9537.835bd6f0.chunk.js', 'js'],
  ...Array.from({length: 6}, (_, index) => [`static/media/${index + 1}_15fps.gif`, 'gif']),
  ['static/media/lets-go-animated_232x64.gif', 'gif'], ['static/media/magick.wasm', 'wasm'],
  ['gifWorker.91a1316a.chunk.js.map', 'sourcemap'], ['9537.835bd6f0.chunk.js.map', 'sourcemap'],
]);
if (resources.product_id !== 691 || resources.manifest !== `${base}asset-manifest.json`
    || resources.manifest_http_receipt !== `${resources.manifest}.http.json`
    || resources.resources.length !== required.size)
  throw Error('Unexpected product-691 resource inventory');
if (hash(read(resources.manifest)) !== resources.manifest_sha256)
  throw Error('OLED resource manifest receipt is stale');
const manifest = json(resources.manifest);
const manifestHttp = json(resources.manifest_http_receipt);
if (manifestHttp.source_url !== `https://apps.razer.com${prefix}asset-manifest.json`
    || manifestHttp.body_file !== 'asset-manifest.json' || manifestHttp.result !== 'ok'
    || manifestHttp.http_status !== 200 || manifestHttp.validation?.valid !== true
    || manifestHttp.sha256 !== resources.manifest_sha256
    || manifestHttp.bytes !== read(resources.manifest).length)
  throw Error('OLED manifest HTTP/body receipt mismatch');
const observed = new Set();
for (const resource of resources.resources) {
  if (required.get(resource.manifest_key) !== resource.kind || observed.has(resource.manifest_key))
    throw Error(`Unexpected or duplicate OLED resource ${resource.manifest_key}`);
  observed.add(resource.manifest_key);
  const declared = manifest.files[resource.manifest_key];
  if (typeof declared !== 'string' || !declared.startsWith(prefix)
      || resource.source_url !== `https://apps.razer.com${declared}`
      || resource.path !== `${base}${declared.slice(prefix.length)}`
      || path.posix.normalize(resource.path) !== resource.path
      || resource.http_receipt !== `${resource.path}.http.json`)
    throw Error(`OLED dependency does not match its manifest URL/path: ${resource.manifest_key}`);
  const http = json(resource.http_receipt);
  const ok = resource.result === 'ok' && resource.http_status === 200;
  const absent = resource.kind === 'sourcemap' && resource.result === 'not_found'
    && resource.http_status === 404;
  if (!ok && !absent)
    throw Error(`Unavailable required dependency ${resource.path}`);
  const body = `${resource.path}${ok ? '' : '.response'}`;
  if (resource.body_path !== body || http.body_file !== path.posix.basename(body))
    throw Error(`OLED receipt points to a different body: ${resource.path}`);
  const bytes = read(body);
  for (const field of ['source_url', 'result', 'http_status', 'fetched_at_utc', 'sha256', 'bytes', 'validation']) {
    if (!isDeepStrictEqual(http[field], resource[field]))
      throw Error(`Stale OLED dependency ${field} receipt ${resource.path}`);
  }
  if (hash(bytes) !== resource.sha256 || bytes.length !== resource.bytes
      || (ok && (resource.validation?.valid !== true || resource.validation.kind !== resource.kind))
      || (absent && resource.validation !== null))
    throw Error(`OLED body/hash/validation mismatch ${resource.path}`);
}
const scripts = resources.resources.filter(resource => resource.kind === 'js');
const modules = new Map();
const scriptSources = new Map();
for (const script of scripts) {
  const source = read(script.path).toString('utf8');
  scriptSources.set(script.path, source);
  walk(acorn.parse(source, {ecmaVersion: 'latest'}), node => {
    if (node.type !== 'Property' || ![71568, 64175, 29623].includes(key(node.key))
        || !/Function/.test(node.value.type)) return;
    if (modules.has(key(node.key))) throw Error(`Duplicate worker module ${key(node.key)}`);
    modules.set(key(node.key), {module_id: key(node.key), path: script.path,
      offset: node.value.start, end: node.value.end,
      sha256: hash(source.slice(node.value.start, node.value.end)),
      source: source.slice(node.value.start, node.value.end)});
  });
  const sourceMap = /sourceMappingURL=([^\s]+)/.exec(source)?.[1];
  if (!resources.resources.some(resource => resource.kind === 'sourcemap'
      && resource.path === path.posix.join(path.posix.dirname(script.path), sourceMap || '')))
    throw Error(`Worker source map declaration missing: ${script.path}`);
}
const worker = modules.get(71568);
if (!worker || !modules.has(64175) || !modules.has(29623))
  throw Error('Worker or ImageMagick module not found');
const contracts = [
  'var o=t(64175)', 'new URL(t(29623),t.b)',
  'self.onmessage=async', 'r.originalData', 'r.animationDelay', 'r.config',
  'await(0,o.ii)(s)', 'o.Yu.readCollection(t,o.qX.Gif', 'e.coalesce()',
  'Math.min(e.length,150)', 'e.slice(0,r),20',
  'o.Qo.create(new o.n9("black"),c,l)', 'e.resize(new o.B0(f,p))',
  'r.compositeGravity(e,"Center",new o.bR(g,d))', 'r.crop(new o.B0(h,y,u,b))',
  'r.grayscale()', 'r.resize(new o.B0(232,64))',
  'r.animationDelay=Math.max(e.animationDelay,n)', 't.optimizePlus()',
  'type:"progress",progress:s', 'type:"complete",payload:{gifData:o,size:s,frames:r}',
  'type:"error",error:v.message',
  'type:"error",error:"No data received"', 'type:"error",error:"Failed to read GIF frames"',
];
for (const token of contracts) {
  if (!worker.source.includes(token)) throw Error(`Worker contract changed: ${token}`);
}
const wasm = resources.resources.find(resource => resource.kind === 'wasm');
if (!modules.get(29623).source.includes(path.posix.basename(wasm.path)))
  throw Error('ImageMagick WASM dependency is not the manifest resource');
const workerBundle = scriptSources.get(worker.path);
for (const token of ['t.O(void 0,[9537],()=>t(71568))', 'var e={8609:1}', 'importScripts(t.p+t.u(r))',
  't.u=e=>"static/js/"+e+".835bd6f0.chunk.js"', 't.p="/synapse/products/691/ui/"']) {
  if (!workerBundle.includes(token)) throw Error(`Worker dependency loader changed: ${token}`);
}
const pages = json('docs/re/keyboard-product-pages.json');
const page = pages.products.find(product => product.product_id === 691).pages.find(page => page.key === 'OLED');
const pageSource = read(page.path).toString('utf8');
if (hash(pageSource) !== page.sha256) throw Error('OLED page source changed');
const crop = page.components.find(component => component.symbol === 'be');
const zoom = page.components.find(component => component.symbol === 'xe');
for (const component of [crop, zoom]) {
  if (pageSource.slice(component.offset, component.end) !== component.source)
    throw Error('Stale OLED crop/zoom source receipt');
}
for (const token of ['15===d?6:3', 'new URL(n.p+n.u(8609),n.b)',
  'originalData:i,config:n,animationDelay:y', 'p.current.terminate()',
  'if(!x){if(b(!0),"animation"===u)', 'const e=i.gifData,t=i.size;b(!1),o(e,t)',
  '"error"===n', 'p.current.onerror=e=>', 'outputType:"base64",outputWidth:232,outputHeight:64',
  'onCancel:j,onApply:C,applyBtnName:"CROPPER_CROP_BTN",loading:x']) {
  if (!crop.source.includes(token)) throw Error(`Crop worker bridge changed: ${token}`);
}
for (const token of ['min:1,max:10,step:1,value:r', 'r>1&&(o(r-1),t(-.1))',
  'r<10&&(o(r+1),t(.1))', 'e<r&&t((e-11)/10),e>r&&t(e/10),o(e)', 'o(1),n()']) {
  if (!zoom.source.includes(token)) throw Error(`Crop zoom control changed: ${token}`);
}
const cropperAudit = json('docs/re/oled-crop-language-current-evidence.json');
const cropperZoom = cropperAudit.cropper_zoom;
if (cropperZoom.path !== '.ref/devices/691/static/js/3725.ac580dd5.chunk.js')
  throw Error('Cropper ratio receipt is not from the current product dependency');
const cropperSource = read(cropperZoom.path).toString('utf8');
if (hash(cropperSource) !== cropperAudit.source_files.find(file => file.path === cropperZoom.path)?.sha256
    || cropperSource.slice(cropperZoom.offset, cropperZoom.end) !== cropperZoom.source
    || hash(cropperZoom.source) !== cropperZoom.sha256)
  throw Error('Stale Cropper ratio source receipt');
let ratioVerified = false;
walk(acorn.parse(cropperSource, {ecmaVersion: 'latest'}), node => {
  if (node.type === 'Property' && key(node.key) === 'zoom'
      && node.value.start === cropperZoom.offset && node.value.end === cropperZoom.end
      && cropperZoom.source.includes('1/(1-A):1+A')) ratioVerified = true;
});
if (!ratioVerified) throw Error('Cropper relative zoom formula changed');
const cssPath = '.ref/devices/691/static/css/OLED.a636cf4a.chunk.css';
const css = read(cssPath).toString('utf8');
const zoomRules = parseCSS(css).filter(rule => /zoom-level-slider|animation-crop-controls/.test(rule.selector));
if (!zoomRules.some(rule => rule.selector === '.zoom-level-slider'
    && rule.declarations === 'margin:4px 10px 0;width:150px'))
  throw Error('Crop zoom slider geometry changed');
const native = read('src/features/source_controls/oled_presets.rs').toString('utf8');
const geometry = read('src/features/source_controls/oled_crop.rs').toString('utf8');
for (const token of ['zoom_slider: Entity<SliderState>', 'zoom_level: u8',
  'crop_slider_delta(this.zoom_level, next)', 'this.canvas.zoom_by(delta)',
  'this.zoom_level = 1;', 'slider.set_value(1., window, cx)',
  'Slider::new(&self.zoom_slider)', '(next as f32 - 11.) / 10.', 'next as f32 / 10.',
  'this.canvas = this.initial_canvas']) {
  if (!native.includes(token)) throw Error(`Native crop zoom contract absent: ${token}`);
}
for (const token of ['1. / (1. - delta)', '1. + delta', 'self.height = CROP_HEIGHT',
  'self.left -= (width - self.width) / 2.', 'self.top -= (height - self.height) / 2.']) {
  if (!geometry.includes(token)) throw Error(`Native crop geometry absent: ${token}`);
}
for (const token of ['.accessibility_label(t("ZOOM_OUT"))', '.accessibility_label(t("ZOOM_IN"))',
  '.background_spawn(async move { crop::load_preset(&path, &extension, renderer) })',
  'let parent = cx.weak_entity()', 'this.import_generation != generation',
  'item.size = Some(size)', 'item.local_crop = Some(placement)']) {
  if (!native.includes(token)) throw Error(`Native import/zoom boundary absent: ${token}`);
}
const report = {
  product_id: 691,
  manifest: {path: resources.manifest, http_receipt: resources.manifest_http_receipt,
    sha256: resources.manifest_sha256},
  receipt_checks: ['complete unique manifest-key inventory', 'exact manifest URL and local path',
    'HTTP status/result and captured timestamp', 'exact body filename/path',
    'body byte count and SHA-256', 'resource/HTTP validation agreement'],
  dependencies: resources.resources.map(({validation, ...resource}) => resource),
  modules: [...modules.values()].map(module => module.module_id === 71568 ? module
    : {module_id: module.module_id, path: module.path, offset: module.offset,
      end: module.end, sha256: module.sha256}),
  crop_bridge: {path: page.path, offset: crop.offset, end: crop.end, sha256: hash(crop.source),
    source: crop.source,
    animation_only: true, image_branch: 'shared image cropper, base64 output at 232x64',
    loading: 'Apply sets busy; progress keeps busy; complete/error/onerror clear busy',
    termination: 'complete, worker error message, Cancel and component unmount terminate the worker',
    initialization_error: 'worker onerror clears busy and logs; does not call terminate itself',
    completion: 'forwards processed gifData and byte count to the isolated preset callback'},
  zoom_controls: {path: page.path, offset: zoom.offset, end: zoom.end,
    sha256: hash(zoom.source), source: zoom.source},
  cropper_zoom: cropperZoom,
  zoom_css: {path: cssPath, sha256: hash(css), rules: zoomRules},
  native_zoom: {
    control_positions: [1, 10], step: 1,
    buttons: 'step position by one; call relative zoom with +0.1 or -0.1',
    slider: 'down: (next - 11) / 10; up: next / 10; same position: no change',
    ratio: 'negative delta: multiply by 1 / (1 - delta); nonnegative: multiply by 1 + delta',
    reset: 'restore initial canvas geometry and slider position 1',
    preview_limit: 'viewMode 0: minimum canvas height 64; no 10x magnification cap; left in [-canvasWidth,232] and top in [-canvasHeight,190]. Native extreme-value guard: canvas edges at most 2^40 CSS pixels, above every u32 image initial canvas. Legacy zoom-only saved drafts retain their earlier preview.',
  },
  worker_contract: {
    initialize_once: true, input: ['originalData', 'config', 'animationDelay'],
    crop_geometry: ['containerWidth', 'containerHeight', 'canvasWidth', 'canvasHeight',
      'canvasOffsetLeft', 'canvasOffsetTop', 'cropBoxWidth', 'cropBoxHeight',
      'cropBoxOffsetLeft', 'cropBoxOffsetTop'],
    coalesce_before_limit: true, maximum_frames: 150, progress_batch_frames: 20,
    output_dimensions: [232, 64], background: 'black', grayscale: true,
    delay: 'max(source frame delay, animationFPS === 15 ? 6 : 3), in GIF centiseconds',
    optimize: 'optimizePlus before GIF encoding',
    output: {type: 'complete', payload: ['gifData (data:image/gif;base64)', 'size (encoded bytes)', 'frames']},
  },
  native_scope: 'Original local upload bytes plus reversible crop preview; no worker execution, processed GIF or device transfer.',
  native_import: {background_work: ['local file read', 'base64 encode', 'GPUI image decode and EXIF orientation'],
    completion: 'weak preset-editor entity update, latest file-selection generation and enabled slot rechecked before opening crop',
    local_preview: 'crop uses decoded RenderImage; restored data URLs use a native Asset cache rather than URI/HTTP loading',
    cache_lifetime: 'displayed-image leases share decoded data and remove the cache entry after the last preview disappears or changes content',
    local_guards: '64MiB encoded input and 256MiB resident-RGBA estimate are checked before decoding all supported formats; these are local safeguards, not vendor limits or a hard process-memory bound',
    accessibility: 'zoom buttons use current ZOOM_IN/ZOOM_OUT translations'},
  limitations: [
    'Both production source maps return HTTP 404; current minified modules are parsed directly.',
    'WASM validation covers its header and section bounds, not instruction semantics or equivalence to a native encoder.',
    'Worker uses ImageMagick composite/resize/grayscale/optimization; Pillow preparation is not an equivalent custom-GIF processing implementation.',
  ],
  validation: 'Acorn AST, source slices, manifest URLs, HTTP receipts and SHA-256 only; no downloaded JavaScript or WASM execution.',
};
const target = 'docs/re/oled-worker-current-evidence.json';
const output = JSON.stringify(report, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (read(target).toString('utf8') !== output) throw Error('OLED worker evidence differs from current source');
} else fs.writeFileSync(path.join(root, target), output);
console.log(`OLED worker: ${scripts.length} scripts, ${modules.size} modules, ${resources.resources.length} dependency receipts; static contracts verified.`);
