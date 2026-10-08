// Current ACTUATION mount, synchronization payload and appearance only.
// Acorn/CSS parse source as data. Never execute reference JS or a DLL.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {Source, walk, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), check = process.argv.includes('--check');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const fileBytes = file => fs.readFileSync(path.join(root, file));
const specifications = JSON.parse(read('src/features/keyboard_actuation_data.json'));
const products = JSON.parse(read('src/features/keyboard_products_data.json'));
const mounts = JSON.parse(read('docs/re/keyboard-product-pages.json'));
function output(file, bytes) {
  bytes = Buffer.isBuffer(bytes) ? bytes : Buffer.from(bytes);
  if (check) {
    if (!fileBytes(file).equals(bytes)) throw Error('Stale ' + file);
  } else {
    fs.mkdirSync(path.dirname(path.join(root, file)), {recursive: true});
    fs.writeFileSync(path.join(root, file), bytes);
  }
}
async function main() {
  const evidence = [], presentation = [];
  let commonIcon;
  for (const spec of specifications) {
    const directory = `.ref/devices/${spec.product_id}`;
    const manifestPath = directory + '/asset-manifest.json', manifest = JSON.parse(read(manifestPath));
    const mounted = mounts.products.find(p => p.product_id === spec.product_id).pages.find(p => p.key === 'ACTUATION');
    const product = products.find(p => p.product_id === spec.product_id);
    const pageText = read(mounted.path);
    if (hash(pageText) !== mounted.sha256) throw Error('Changed current ACTUATION page');
    const pageAST = acorn.parse(pageText, {ecmaVersion: 'latest'}), receipts = [];
    const editors = [];
    walk(pageAST, node => {
      if (node.type === 'ClassDeclaration' && pageText.slice(node.start, node.end).includes('onMakeActuationPointChange')) editors.push(node);
    });
    if (editors.length !== 1) throw Error('Re-audit mounted actuation class ' + spec.product_id);
    const editor = editors[0], editorText = pageText.slice(editor.start, editor.end);
    if (!mounted.components.some(c => c.offset === editor.start && c.end === editor.end && c.source === editorText)) throw Error('Editor not in current mounted component chain');
    const receipt = node => ({path: mounted.path, sha256: hash(pageText), offset: node.start, end: node.end, source: pageText.slice(node.start, node.end)});
    const lockedParents = mounted.components.filter(c => c.source.includes('className:this.props.isFactoryDefaultProfile?"disabled":""'));
    for (const parent of lockedParents) {
      if (pageText.slice(parent.offset, parent.end) !== parent.source) throw Error('Changed factory-profile parent');
      receipts.push({role: 'factory-profile-parent', path: mounted.path, sha256: hash(pageText), offset: parent.offset, end: parent.end, source: parent.source});
    }
    for (const member of ['syncActuationKeys', 'onMakeActuationPointChange']) {
      const nodes = [];
      walk(editor, node => {
        if (node.type === 'AssignmentExpression' && node.left.type === 'MemberExpression' && node.left.object.type === 'ThisExpression' && node.left.property.name === member) nodes.push(node);
      });
      if (nodes.length !== 1) throw Error('Re-audit actuation command ' + member);
      receipts.push({role: member, ...receipt(nodes[0])});
    }
    const sync = receipts.find(r => r.role === 'syncActuationKeys').source;
    if (!sync.includes('0:this.state.makeActuationPointValue,1:this.state.makeActuationPointValue')) throw Error('Changed sync payload');
    if (!/"analogV2"===\w+\.DeviceInfo\.AnalogGenVersion&&\w+<10/.test(editorText)) throw Error('Changed warning gate');
    const rows = [];
    walk(editor, node => {
      if (node.type !== 'CallExpression' || node.arguments[1]?.type !== 'ObjectExpression') return;
      const props = new Map(node.arguments[1].properties.map(p => [p.key.name ?? p.key.value, p.value]));
      if (props.has('syncSettings') && props.has('name') && props.has('onClick') && props.has('disabled')) rows.push(node);
    });
    if (rows.length !== 2) throw Error('Changed sync controls');
    receipts.push(...rows.map(node => ({role: 'sync-row', ...receipt(node)})));
    const buttonSymbol = rows[0].arguments[0].name;
    const buttons = [];
    walk(pageAST, node => {
      if (node.type === 'VariableDeclarator' && node.id.name === buttonSymbol && pageText.slice(node.start, node.end).includes('btn-sync-icon')) buttons.push(node);
    });
    if (buttons.length !== 1) throw Error('Re-audit sync button');
    receipts.push({role: 'sync-button', ...receipt(buttons[0])});
    const button = receipts.at(-1).source;
    if (!button.includes('className:"btn-sync-icon",onClick:')) throw Error('Changed source icon-only click target');

    const source = Object.assign(Object.create(Source.prototype), {directory,
      files: [...new Set(Object.values(manifest.files))].filter(f => f.endsWith('.js')).map(f => directory + '/' + f.slice(2)),
      modules: new Map(), texts: new Map(), parsed: new Set()});
    const deviceInfo = source.binding(78193, source.exported(78193, 'DeviceInfo').name);
    const generation = deviceInfo.properties?.find(p => (p.key.name ?? p.key.value) === 'AnalogGenVersion')?.value;
    if (!generation || source.literal(78193, generation) !== product.config.DeviceInfo.AnalogGenVersion) throw Error('Changed source AnalogGenVersion');
    receipts.push({role: 'warning-generation', ...source.receipt(78193, generation)});
    const mainPath = directory + '/' + manifest.files['main.js'].slice(2), mainText = read(mainPath);
    const mainAST = mainPath === mounted.path ? pageAST : acorn.parse(mainText, {ecmaVersion: 'latest'});
    const reducers = [];
    walk(mainAST, node => {
      if (node.type === 'VariableDeclarator' && node.init?.type === 'ArrowFunctionExpression') {
        const text = mainText.slice(node.start, node.end);
        if (text.includes('ON_SYNC_GLOBAL_ACTUATION') && text.includes('disableActuationMappings') && text.includes('selectedButtonList')) reducers.push(node);
      }
    });
    if (reducers.length !== 1) throw Error('Re-audit sync reducer ' + spec.product_id);
    receipts.push({role: 'sync-reducer', path: mainPath, sha256: hash(mainText), offset: reducers[0].start, end: reducers[0].end, source: mainText.slice(reducers[0].start, reducers[0].end)});
    // Every product independently binds these export getters to its own current module.
    for (const name of ['aC', 'QY', 'hF', 'q$']) {
      const binding = source.exported(3342, name);
      receipts.push({role: 'mapping-helper:' + name, ...source.receipt(3342, source.binding(3342, binding.name))});
    }
    let syncDefaultSecondary = false;
    walk(source.binding(3342, source.exported(3342, 'aC').name), node => {
      const left = node.type === 'AssignmentExpression' ? node.left : null;
      if (left?.type === 'MemberExpression' && left.property.name === 'actuationPoint'
          && left.object.type === 'MemberExpression' && left.object.property.value === 1
          && left.object.object?.property?.name === 'mapping') syncDefaultSecondary = true;
    });
    for (const [index, label] of ['SYNC_SETTINGS_TO_ALL_KEYS', 'SYNC_SETTINGS_TO_SELECTED_KEYS'].entries()) {
      const props = rows[index].arguments[1].properties;
      const name = props.find(p => (p.key.name ?? p.key.value) === 'name')?.value;
      if (name?.type !== 'MemberExpression') throw Error('Re-audit sync label binding');
      const alias = name.property.name, binding = source.exported(54693, alias);
      if (source.literal(54693, binding) !== label) throw Error('Changed source sync label');
      receipts.push({role: 'label:' + label, ...source.receipt(54693, source.binding(54693, binding.name))});
    }
    const css = [];
    for (const relative of [...new Set(Object.values(manifest.files))].filter(f => f.endsWith('.css'))) {
      const file = directory + '/' + relative.slice(2), text = read(file);
      const rules = parseCSS(text).filter(r => /\.btn-sync(?:[ .,:-]|$)|\.btn-col(?:[ ,.]|$)|\.actuation-warning(?:[ ,.]|$)/.test(r.selector) || r.selector === '.disabled' || r.selector === 'body');
      if (rules.length) css.push({path: file, sha256: hash(text), rules});
    }
    const rule = selector => css.flatMap(c => c.rules).find(r => r.selector === selector)?.declarations;
    for (const [selector, fragment] of [['.actuation-warning', 'color:#fd8611;margin-top:20px'], ['.btn-sync', 'font-size:14px;height:27px'], ['.btn-sync-icon', 'background-position-x:7px;background-position-y:-5px']]) {
      if (!rule(selector)?.includes(fragment)) throw Error('Changed appearance ' + selector + ' / ' + spec.product_id);
    }
    const column = css.flatMap(c => c.rules).find(r => r.selector === '.btn-col');
    const padding = column?.properties.find(p => p.property === 'padding')?.value;
    const container = css.flatMap(c => c.rules).find(r => r.selector === '.btn-sync-container');
    const height = container?.properties.find(p => p.property === 'height')?.value;
    if (!['10px 10px 0', '10px 0'].includes(padding) || (height !== undefined && height !== '60px')) throw Error('Re-audit column presentation');
    presentation.push({product_id: spec.product_id, factory_profile_lock: lockedParents.length > 0,
      sync_padding_x: padding === '10px 10px 0' ? 10 : 0, sync_padding_bottom: padding === '10px 0' ? 10 : 0,
      sync_container_height: height === undefined ? null : 60, sync_default_secondary: syncDefaultSecondary});
    const iconRelative = manifest.files['static/media/icon_sync.svg'];
    if (!iconRelative) throw Error('Missing current sync asset manifest');
    const iconPath = directory + '/' + iconRelative.slice(2);
    const origin = JSON.parse(read(manifestPath + '.http.json')).source_url;
    const url = new URL(iconRelative, origin).href;
    if (!fs.existsSync(path.join(root, iconPath))) {
      if (check) throw Error('Missing statically prepared icon ' + iconPath);
      const response = await fetch(url);
      if (!response.ok) throw Error('Resource fetch failed ' + url);
      const bytes = Buffer.from(await response.arrayBuffer());
      if (!bytes.toString('utf8').includes('<svg')) throw Error('Invalid sync resource');
      output(iconPath, bytes);
      output(iconPath + '.http.json', JSON.stringify({source_url: url, final_url: response.url, http_status: response.status, sha256: hash(bytes), bytes: bytes.length}, null, 2) + '\n');
    }
    const bytes = fileBytes(iconPath);
    if (commonIcon && !commonIcon.equals(bytes)) throw Error('Product sync resource differs');
    commonIcon = bytes;
    evidence.push({product_id: spec.product_id, generation: product.config.DeviceInfo.AnalogGenVersion,
      mount: {path: mounted.path, sha256: mounted.sha256, component: mounted.component},
      manifest: {path: manifestPath, sha256: hash(read(manifestPath))}, receipts, css,
      icon: {path: iconPath, sha256: hash(bytes), source_url: url}});
    console.log('Audited current actuation sync controls: ' + spec.product_id);
  }
  output('assets/synapse/keyboard-actuation-sync.svg', commonIcon);
  output('assets/synapse/keyboard-actuation-embedded.rs', '&[\n    ("synapse/keyboard-actuation-sync.svg", include_bytes!("keyboard-actuation-sync.svg") as &[u8]),\n]\n');
  output('src/features/keyboard_actuation_presentation_data.json', JSON.stringify(presentation, null, 2) + '\n');
  output('docs/re/keyboard-actuation-sync-current-evidence.json', JSON.stringify({schema_version: 1,
    method: 'Static Acorn and CSS parsing; UTF-16 offsets; vendor code never evaluated',
    scope: 'Sync all / selected controls, identical make-release payload, warning generation gate and source CSS; full editor remains partial', products: evidence}, null, 2) + '\n');
  const implementation = read('src/features/keyboard_actuation.rs');
  for (const needle of ['actuation-sync-all', 'actuation-sync-selected', 'SYNC_SETTINGS_TO_SELECTED_KEYS', '"AnalogGenVersion"', 'KeyboardActuationColors::warning()', 'if synchronize { raw }', '.left(surface::css(7.))', '.top(surface::css(-5.))', '.w(surface::css(210.))', '.h(surface::css(33.))']) {
    if (!implementation.includes(needle)) throw Error('Missing implementation contract ' + needle);
  }
  for (const needle of ['state.make_point', 'state.selected.iter()', 'state.sync_all_disabled', 'Duration::from_millis(2500)', 'state.lifecycle == lifecycle', 'state.presentation.factory_profile_lock', 'state.presentation.sync_padding_x', 'state.presentation.sync_container_height']) {
    if (!implementation.includes(needle)) throw Error('Missing retained-owner contract ' + needle);
  }
  if (implementation.includes('info.offset == 0. && value < 10.')) throw Error('Inferred warning gate returned');
  console.log('Validated ' + evidence.length + ' current ACTUATION command/CSS contracts; runtime and DLL write-back not run.');
}
main().catch(error => {console.error(error); process.exitCode = 1;});
