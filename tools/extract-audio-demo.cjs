// Resolve current mounted demo components without evaluating downloaded code.
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const {inspect} = require('./source-help-ast.cjs');
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
  const css = [...declarations].filter(p=>p.endsWith('.css')).map(file => {
    const source = base+'/'+file, text=fs.readFileSync(path.join(root,source),'utf8');
    return {path:source,sha256:hash(text),rules:text.split('}').filter(r=>/\.demo-body|\.demo-preview|floating-video-checkbox-wrapper/.test(r)).map(r=>r+'}')};
  }).filter(p=>p.rules.length);
  if (!css.some(p=>p.rules.some(r=>r.includes('height:450px')&&r.includes('width:800px')))) throw Error('Changed demo dimensions');
  if (!page.components.some(c=>c.source.includes('"/synapse/assets/videos/audio_mode.mov"'))) throw Error('Changed demo media');
  return {product_id:pid,poster,asset:'synapse/audio-demo-poster.png',width:800,height:450,
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
});
console.log('Resolved 3 current audio demo pages.');
