// Resolve current mounted demo components without evaluating downloaded code.
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const {inspect} = require('./source-help-ast.cjs');
const {Source} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const hash = text => crypto.createHash('sha256').update(text).digest('hex');
const check = process.argv.includes('--check');
function emit(file, value) {
  const content = JSON.stringify(value, null, 2) + '\n';
  if (check) {
    if (fs.readFileSync(path.join(root,file),'utf8') !== content) throw Error('Stale audio demo data: '+file);
  } else fs.writeFileSync(path.join(root,file),content);
}
const catalog = JSON.parse(fs.readFileSync(path.join(root, 'docs/re/unimplemented-products.json'), 'utf8')).products;
const products = [1392, 1442, 3942].map(pid => {
  const product = catalog.find(p => p.product_id === pid);
  const nav = product.navigation.find(n => n.items.some(i => i.name?.value === 'TAB_DEMO'));
  if (!nav) throw Error('Missing demo navigation: ' + pid);
  return inspect({...product, config:{path:nav.source, sha256:nav.sha256}}, ['TAB_DEMO']);
});
function walk(node, visit) {
  if (!node?.type || visit(node) === false) return;
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) value.forEach(child => walk(child, visit));
    else if (value?.type) walk(value, visit);
  }
}
const descriptors = products.map(product => {
  const pid = product.product_id;
  const base = `.ref/devices/${pid}`;
  const manifestPath = `${base}/asset-manifest.json`;
  const manifestText = fs.readFileSync(path.join(root,manifestPath), 'utf8');
  const manifest = JSON.parse(manifestText);
  const declarations = new Set(Object.values(manifest.files).map(p => p.replace(/^\.\//,'')));
  const mainPath = base + '/' + manifest.files['main.js'].replace(/^\.\//,'');
  const main = fs.readFileSync(path.join(root, mainPath), 'utf8');
  const defaults = 'demoFloatingVideoEnabled:!0,floatingVideoTime:{currentTime:0,isPlaying:!1},forceOpen:!1';
  const defaultsOffset = main.indexOf(defaults);
  if (defaultsOffset < 0) throw Error('Changed audio demo defaults');
  let poster;
  for (const file of product.source_files) {
    const text = fs.readFileSync(path.join(root,file.path),'utf8');
    if (hash(text) !== file.sha256) throw Error('Changed audio demo source');
    walk(acorn.parse(text,{ecmaVersion:'latest'}), node => {
      if (node.type !== 'Property' || (node.key.name ?? node.key.value) !== 2222) return;
      walk(node.value, child => {
        if (child.type === 'AssignmentExpression' && child.left.property?.name === 'exports'
            && child.right.type === 'BinaryExpression' && child.right.operator === '+'
            && child.right.right.type === 'Literal') {
          if (poster) throw Error('Ambiguous poster');
          const request = child.right.right.value;
          if (!declarations.has(request)) throw Error('Undeclared demo poster');
          poster = {source:base+'/'+request, url:`https://apps.razer.com/synapse/products/${pid}/ui/${request}`,
            module_file:file.path, module_sha256:file.sha256, module_id:2222, offset:child.start, end:child.end};
        }
      });
      return false;
    });
  }
  if (!poster) throw Error('Missing source demo poster');
  const page = product.pages[0];
  const labels = new Set(page.components.flatMap(c=>c.jsx.map(j=>j.props.text)).filter(Boolean));
  for (const key of ['DEMO_TITLE_DESC','FLOATING_VIDEO_CHECKBOX_LABEL']) if (!labels.has(key)) throw Error('Changed demo label');
  const player = page.components.find(c => c.source.includes('className:"custom-control-bar"'));
  if (!player) throw Error('Missing current mounted demo control bar');
  const playerTree = acorn.parse('('+player.source+')',{ecmaVersion:'latest'});
  const bars = [];
  walk(playerTree, node => {
    if (node.type !== 'ObjectExpression') return;
    const props = Object.fromEntries(node.properties.filter(p=>p.type==='Property').map(p=>[p.key.name??p.key.value,p.value]));
    if (props.className?.value === 'custom-control-bar') bars.push(props);
  });
  if (bars.length !== 1 || bars[0].disableDefaultControls?.type !== 'UnaryExpression'
      || bars[0].disableDefaultControls.operator !== '!' || bars[0].disableDefaultControls.argument.value !== 0
      || bars[0].children?.type !== 'ArrayExpression') throw Error('Changed custom control contract');
  const controlClasses = bars[0].children.elements.map(node => {
    const props = node.arguments?.[1];
    if (props?.type !== 'ObjectExpression') throw Error('Changed control props');
    return props.properties.find(p=>(p.key.name??p.key.value)==='className')?.value.value;
  });
  if (JSON.stringify(controlClasses) !== JSON.stringify(['custom-play-toggle','custom-progress-bar','custom-fullscreen-toggle']))
    throw Error('Changed mounted demo controls');
  let playerAlias;
  walk(playerTree, node => {
    if (node.type === 'MemberExpression' && node.property.name === 'ai') playerAlias = node.object.name;
  });
  if (!playerAlias) throw Error('Unresolved mounted Player import');
  const scope = Object.create(Source.prototype);
  scope.directory = base;
  scope.files = [...declarations].filter(f=>f.endsWith('.js')).map(f=>`${base}/${f}`);
  scope.modules = new Map(); scope.texts = new Map(); scope.parsed = new Set();
  const mountedText = fs.readFileSync(path.join(root,player.path),'utf8');
  const playerImports = [];
  walk(acorn.parse(mountedText,{ecmaVersion:'latest'}), node => {
    if (node.type === 'VariableDeclarator' && node.id.name === playerAlias
        && node.init?.type === 'CallExpression' && Number.isInteger(node.init.arguments[0]?.value))
      playerImports.push(node);
  });
  if (playerImports.length !== 1) throw Error('Ambiguous mounted Player module import');
  const libraryId = playerImports[0].init.arguments[0].value;
  const library = scope.module(libraryId), libraryExports = {};
  walk(library.fn, node => {
    if (node.type !== 'CallExpression' || node.callee.object?.name !== 'Object'
        || node.callee.property?.name !== 'defineProperty' || node.arguments[0]?.name !== library.fn.params[1].name
        || !['ai','zA','Wc'].includes(node.arguments[1]?.value)) return;
    const getter = node.arguments[2]?.properties?.find(p=>(p.key.name??p.key.value)==='get')?.value;
    const returned = getter?.body?.body?.find(n=>n.type==='ReturnStatement')?.argument;
    if (returned?.type !== 'MemberExpression' || returned.property.name !== 'default') throw Error('Changed Player export');
    const binding = scope.binding(libraryId,returned.object.name);
    const request = binding.arguments?.[0];
    if (request?.type !== 'CallExpression' || !Number.isInteger(request.arguments[0]?.value)) throw Error('Changed Player implementation import');
    libraryExports[node.arguments[1].value] = request.arguments[0].value;
  });
  if (Object.keys(libraryExports).length !== 3) throw Error('Missing actual Player/control implementations');
  const implementationReceipts = Object.entries(libraryExports).map(([export_name,id]) => {
    const module = scope.module(id), receipt = scope.receipt(id,module.fn);
    let buttonClass = false;
    walk(module.fn, node => {
      if (node.type === 'Literal' && typeof node.value === 'string' && node.value.split(/\s+/).includes('video-react-button')) buttonClass = true;
    });
    if (export_name !== 'ai' && !buttonClass) throw Error('Changed actual button classes');
    if (export_name === 'ai' && receipt.source.includes('fontSize')) throw Error('Review changed Player inline font size');
    return {export_name,module:id,...receipt};
  });
  const contextRequests = [], contextImports = [], directOptionalLoads = [], optionalCssKeys = [];
  for (const file of scope.files) {
    const text = scope.text(file);
    if (!/58388|1491|video-player\.css/.test(text)) continue;
    walk(acorn.parse(text,{ecmaVersion:'latest'}), node => {
      if (node.type === 'CallExpression' && node.arguments[0]?.value === 58388)
        contextImports.push({path:file,offset:node.start,end:node.end});
      if (node.type === 'CallExpression' && node.callee.type === 'CallExpression'
          && node.callee.arguments[0]?.value === 58388) {
        const request = text.slice(node.start,node.end);
        if (!request.includes('/img_prods/prd-')) throw Error('New resource-context request requires cascade review');
        contextRequests.push({path:file,sha256:hash(text),offset:node.start,end:node.end,source:request});
      }
      if (node.type === 'CallExpression' && node.callee.type === 'MemberExpression'
          && node.callee.property.name === 'e' && node.arguments[0]?.value === 1491)
        directOptionalLoads.push({path:file,offset:node.start});
      if (node.type === 'Literal' && node.value === './scss/video-player.css')
        optionalCssKeys.push({path:file,sha256:hash(text),offset:node.start,end:node.end,source:text.slice(node.start,node.end)});
    });
  }
  if (directOptionalLoads.length || optionalCssKeys.length > 1
      || contextImports.length !== contextRequests.length
      || (optionalCssKeys.length && contextRequests.length !== 6)) throw Error('Review changed optional video CSS loading');
  const css = [...declarations].filter(p=>p.endsWith('.css')).map(file => {
    const source = base+'/'+file, text=fs.readFileSync(path.join(root,source),'utf8');
    return {path:source,sha256:hash(text),rules:text.split('}').filter(r=>/\.demo-body|\.demo-preview|floating-video-checkbox-wrapper|^\.custom-(control-bar|play-toggle|progress-bar|fullscreen-toggle)|^\.video-react \.video-react-control\{|^\.check-(item|box|text)|^\.body-wrapper\{/.test(r)).map(r=>r+'}')};
  }).filter(p=>p.rules.length);
  if (!css.some(p=>p.rules.some(r=>r.includes('height:450px')&&r.includes('width:800px')))) throw Error('Changed demo dimensions');
  if (!page.components.some(c=>c.source.includes('"/synapse/assets/videos/audio_mode.mov"'))) throw Error('Changed demo media');
  const posterRules = css.flatMap(p => p.rules).filter(r=>r.startsWith('.demo-body .demo-preview .btn-image img{'));
  if (posterRules.length !== 1) throw Error('Ambiguous current poster rule');
  const object_fit = posterRules[0].includes('object-fit:cover') ? 'cover' : 'fill';
  if (posterRules[0].includes('object-fit:') && object_fit !== 'cover') throw Error('Unsupported source object-fit');
  const controlRule = css.flatMap(p=>p.rules).find(r=>r.startsWith('.custom-control-bar{'));
  for (const declaration of ['background:#0003!important','border-radius:5px','height:40px','padding:0'])
    if (!controlRule?.includes(declaration)) throw Error('Changed demo bar style: '+declaration);
  const checkboxRule = css.flatMap(p=>p.rules).find(r=>r.startsWith('.check-box{'));
  if (!checkboxRule?.includes('background-color:#0000')) throw Error('Changed unchecked demo checkbox background');
  const tickRule = css.flatMap(p=>p.rules).find(r=>r.startsWith('.check-box:after{'));
  if (!tickRule?.includes('left:.8px') || !tickRule.includes('top:10.2px')) throw Error('Changed demo lower tick origin');
  const mainCssPath = `${base}/${manifest.files['main.css'].replace(/^\.\//,'')}`;
  const mainCssText = fs.readFileSync(path.join(root,mainCssPath),'utf8');
  const cascadeSelectors = ['.video-react','.video-react .video-react-button','.video-react .video-react-control','.video-react .video-react-control:before'];
  const cascadeRules = parseCSS(mainCssText).filter(r=>cascadeSelectors.includes(r.selector));
  const declaration = (selector,property) => cascadeRules.find(r=>r.selector===selector)?.properties.find(d=>d.property===property)?.value;
  if (declaration('.video-react','font-size') !== '10px'
      || declaration('.video-react .video-react-button','font-size') !== 'inherit'
      || declaration('.video-react .video-react-control','width') !== '4em'
      || declaration('.video-react .video-react-control:before','font-size') !== '1.8em') throw Error('Review changed control font cascade');
  const indexPath = `${base}/index.html`, index = fs.readFileSync(path.join(root,indexPath),'utf8');
  const initialStyles = [...index.matchAll(/<link\b[^>]*href="([^"]+)"[^>]*rel="stylesheet"[^>]*>/g)].map(m=>({offset:m.index,end:m.index+m[0].length,source:m[0],href:m[1]}));
  if (initialStyles.length !== 1 || initialStyles[0].href.replace(/^\.\//,'') !== manifest.files['main.css'].replace(/^\.\//,''))
    throw Error('Review changed entry stylesheet order');
  const loaderStart = main.indexOf('miniCssF='), loaderEnd = main.indexOf('.f.miniCss=',loaderStart);
  if (loaderStart < 0 || loaderEnd < 0 || !main.slice(loaderStart,loaderEnd).includes('document.head.appendChild(')) throw Error('Review changed async CSS insertion');
  return {product_id:pid,poster,asset:'synapse/audio-demo-poster.png',width:800,height:450,
    object_fit,
    controls: controlClasses,
    presentation:{player_font_size:10,button_font_size:10,button_width:40,glyph_size:18,checkbox_lower_tick_origin:[0.8,10.2]},
    cascade_evidence: {
      method:'Mounted controls carry video-react-button. Its font-size:inherit specificity 0,2,0 overrides custom font-size:18px specificity 0,1,0. Player has no inline fontSize; video-react root is 10px, width is 4em, glyph is 1.8em.',
      player_import:{path:player.path,sha256:hash(mountedText),offset:playerImports[0].start,end:playerImports[0].end,source:mountedText.slice(playerImports[0].start,playerImports[0].end)},
      library:{module:libraryId,...scope.receipt(libraryId,library.fn)}, implementations:implementationReceipts,
      css:{path:mainCssPath,sha256:hash(mainCssText),rules:cascadeRules},
      stylesheet_loading:{index:{path:indexPath,sha256:hash(index),styles:initialStyles},runtime:{path:mainPath,sha256:hash(main),offset:loaderStart,end:loaderEnd+300,source:main.slice(loaderStart,loaderEnd+300)}},
      optional_1491_css:{context_imports:contextImports,context_requests:contextRequests,resource_keys:optionalCssKeys,direct_chunk_loads:directOptionalLoads,status:optionalCssKeys.length?'not_requested_by_current_product_calls':'not_in_current_product'},
    },
    title:'DEMO_TITLE_DESC',floating_label:'FLOATING_VIDEO_CHECKBOX_LABEL',floating_default:true,
    video_url:'https://apps.razer.com/synapse/assets/videos/audio_mode.mov',
    manifest:{path:manifestPath,sha256:hash(manifestText)},
    defaults:{path:mainPath,sha256:hash(main),offset:defaultsOffset,source:defaults},css,
    playback_status:'not_implemented'};
});
emit('src/features/audio_demo_data.json',descriptors);
emit('docs/re/audio-demo-current-evidence.json', {
  method:'Acorn lexical resolution of mounted source components; no vendor execution',
  generator_sha256:hash(fs.readFileSync(__filename)),
  resolver_sha256:hash(fs.readFileSync(path.join(__dirname,'source-help-ast.cjs'))), products,
  presentation_audits:descriptors.map(d=>({product_id:d.product_id,presentation:d.presentation,cascade_evidence:d.cascade_evidence})),
});
const nativeDemo = fs.readFileSync(path.join(root,'src/features/audio_demo.rs'),'utf8');
if (!nativeDemo.includes('surface::check_item_with_style(') || !nativeDemo.includes('Colors::unchecked_background()')
    || !nativeDemo.includes('tick_bottom_origin: (0.8, 10.2)')) throw Error('Demo checkbox ignores its source style');
if (!nativeDemo.includes('surface::css(self.spec.presentation.button_width)') || !nativeDemo.includes('surface::css(self.spec.presentation.glyph_size)'))
  throw Error('Demo media controls ignore source cascade dimensions');
console.log('Resolved 3 current audio demo pages.');
