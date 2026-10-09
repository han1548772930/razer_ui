// Current product content audit. All vendor JS is parsed as syntax/data only.
// CSS equality is deliberately scoped to the outer body, never whole-page parity.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const acorn = require('acorn');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const key = node => node?.name ?? node?.value;
const assert = (condition, message) => { if (!condition) throw Error(message); };
function walk(node, visit) {
  if (!node?.type || visit(node) === false) return;
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) value.forEach(child => walk(child, visit));
    else if (value?.type) walk(value, visit);
  }
}
function sourcePath(pid, value) {
  assert(value.includes('static/'), `Not a product static resource: ${value}`);
  return `.ref/devices/${pid}/${value.slice(value.indexOf('static/'))}`;
}
function receipt(file, text, node) {
  return {path: file, sha256: sha(text), offset: node.start, end: node.end,
    source: text.slice(node.start, node.end)};
}
function cssRules(pid, manifest) {
  return [...new Set(Object.values(manifest.files))]
    .filter(file => file.includes('static/css/') && file.endsWith('.css'))
    .flatMap(value => {
      const file = sourcePath(pid, value), text = read(file), digest = sha(text);
      return parseCSS(text).map(rule => ({path: file, sha256: digest, ...rule}));
    });
}
const families = ['mouse', 'keyboard', 'audio', 'gamepad', 'accessory_system', 'system'];
const selectors = ['body,html', '.body-wrapper', '.body-widgets', '.body-widgets .widget',
  '.widget .titleRow .title', '.widget .content', '.h1-body'];
const common = [], inventories = [];
for (const family of families) {
  const dataFile = `crates/razer-pages/src/features/${family}_products_data.json`;
  const products = JSON.parse(read(dataFile));
  inventories.push({family, data: dataFile, products: products.map(p => ({
    product_id: p.product_id,
    pages: p.pages.map(page => typeof page === 'string' ? page : page.key),
    content_parity: 'not established by this body-only survey',
  }))});
  for (const product of products) {
    const pid = product.product_id;
    const manifestFile = `.ref/devices/${pid}/asset-manifest.json`;
    const manifest = JSON.parse(read(manifestFile));
    let cssResource = manifest.files['main.css'];
    if (!cssResource) {
      assert(pid === 691, `New unreviewed lazy CSS product ${pid}`);
      cssResource = Object.values(manifest.files).find(file => file.endsWith('/5171.1330bdc6.chunk.css'));
      assert(cssResource, '691 current lazy base stylesheet missing');
      // Its root mount and miniCssF mapping are independently checked by
      // audit-keyboard-691-current.cjs; this survey only checks declarations.
    }
    const cssFile = sourcePath(pid, cssResource), css = read(cssFile);
    const rules = parseCSS(css).filter(r => !r.conditions.length && selectors.includes(r.selector));
    const declarations = selector => Object.fromEntries(rules.filter(r => r.selector === selector)
      .flatMap(r => r.properties.map(p => [p.property, p.value])));
    const body = declarations('body,html'), wrapper = declarations('.body-wrapper');
    assert(body['font-family'] === 'Roboto,sans-serif' && body['font-size'] === '16px'
      && body.color === '#ccc' && body['background-color'] === '#222', `Body typography differs for ${pid}`);
    assert(wrapper.padding === '10px 20px 20px' && wrapper['min-width'] === '600px'
      && wrapper.width === '100%', `Body geometry differs for ${pid}`);
    assert(declarations('.body-widgets .widget')['font-size'] === '14px', `Widget font differs for ${pid}`);
    let jsFile = sourcePath(pid, manifest.files['main.js']), js = read(jsFile);
    // Read the actual shared body props as an AST object. This establishes the
    // current component's class contract, not every route's complete mount tree.
    let match = /id:"body-wrapper"/.exec(js);
    if (!match) {
      for (const value of [...new Set(Object.values(manifest.files))]) {
        if (!value.includes('static/js/') || !value.endsWith('.js')) continue;
        const file = sourcePath(pid, value), text = read(file);
        const candidate = /id:"body-wrapper"/.exec(text);
        if (candidate) { jsFile = file; js = text; match = candidate; break; }
      }
    }
    assert(match, `No current shared body class ${pid}`);
    const begin = js.lastIndexOf('{', match.index);
    const props = acorn.parseExpressionAt(js, begin, {ecmaVersion: 'latest'});
    assert(props.type === 'ObjectExpression' && props.end > match.index, `Unexpected body props ${pid}`);
    common.push({family, product_id: pid, status: 'outer body declarations verified',
      local_dispatch_exception: pid === 182 ? 'original DeviceWorkspace adapter, not MouseProductWorkspace' : null,
      manifest: {path: manifestFile, sha256: sha(read(manifestFile))},
      stylesheet: {path: cssFile, sha256: sha(css), rules},
      body_component_props: receipt(jsFile, js, props)});
  }
}

const nommo = [];
for (const [pid, names] of [[1303, ['wm', 'zm', 'tP', '_P', 'Ym', 'Wm', 'Yl']],
    [1304, ['kM', 'KM', 'op', 'ip', 'rp', 'sp', 'AR']]]) {
  const manifest = JSON.parse(read(`.ref/devices/${pid}/asset-manifest.json`));
  const file = sourcePath(pid, manifest.files['main.js']), text = read(file);
  const ast = acorn.parse(text, {ecmaVersion: 'latest'}), declarations = [];
  walk(ast, node => {
    if (node.type === 'VariableDeclarator' && node.id.type === 'Identifier' && node.init)
      declarations.push({name: node.id.name, node: node.init});
    if (['ClassDeclaration', 'FunctionDeclaration'].includes(node.type) && node.id)
      declarations.push({name: node.id.name, node});
  });
  const bindings = names.map(name => {
    const matches = declarations.filter(d => d.name === name && d.node.start > 4000000);
    assert(matches.length === 1, `Ambiguous current binding ${pid}/${name}`);
    return {symbol: name, ...receipt(file, text, matches[0].node)};
  });
  const page = bindings[0].source, brightness = bindings[3].source, off = bindings[5].source;
  assert(page.includes('direction:"left"') && page.includes('direction:"right"')
    && page.indexOf(names[2]) < page.indexOf(names[4]), `Nommo column mount order ${pid}`);
  assert(brightness.includes('hasSwitch:!0') && brightness.includes('min:0,max:100')
    && brightness.includes('0===e&&this.setState'), `Nommo brightness contract ${pid}`);
  const renderedOff = off.slice(off.lastIndexOf('render(){'));
  assert(renderedOff.includes('id:"checkDisplay"') && !renderedOff.includes('idleMinutes')
    && !renderedOff.includes('isIdleEnabled'), `Unexpected mounted idle controls ${pid}`);
  const data = JSON.parse(read('crates/razer-pages/src/features/audio_products_data.json')).find(p => p.product_id === pid);
  const offControls = data.pages.find(p => p.key === 'TAB_LIGHTING').sections
    .find(s => s.title === 'SWITCH_OFF_LIGHTING_HEADER').controls;
  assert(offControls.length === 1 && offControls[0].path.endsWith('/isDisplayOn')
    && offControls[0].enabled_by === '/profile/brightness/isEnabled', `Nommo native display-off descriptor ${pid}`);
  nommo.push({product_id: pid, reviewed_page: 'TAB_LIGHTING',
    reviewed_components: 'left-column brightness and display-off controls; right-column effects are still partial',
    bindings, css: cssRules(pid, manifest).filter(r => /^\.slider(?:\b|:|-)|\.check-item|\.widget-switch/.test(r.selector))});
}

const cameras = [];
for (const pid of [3592, 3594, 3595, 3596]) {
  const manifest = JSON.parse(read(`.ref/devices/${pid}/asset-manifest.json`));
  const file = sourcePath(pid, manifest.files['main.js']), text = read(file);
  const classUse = [...text.matchAll(/advanced-camera-container/g)].find(match =>
    text.slice(match.index - 50, match.index).includes('extraClass:'));
  assert(classUse, `No actual camera root class ${pid}`);
  const begin = text.lastIndexOf('{', classUse.index);
  const props = acorn.parseExpressionAt(text, begin, {ecmaVersion: 'latest'});
  const rootProps = receipt(file, text, props);
  assert(rootProps.source.includes('children:[this.renderView(),')
    && rootProps.source.includes('extraStyle:'), `Camera/video sibling order ${pid}`);
  const rules = cssRules(pid, manifest).filter(r => ['body,html', '.body-wrapper',
    '.advanced-camera-container', '.advanced-camera-container .camera-container',
    '.advanced-camera-container .video-container', '.advanced-camera-container .video-container video',
    '.advanced-camera-container .camera-container .camera-divider',
    '.advanced-camera-container .wrapper-container .header-wrapper .function-name'].includes(r.selector));
  const rootRule = rules.find(r => r.selector === '.advanced-camera-container');
  const columnRule = rules.find(r => r.selector === '.advanced-camera-container .camera-container');
  assert(rootRule.declarations.includes('padding:0') && rootRule.declarations.includes('flex-direction:row'), `Camera root CSS ${pid}`);
  assert(columnRule.declarations.includes('width:400px') && columnRule.declarations.includes('padding:27px 20px'), `Camera column CSS ${pid}`);
  cameras.push({product_id: pid, reviewed: 'root split and settings-column geometry; individual control parity remains incomplete',
    root_props: rootProps, css: rules});
}
const localFiles = ['crates/razer-pages/src/features/product_surface.rs', ...families.map(f => `crates/razer-pages/src/features/${f}_products.rs`),
  'crates/razer-pages/src/features/audio_nommo.rs', 'crates/razer-pages/src/features/audio_products_data.json', 'crates/razer-pages/src/features/source_controls.rs',
  'crates/razer-pages/src/features/source_workspace.rs', 'tools/prepare-audio-products.cjs'];
const local = Object.fromEntries(localFiles.map(file => [file, sha(read(file))]));
const body = read('crates/razer-pages/src/features/product_surface.rs');
for (const token of ['.pt(surface::css(10.))', '.px(surface::css(20.))', '.pb(surface::css(20.))',
  '.font_family("Roboto")', '.font_weight(FontWeight::NORMAL)', '.text_size(surface::css(16.))'])
  assert(body.includes(token), `Missing local body contract ${token}`);
const nativeNommo = read('crates/razer-pages/src/features/audio_nommo.rs');
assert(nativeNommo.includes('surface::panel_with_title_switch') && nativeNommo.includes('surface::check_item')
  && !nativeNommo.includes('IDLE_FOR_MIN') && nativeNommo.includes('"isEnabled": value != 0'), 'Nommo native behavior drift');
const sourceControls = read('crates/razer-pages/src/features/source_controls.rs');
assert(!sourceControls.includes('.id("camera-preview-unavailable")')
  && sourceControls.includes('.id("source-camera-video")')
  && sourceControls.includes('.id("source-camera-settings-column")'), 'Camera region split drift');
const result = {schema_version: 1, date: '2026-10-05',
  method: 'Current manifests/CSS and AST data only. No application, build, test, downloaded JS or DLL executed.',
  scope: 'This receipt proves the listed component contracts; it does not certify whole-page equality.',
  scanner_sha256: sha(fs.readFileSync(__filename)), inventory: inventories, common_body: common,
  nommo_lighting: nommo, camera_roots: cameras,
  remaining: ['All per-page content not explicitly listed as reviewed',
    'Original DeviceWorkspace adapters (including 182/653/777 and mouse mats)',
    'Legacy-camera 3587/3589/3590 and accessory SourceControls content',
    'CSS normal line-height versus native font metrics', 'Nommo quick/advanced effects and service-derived nanoLeaf state',
    'Camera preview/transport/error/tutorial states and individual control order/types/styles',
    'Hover thumb background transition and subpixel browser/native rasterization'],
  local};
const output = JSON.stringify(result, null, 2) + '\n';
const target = 'docs/re/product-content-current-evidence.json';
if (process.argv.includes('--check')) assert(read(target) === output, 'Stale product content receipt');
else fs.writeFileSync(path.join(root, target), output);
console.log(`Product content: ${common.length} family data rows; ${common.filter(p => p.status === 'outer body declarations verified').length} body CSS contracts; ${nommo.length} Nommo left columns; ${cameras.length} camera roots. Whole-page parity remains incomplete.`);
