// Current product-691 CropperJS geometry and native rendering/asset boundaries.
// Parse reference code as Acorn data; never execute vendor JS or native UI.
const fs = require('fs'), path = require('path'), os = require('os'), acorn = require('acorn');
const {walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const json = file => JSON.parse(read(file));
const previous = json('docs/re/oled-crop-language-current-evidence.json');
const cropperPath = '.ref/devices/691/static/js/3725.ac580dd5.chunk.js';
const source = read(cropperPath);
if (hash(source) !== previous.source_files.find(file => file.path === cropperPath)?.sha256)
  throw Error('Cropper dependency differs from the current crop/language receipt');
const required = new Map([
  ['initCanvas', ['this.containerData', 'this.options.viewMode', '3===t', 'this.limitCanvas(!0,!0)', 'this.initialCanvasData=yA({},c)']],
  ['limitCanvas', ['Number(t.minCanvasHeight)||0', 'n.maxWidth=1/0,n.maxHeight=1/0', 'n.minLeft=-n.width,n.minTop=-n.height,n.maxLeft=r.width,n.maxTop=r.height']],
  ['renderCanvas', ['t.left=t.oldLeft', 't.top=t.oldTop', 'this.limitCanvas(!1,!0)', 't.oldLeft=t.left,t.oldTop=t.top']],
  ['initCropBox', ['Number(A.autoCropArea)||.8', 'this.limitCropBox(!0,!0)', 'this.initialCropBoxData=yA({},n)']],
  ['limitCropBox', ['Number(t.minCropBoxWidth)||0', 'i.minLeft=0,i.minTop=0,i.maxLeft=r.width-i.width,i.maxTop=r.height-i.height']],
  ['renderCropBox', ['this.limitCropBox(!1,!0)', 't.oldLeft=t.left,t.oldTop=t.top']],
  ['zoom', ['1/(1-A):1+A', 'this.zoomTo(t.width*A/t.naturalWidth,null,e)']],
  ['zoomTo', ['n.left-=(c-i)/2,n.top-=(B-s)/2', 'n.width=c,n.height=B,this.renderCanvas(!0)']],
  ['moveTo', ['this.options.movable', 'this.renderCanvas(!0)']],
  ['reset', ['this.canvasData=yA({},this.initialCanvasData)', 'this.cropBoxData=yA({},this.initialCropBoxData)']],
  ['change', ['this.pointers', 'this.move(L.x,L.y),Q=!1', 'A.startX=A.endX,A.startY=A.endY']],
]);
const functions = new Map();
walk(acorn.parse(source, {ecmaVersion: 'latest'}), node => {
  if (node.type !== 'Property' || !required.has(key(node.key)) || !/Function/.test(node.value.type)) return;
  const code = source.slice(node.value.start, node.value.end);
  if (!required.get(key(node.key)).every(token => code.includes(token))) return;
  if (functions.has(key(node.key))) throw Error(`Ambiguous Cropper function ${key(node.key)}`);
  functions.set(key(node.key), {name: key(node.key), path: cropperPath, offset: node.value.start,
    end: node.value.end, sha256: hash(code), source: code});
});
for (const name of required.keys()) if (!functions.has(name)) throw Error(`Cropper contract changed: ${name}`);
const page = json('docs/re/keyboard-product-pages.json').products.find(product => product.product_id === 691)
  .pages.find(page => page.key === 'OLED');
const pageSource = read(page.path);
if (hash(pageSource) !== page.sha256) throw Error('Current OLED page changed');
const crop = page.components.find(component => component.symbol === 'be');
if (pageSource.slice(crop.offset, crop.end) !== crop.source) throw Error('Stale mounted crop component');
for (const token of ['height:190,width:"100%"', 'aspectRatio:3.625', 'viewMode:0',
  'cropBoxMovable:!1', 'cropBoxResizable:!1', 'dragMode:"move"', 'minCanvasHeight:64',
  'minCropBoxHeight:64', 'minCropBoxWidth:232', 'autoCropArea:1', 'zoomOnWheel:!1', 'zoomOnTouch:!1']) {
  if (!crop.source.includes(token)) throw Error(`Mounted crop geometry changed: ${token}`);
}
const cssPath = '.ref/devices/691/static/css/OLED.a636cf4a.chunk.css';
const css = read(cssPath), rules = parseCSS(css).filter(rule => /cropper|animation-crop/.test(rule.selector));
for (const [selector, declaration] of [
  ['.animation-crop-body .animation-cropper-wrapper', 'width:232px'],
  ['.cropper-modal', 'background-color:#000;opacity:.5'],
  ['.cropper-face', 'background-color:#fff;left:0;top:0'],
  ['.animation-crop-body .cropper-view-box', 'outline:1px dashed #44d62c;outline-color:#44d62c'],
]) if (!rules.some(rule => rule.selector === selector && rule.declarations === declaration))
  throw Error(`Crop stylesheet changed: ${selector}`);

const nativePath = 'src/features/source_controls/oled_presets.rs';
const geometryPath = 'src/features/source_controls/oled_crop.rs';
const native = read(nativePath), geometry = read(geometryPath);
for (const token of ['canvas: Option<CropCanvas>', 'canvas.top() - CROP_TOP', 'crop::preview_source(image_id.clone(), source)',
  '.id(image_id)', '.id(("oled-crop-image", self.preview.id.0))', 'gpui_kit::hash(&source)',
  'CropCanvas::new(dimensions.width.0 as f32, dimensions.height.0 as f32)',
  'canvas: Some(crop_for_apply.read(cx).canvas)', 'this.canvas = this.initial_canvas',
  'this.drag_position = Some(event.position)', 'window.on_mouse_event(', 'phase.capture()',
  'this.drag_canvas(event, window, cx)', 'this.end_drag(cx)', '.on_mouse_up_out(',
  'event.keystroke.modifiers.shift', 'this.canvas.move_by(delta.0 * step, delta.1 * step)',
  '.grayscale(true)', '.border_dashed()', 'OledColors::crop_face()', '.opacity(0.5)', '.opacity(0.1)',
  'this.import_generation != generation']) {
  if (!native.includes(token)) throw Error(`Native crop wiring absent: ${token}`);
}
for (const token of ['const WIDTH: f32 = 232.', 'const HEIGHT: f32 = 190.', 'const CROP_HEIGHT: f32 = 64.',
  'const CROP_TOP: f32 = (HEIGHT - CROP_HEIGHT) / 2.', '(WIDTH / aspect).min(HEIGHT).max(CROP_HEIGHT)',
  'self.left.clamp(-self.width, WIDTH)', 'self.top.clamp(-self.height, HEIGHT)',
  'self.left -= (width - self.width) / 2.', 'self.top -= (height - self.height) / 2.',
  'self.width *= CROP_HEIGHT / self.height', 'self.height = CROP_HEIGHT',
  'impl Asset for ImportedImageAsset', 'window.use_asset::<ImportedImageAsset>',
  'Image::from_bytes(format, bytes)', '.to_image_data(renderer)', 'preview.size(0)', 'decode_base64(payload)?',
  'window.use_keyed_state(id.clone()', 'cx.on_release(', 'cx.remove_asset::<ImportedImageAsset>',
  'leases.remove(&this.source)', 'const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024',
  'const MAX_DECODED_ESTIMATE: u64 = 256 * 1024 * 1024',
  'const MAX_CANVAS_EDGE: f32 = 1_099_511_627_776.', 'validate_media(&bytes, format)?',
  'file.take(MAX_INPUT_BYTES as u64 + 1)', 'fn gif_layout(', 'fn jpeg_dimensions(',
  'dimensions.is_some_and(|previous| previous != (width, height))', 'u64::from(dib) + 14 > bytes.len() as u64',
  'frame.checked_mul(frames)', '.try_reserve_exact(', 'preview.frame_count() == 0',
  'preview.size(index) != size', 'resident > MAX_DECODED_ESTIMATE']) {
  if (!geometry.includes(token)) throw Error(`Native crop model/decoder absent: ${token}`);
}
if (geometry.split('validate_media(&bytes, format)?').length !== 3)
  throw Error('Both imported files and restored data URLs must pass media preflight');
if (geometry.includes('.clamp(1., 10.)')) throw Error('Active crop geometry still uses a 10x limit');

// The pinned GPUI implementation supplies frame decoding and EXIF orientation.
// Read the installed crate as source only; do not execute a codec or application.
const cargoHome = process.env.CARGO_HOME || path.join(os.homedir(), '.cargo');
const registry = path.join(cargoHome, 'registry', 'src');
const candidates = fs.readdirSync(registry).map(directory => path.join(registry, directory, 'gpui-pre-0.3.7', 'src'))
  .filter(directory => fs.existsSync(path.join(directory, 'platform.rs')));
if (candidates.length !== 1) throw Error('Expected one pinned GPUI source for decoder inspection');
const platform = fs.readFileSync(path.join(candidates[0], 'platform.rs'), 'utf8');
const assets = fs.readFileSync(path.join(candidates[0], 'asset_cache.rs'), 'utf8');
const app = fs.readFileSync(path.join(candidates[0], 'app.rs'), 'utf8');
const image = fs.readFileSync(path.join(candidates[0], 'elements', 'img.rs'), 'utf8');
for (const token of ['decode_static_image_from_decoder', '.orientation()', 'image.apply_orientation(orientation)',
  'pub fn to_image_data(', 'ImageFormat::Gif =>', 'decoder.into_frames()']) {
  if (!platform.includes(token)) throw Error(`GPUI media decoding source changed: ${token}`);
}
if (!assets.includes('let task = cx.background_executor().spawn(future)'))
  throw Error('GPUI asset background loading contract changed');
for (const token of ['if global_id.is_some()', 'frame_count > 1 && !cx.reduce_motion()',
  'data.delay(state.frame_index)', 'state.frame_index = state.frame_index.min(max_frame_index)']) {
  if (!image.includes(token)) throw Error(`GPUI animated image state changed: ${token}`);
}
if (!app.includes('Pending loads and completed results are cached until')
    || !assets.includes('let state = Rc::downgrade(&state)'))
  throw Error('GPUI asset lifetime/late-result guard changed');
const report = {
  product_id: 691,
  sources: [cropperPath, page.path, cssPath].map(path => ({path, sha256: hash(read(path))})),
  mounted_cropper: {symbol: 'be', path: page.path, offset: crop.offset, end: crop.end, sha256: hash(crop.source)},
  functions: [...functions.values()], css_rules: rules,
  geometry: {
    container: {width: 232, height: 190}, crop_box: {left: 0, top: 63, width: 232, height: 64},
    derivation: 'CSS fixes container width 232. Crop-box minWidth 232 and viewMode-0 container maxWidth 232 fix its width; aspect 3.625 fixes height 64. initCropBox centres it on the centred initial canvas.',
    initial_canvas: 'contain in 232x190, then minCanvasHeight 64 with preserved decoded image aspect; centre in container',
    movement: 'incremental pointer delta; left clamps to [-width,232], top to [-height,190]; pointer origin advances even at a bound',
    zoom: 'relative Cropper factor about current canvas centre; on a minimum-size violation restore old position before sizing to height 64; source has no maximum zoom, native uses the extreme-value safeguard documented below',
    reset: 'restore initial canvas rectangle and slider position 1; clear drag',
    source_derived_examples: [
      {image: [232, 64], canvas: {left: 0, top: 63, width: 232, height: 64}},
      {image: [100, 1000], canvas: {left: 106.5, top: 0, width: 19, height: 190}},
      {image: [1000, 100], canvas: {left: -204, top: 63, width: 640, height: 64}},
      {image: [100, 100], canvas: {left: 21, top: 0, width: 190, height: 190}},
    ],
    example_scope: 'Algebraic examples from the inspected source, not executed native UI tests',
  },
  native: {
    files: [nativePath, geometryPath].map(path => ({path, sha256: hash(read(path))})),
    pointer: 'focus on press; window capture listeners continue outside the canvas; local/up-out release also handles a release before the next frame',
    keyboard: 'focused crop group: arrows move one CSS pixel; Shift+arrows move ten; framework controls keep zoom/reset keyboard access',
    preview: 'grayscale decoded image in full canvas; 50% black masks outside crop and 10% white face overlay; dashed outline and corner marks',
    persistence: 'store width/height/left/top under local_crop.canvas; preset/home preview subtracts cropTop 63; legacy zoom-only drafts retain the previous placement',
    media: 'read and decode on background executor using existing GPUI codecs; crop reuses RenderImage; saved data URLs use the native Asset cache and a local base64 decoder',
    animation: 'image ElementIds retain frame state; slot/content identity resets a replaced animation; GPUI uses original frame delays while active and respects reduced motion',
    asset_lifetime: 'keyed image leases count simultaneous previews by data URL; the last released lease removes pending or completed asset data; GPUI weak completion state prevents a late result replacing an evicted entry',
    input_guards: {
      source: 'native allocation/layout safeguards, not vendor limits',
      maximum_encoded_bytes: 67108864,
      decoded_rgba_estimate_bytes: 268435456,
      estimate: 'width * height * 4 * frameCount + 128 * frameCount; excludes codec working canvases, compressed data and metadata, so not a hard process-memory ceiling',
      preflight: ['PNG complete IHDR dimensions', 'JPEG SOF dimensions before SOS, reject conflicting dimensions',
        'BMP complete DIB header bounds, positive width and absolute height', 'GIF logical canvas, image bounds, palettes/sub-block bounds and all frame records'],
      decoding: 'preflight does not execute compressed media; malformed compressed data remains subject to GPUI codec errors. GPUI can skip individually damaged GIF frames and errors if none decode',
      decoded_validation: 'before returning any RenderImage, require nonzero dimensions, a consistent canvas size across all frames and resident frame buffers within the estimate; this cannot bound decoder transient memory',
      maximum_canvas_edge_css: 1099511627776,
      initial_canvas: '2^40 exceeds the maximum 2^38-width initial canvas produced by a u32 image aspect and minCanvasHeight 64',
      failure: 'an import error preserves the draft; malformed restored media returns a cached image error and remains replaceable/resettable; invalid saved canvas falls back to the legacy placement',
    },
    gpui_decoder: {package: 'gpui-pre', version: '0.3.7', platform_sha256: hash(platform), asset_cache_sha256: hash(assets),
      app_sha256: hash(app), image_element_sha256: hash(image)},
  },
  boundaries: ['No application, tests, downloaded JS/WASM/DLL or image codec was executed by this audit.',
    'Native image/GIF crop encoding and processed byte counts remain unimplemented; only original bytes and preview geometry are stored.',
    'No hardware transport, source ImageMagick equivalence or measured runtime pixel parity is claimed.'],
  validation: 'Acorn AST, exact mounted-source slices, CSS, SHA-256 and native/GPUI source inspection only.',
};
const target = 'docs/re/oled-canvas-current-evidence.json';
const output = JSON.stringify(report, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (read(target) !== output) throw Error('OLED canvas evidence differs from current source');
} else fs.writeFileSync(path.join(root, target), output);
console.log(`OLED canvas: ${functions.size} Cropper functions, fixed 232x190/232x64 geometry and native media/pointer boundaries verified statically.`);
