// Current 1383 Banner editor: literal AST/CSS extraction, never vendor execution.
const fs = require('fs'), path = require('path');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), directory = '.ref/devices/1383';
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const manifestPath = `${directory}/asset-manifest.json`, manifest = JSON.parse(read(manifestPath));
const source = Object.create(Source.prototype);
Object.assign(source, {directory, files: [...new Set(Object.values(manifest.files))]
  .filter(file => file.includes('/static/js/') && file.endsWith('.js'))
  .map(file => `${directory}/${file.slice(file.indexOf('static/'))}`),
  modules: new Map(), texts: new Map(), parsed: new Set()});
const receipts = [], owner = 51278;
const binding = (id, symbol) => { const node = source.binding(id, symbol);
  receipts.push({module: id, symbol, ...source.receipt(id, node)}); return node; };
const property = (node, name) => {
  const props = node.properties.filter(p => key(p.key) === name);
  if (props.length !== 1) throw Error(`Expected literal property ${name}`);
  return props[0].value;
};
function literal(id, node) {
  if (node.type === 'MemberExpression' && node.computed) {
    const values = literal(id, node.object), index = literal(id, node.property);
    if (!Array.isArray(values) || !Number.isInteger(index) || index < 0 || index >= values.length) throw Error('Nonliteral array selection');
    return values[index];
  }
  if (node.type === 'ObjectExpression') return Object.fromEntries(node.properties.map(p => {
    if (p.type !== 'Property' || p.computed) throw Error('Nonliteral object');
    return [key(p.key), literal(id, p.value)];
  }));
  return source.literal(id, node);
}
const defaultsNode = property(binding(79826, 'T'), 'banner');
const defaults = literal(79826, defaultsNode);
const fonts = literal(9483, binding(9483, 'r')), sizes = literal(9483, binding(9483, 'a'));
const imageNode = binding(9483, 'o'), images = literal(9483, imageNode);
const editor = binding(owner, 'Ug'), preview = binding(owner, 'Hg');
binding(owner, 'Bg'); binding(owner, 'Mv'); binding(owner, 'Hp'); binding(owner, 'oh');
const editorText = source.snippet(owner, editor), previewText = source.snippet(owner, preview);
for (const fragment of ['g.current.offsetWidth>920', 'g.current.offsetHeight>(o.enabled?176:256)',
  'value:r.value.slice(0,-1)', 'setTimeout(()=>{u(!0)},100)', 'placeholder:"Enter text here"',
  'fontWeight:r.style.fontWeight?null:"bold"', 'fontStyle:r.style.fontStyle?null:"italic"',
  'textDecoration:r.style.textDecoration?null:"underline"', 'backgroundColor:"#000",scale:1']) {
  if (!editorText.includes(fragment)) throw Error(`Banner editor changed: ${fragment}`);
}
for (const fragment of ['d=t.enabled?44:64', 'const e=3400,t=28*n.current.scrollWidth',
  's.value.replace(/\\n/g," ")', 'Math.ceil(h().scrollHeight/d)*d+"px"']) {
  if (!previewText.includes(fragment)) throw Error(`Banner preview changed: ${fragment}`);
}
const reducer = source.binding(79826, 'I');
walk(reducer, node => {
  if (node.type === 'SwitchCase' && /SET_OLED_DISPLAY_BANNER/.test(source.snippet(79826, node.test || node))) {
    receipts.push({module: 79826, symbol: 'I:banner-case', ...source.receipt(79826, node)});
  }
});
const labels = {};
for (const symbol of ['ADx', '$yX', 'Ufo', 'wLi', 'MV7', 'wv5', '_NK', 'gc6', 'Xpn', 'erU', 'gv6', 'E_v', 'WAD', 'sn9', 'BZ0', 'pJk', 'bOp']) {
  const node = source.exported(54693, symbol);
  labels[symbol] = source.literal(54693, node);
  receipts.push({module: 54693, symbol, ...source.receipt(54693, node)});
}
const escape = value => String(value).replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;');
const attrs = {className:'class',strokeWidth:'stroke-width',strokeLinecap:'stroke-linecap',strokeLinejoin:'stroke-linejoin',clipPath:'clip-path',fillRule:'fill-rule',clipRule:'clip-rule',xmlnsXlink:'xmlns:xlink',xmlSpace:'xml:space'};
function svg(node) {
  if (!node || node.type === 'ConditionalExpression') return '';
  if (node.type === 'LogicalExpression' || node.type === 'AssignmentExpression') return svg(node.right);
  if (node.type === 'Literal') return node.value == null ? '' : escape(node.value);
  if (node.type !== 'CallExpression' || node.callee.property?.name !== 'createElement') throw Error('Nonliteral SVG child');
  const [tag, input, ...children] = node.arguments, props = input.type === 'CallExpression' ? input.arguments[0] : input;
  if (tag.type !== 'Literal') throw Error('Nonliteral SVG tag');
  let attributes = '';
  if (props.type === 'ObjectExpression') for (const p of props.properties) {
    const name = key(p.key); if (['ref','aria-labelledby','nonce'].includes(name)) continue;
    if(name==='style' && p.value.type==='ObjectExpression') {
      const style=p.value.properties.map(prop=>{
        const k=key(prop.key);if(prop.type!=='Property'||prop.value.type!=='Literal')throw Error('Nonliteral SVG style');
        const v=typeof prop.value.value==='number'&&['width','height'].includes(k)?`${prop.value.value}px`:prop.value.value;
        return `${k.replace(/[A-Z]/g,c=>'-'+c.toLowerCase())}:${v}`;
      }).join(';');
      attributes+=` style="${escape(style)}"`;continue;
    }
    if (p.value.type !== 'Literal') throw Error(`Nonliteral SVG attribute ${name}: ${source.snippet(owner,p.value)}`);
    attributes += ` ${attrs[name] || name}="${escape(p.value.value)}"`;
  } else if (!(props.type === 'Literal' && props.value == null)) throw Error('Nonliteral SVG props');
  return `<${tag.value}${attributes}>${children.map(svg).join('')}</${tag.value}>`;
}
const assets = images.map(image => ({id:image.id,src:image.src,output:`synapse/audio-oled-banner-${image.id}.png`,receipt:source.receipt(9483,imageNode)}));
const icons = [];
for (const [symbol,id] of [['$u','restart'],['Xu','pause'],['tg','increase'],['ng','decrease'],['cg','bold'],['ug','italic'],['fg','underline'],['wg','left'],['Mg','right'],['Og','up'],['Vg','down']]) {
  let node = binding(owner, symbol);
  if(node.type==='CallExpression' && node.arguments[0]?.type==='Identifier'
      && node.callee.type==='SequenceExpression' && key(node.callee.expressions.at(-1).property)==='forwardRef') {
    node=binding(owner,node.arguments[0].name);
  }
  const roots = [];
  walk(node, n => {if(n.type === 'CallExpression' && n.callee.property?.name === 'createElement' && n.arguments[0]?.value === 'svg') roots.push(n);});
  if (roots.length !== 1) throw Error(`Ambiguous icon ${id}`);
  icons.push({id,output:`synapse/audio-oled-banner-${id}.svg`,svg:svg(roots[0])+'\n',receipt:source.receipt(owner,node)});
}
const css = [], keyframes = [], fontFaces = [], boxSizingRules = [], textareaRules = [];
for (const path of [...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css')).map(f=>`${directory}/${f.slice(f.indexOf('static/'))}`)) {
  const text = read(path), sha256 = hash(text);
  const rules=parseCSS(text);
  css.push(...rules.filter(r=>/CustomizeBanner_|CustomizeModal_|DisplayWidget_|^body,html$|^div$|^\.s3-|^\.radio-item|type=radio/.test(r.selector)).map(r=>({path,sha256,...r})));
  boxSizingRules.push(...rules.filter(r=>r.declarations.includes('box-sizing:')).map(r=>({path,sha256,...r})));
  textareaRules.push(...rules.filter(r=>r.selector.includes('textarea')).map(r=>({path,sha256,...r})));
  for (const match of text.matchAll(/@keyframes CustomizeBanner_[^{]+\{/g)) {
    let end = match.index + match[0].length, depth=1;
    while(depth && end<text.length){if(text[end]==='{')depth++;else if(text[end]==='}')depth--;end++;}
    if(depth)throw Error('Unclosed keyframes');
    keyframes.push({path,sha256,offset:match.index,end,source:text.slice(match.index,end)});
  }
  for(const match of text.matchAll(/@font-face\{[^}]+\}/g))if(/font-family:(RazerF5|Roboto);/.test(match[0]))fontFaces.push({path,sha256,offset:match.index,end:match.index+match[0].length,source:match[0]});
}
if(images.length!==8 || sizes.length!==14 || keyframes.length!==4)throw Error('Changed Banner choices/motion');
// Ug's textarea has no class, type, id, or tooltip attribute. Of every author
// box-sizing rule, only these selectors could reach a textarea. Neither scoped
// ancestor is present in Ug -> CustomizeModal -> choose-a-mat.
const boxCandidates=boxSizingRules.filter(r=>r.selector.includes('textarea')||r.selector.includes('*'));
const excludedScopes=['.key-config .body textarea','.video-react *,.video-react :after,.video-react :before'];
if(boxCandidates.some(r=>!excludedScopes.includes(r.selector)))throw Error('Reaudit author textarea/universal box sizing');
if(!boxSizingRules.some(r=>r.selector==='div'&&r.declarations==='box-sizing:border-box'))throw Error('Changed div box sizing');
if(textareaRules.some(r=>r.selector==='textarea'||r.selector.split(',').some(s=>s.trim()==='textarea')))throw Error('New global author textarea rules');
const ua=JSON.parse(read('docs/re/current-browser-ua-evidence.json'));
if(hash(read(ua.css.path))!==ua.css.sha256||ua.textarea.padding_css_px!==2)throw Error('Changed current browser UA receipt');
const textarea={content_width:385,content_height:98,padding:ua.textarea.padding_css_px,border:1,
  outer_width:385+2*ua.textarea.padding_css_px+2,outer_height:98+2*ua.textarea.padding_css_px+2};
const data = {product_id:1383,labels,fonts,sizes,defaults,images:assets.map(({id,src,output})=>({id,src,asset:output})),icons:Object.fromEntries(icons.map(({id,output})=>[id,output])),
  limits:{horizontal_width:920,vertical_with_image:176,vertical_without_image:256},
  motion:{horizontal_base_ms:3400,horizontal_width_ms:28,vertical_ms:5000,restart_ms:100},textarea,placeholder:'Enter text here'};
const evidence = {method:'Current manifest AST/CSS, no vendor code execution; offsets are UTF-16 units.',manifest:{path:manifestPath,sha256:hash(read(manifestPath))},receipts,assets,icons,css,keyframes,fontFaces,
  textareaCascade:{boxSizingRules,textareaRules,excludedBoxSizingCandidates:boxCandidates,boxSizing:'content-box',
    rationale:'No class/id/type on Ug textarea; .key-config and .video-react are absent from its mounted ancestor chain. div border-box does not match textarea and box-sizing does not inherit.',
    ua_receipt:'docs/re/current-browser-ua-evidence.json',ua_sha256:hash(read('docs/re/current-browser-ua-evidence.json')),computed:textarea}};
const flags=process.argv.slice(2);if(flags.some(f=>f!=='--check'))throw Error('Unknown argument');
for(const [file,object]of [['crates/razer-pages/src/features/audio_oled_banner_data.json',data],['docs/re/audio-oled-banner-current-evidence.json',evidence]]){
  const output=JSON.stringify(object,null,2)+'\n';if(flags.includes('--check')){if(read(file)!==output)throw Error(`Stale ${file}`);}else fs.writeFileSync(path.join(root,file),output);
}
console.log(`1383 Banner: ${fonts.length} fonts, ${sizes.length} sizes, 8 images, 11 icons, four scroll directions audited.`);
