// Parse current product 784 as data; no downloaded code is evaluated.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const root = path.resolve(__dirname, '..'), read = p => fs.readFileSync(path.join(root, p), 'utf8');
const hash = b => crypto.createHash('sha256').update(b).digest('hex');
const receipt = JSON.parse(read('docs/re/pending-product-784-source.json'));
for (const f of receipt.source_files) if (hash(read(f.path)) !== f.sha256) throw Error('Changed source: ' + f.path);
const sourcePath = receipt.source_files[0].path, source = read(sourcePath), ast = acorn.parse(source, {ecmaVersion:'latest'});
const cssPath = '.ref/devices/784/static/css/main.03a1d509.css', css = read(cssPath);
function walk(n, fn) { if (!n?.type || fn(n) === false) return; for (const v of Object.values(n)) { if (Array.isArray(v)) v.forEach(x => walk(x, fn)); else if (v?.type) walk(v, fn); } }
function literal(n) {
  if (n.type === 'Literal') return n.value;
  if (n.type === 'UnaryExpression' && n.operator === '!') return !literal(n.argument);
  if (n.type === 'ArrayExpression') return n.elements.map(literal);
  if (n.type === 'ObjectExpression') return Object.fromEntries(n.properties.map(p => [p.key.name ?? p.key.value, literal(p.value)]));
  throw Error('Non-literal data: ' + n.type);
}
const defs = new Map(), strings = new Map(), dictionaries = [];
let localeModule;
walk(ast, n => {
  if (n.type === 'VariableDeclarator') {
    if (n.start > 4000000) defs.set(n.id.name, n);
  }
  if (n.type === 'Property' && n.key.value === 5955) localeModule = n.value;
});
walk(localeModule, n => {
  if (n.type === 'VariableDeclarator' && n.init?.type === 'Literal') strings.set(n.id.name,n.init.value);
  if (n.type === 'ObjectExpression' && n.properties.some(p => p.key?.name === 'DEVICE_LAYOUT')) dictionaries.push(n);
});
const keys = ['DEVICE_LAYOUT','DEVICE_LAYOUT_TIP','BEND_LED_STRIP','LEDSCONFIGURED','LEDSDETECTED','IDENTIFY_LED','REFRESH_STRIP','SMART_HOME_APP_CONTROLLING','DEVICE_OFFLINE'];
// Resolve the actual translation keys used by the mounted component.
let labelModule;
walk(ast, n => { if (n.type === 'Property' && n.key.value === 4693) labelModule = n.value; });
const labelDefs = new Map(), labelRefs = new Map();
walk(labelModule, n => {
  if (n.type === 'VariableDeclarator') labelDefs.set(n.id.name, n.init);
  if (n.type === 'CallExpression' && n.callee.property?.name === 'd' && n.arguments[1]?.type === 'ObjectExpression')
    for (const p of n.arguments[1].properties) labelRefs.set(p.key.name ?? p.key.value, p.value.body);
});
const labels = Object.fromEntries(['gCl','gRD','Ryt','xRj','tMK','Phx','LlS','avl','jA_','lGA','j$8','KvW','WKx','t20','wzC','bHZ','AVe','gxE','efo','xmU','tCr','S2T','Cir','_$r','lgc'].map(k => [k,literal(labelDefs.get(labelRefs.get(k)?.name))]));
for (const k of Object.values(labels)) if (!keys.includes(k)) keys.push(k);
const translations = {}, translationReceipts = [];
for (const d of dictionaries) {
  const values = Object.fromEntries(d.properties.filter(p => keys.includes(p.key.name ?? p.key.value)).map(p => [p.key.name ?? p.key.value, strings.get(p.value.body.name)]));
  const matches = fs.readdirSync(path.join(root,'locales')).filter(f => f.endsWith('.json') && JSON.parse(read('locales/'+f)).DEVICE_LAYOUT === values.DEVICE_LAYOUT);
  if (matches.length !== 1) throw Error('Ambiguous locale identity: '+values.DEVICE_LAYOUT);
  const locale = matches[0].slice(0,-5);
  for (const k of Object.values(labels)) if (typeof values[k] !== 'string') throw Error('Missing '+locale+'/'+k);
  translations[locale] = values;
  translationReceipts.push({locale,offset:d.start,end:d.end,sha256:hash(source.slice(d.start,d.end))});
}
if (Object.keys(translations).length !== 10) throw Error('Incomplete current translations');
const esc = s => String(s).replaceAll('&','&amp;').replaceAll('"','&quot;').replaceAll('<','&lt;').replaceAll('>','&gt;');
function props(n) {
  if (n.type === 'ObjectExpression') return Object.fromEntries(n.properties.map(p => [p.key.name ?? p.key.value,p.value]));
  if (n.type === 'Identifier' && n.name === 'e') return {}; // dynamic optional SVG props, unused by mounted source
  if (n.type === 'CallExpression') return Object.assign({}, ...n.arguments.map(props));
  throw Error('Unsupported SVG props: '+n.type);
}
function svg(n) {
  if (n.type === 'Literal') return esc(n.value);
  if (n.type === 'ArrayExpression') return n.elements.map(svg).join('');
  if (n.type !== 'CallExpression' || n.arguments[0]?.type !== 'Literal') throw Error('Unsupported SVG node: '+n.type);
  const tag = n.arguments[0].value, p = props(n.arguments[1]);
  const attributes = Object.entries(p).filter(([k]) => k !== 'children').map(([k,v]) => `${k==='className'?'class':k==='viewBox'?k:k.replace(/[A-Z]/g,c=>'-'+c.toLowerCase())}="${esc(literal(v))}"`).join(' ');
  return `<${tag}${attributes?' '+attributes:''}>${p.children?svg(p.children):''}</${tag}>`;
}
const assets = [['$G','shape-1'],['eg','shape-2'],['Eg','shape-3'],['ag','shape-4'],['tg','side-1'],['ig','side-2'],['og','side-3'],['ng','side-4'],['_g','identify'],['sg','refresh']].map(([symbol,name])=>{
  const n = defs.get(symbol); if (!n || n.start < 4580000 || n.start > 4595000) throw Error('Changed SVG binding '+symbol);
  let markup = svg(n.init.body);
  // The two icon path fills are supplied by current CSS, not the SVG literal.
  if (['identify','refresh'].includes(name)) markup = markup.replace(/<path /g,'<path fill="#ccc" ');
  return {name,output:`assets/synapse/aether-${name}.svg`,source:sourcePath,offset:n.start,end:n.end,source_sha256:hash(source.slice(n.start,n.end)),svg:markup};
});
const initialNode = defs.get('qs');
if (!initialNode || initialNode.start < 4030000 || initialNode.start > 4040000) throw Error('Reducer moved');
const initial = literal(initialNode.init);
const contracts = [
  'if(e<0||e>3)return a', 'Math.ceil(e/E)', 'colStart:_,colEnd:t',
  'pollTime:3,sleepInterval:500', 'maxLength:2,maxValue:o,minValue:1,stepValue:1',
  'O=_||!i,A=_||!n,S=A||O||!o', 'const i=_-E.reduce',
  'case"MW_AETHER_REFRESH_START"', 'case"MW_AETHER_REFRESH_STOP"',
  'ON_SET_CHROMA_LED_NUMBER','ON_GET_HARDWARE_LED_INFO','ON_AETHER_IDENTIFY_LEDS'
  ,'ON_SET_POWER_STATE_IOT','lg=[We._$r,We.Cir],Rg=[We.Cir]','backdrop:!1,isClosable:!1'
];
for (const c of contracts) if (!source.includes(c)) throw Error('Changed contract: '+c);
const geometry = css.split('}').filter(r=> /deviceLayoutWrapper|\.iot-offline|carousel--|s3-modal--|md-devices|indicator--|nav-tabs|profile-bar|rename-rect|gamer-room-icon|\.loader|\.widget \.help|body-widget-tip-portal/.test(r.split('{')[0])).map(r=>r+'}');
const manifestPath='.ref/devices/784/asset-manifest.json', manifest=JSON.parse(read(manifestPath));
const mediaNames=['indentify-btn','power-on-btn','power-off-btn','close-btn','close-hovered-btn','offline-badge','busy-btn','busy-btn-white','prd-3x','spinner','tooltip_questionmark'];
const media=mediaNames.map(name=>{
  const declared=Object.values(manifest.files).map(p=>p.replace(/^\.\//,'')).filter(p=>new RegExp('/'+name+'\\.[a-f0-9]+\\.(svg|avif)$').test(p));
  if(declared.length!==1)throw Error('Ambiguous media '+name);
  const file=declared[0];
  return {name,source:'.ref/devices/784/'+file,output:'assets/synapse/aether-'+name+(file.endsWith('.avif')?'.png':'.svg'),url:'https://apps.razer.com/synapse/products/784/ui/'+file,context:manifestPath,context_sha256:hash(read(manifestPath))};
});
function emit(p,value) { const output=JSON.stringify(value,null,2)+'\n'; if(process.argv.includes('--check')) { if(read(p)!==output)throw Error('Stale '+p); } else fs.writeFileSync(path.join(root,p),output); }
const headerFragments=[];
walk(ast,n=>{
 if(n.type==='ClassDeclaration'&&['dg','pI'].includes(n.id?.name)&&n.start>3800000){
   for(const m of n.body.body)if(['render','componentDidMount','componentDidUpdate'].includes(m.key?.name))headerFragments.push({owner:n.id.name,method:m.key.name,offset:m.start,end:m.end,source:source.slice(m.start,m.end)});
 }
 if(n.type==='AssignmentExpression'&&n.start>4269000&&n.start<4281000&&['renderProfileBar','renderProfileBarIcon','enableSwitchProfile'].includes(n.left.property?.name))headerFragments.push({owner:'ZS',method:n.left.property.name,offset:n.start,end:n.end,source:source.slice(n.start,n.end)});
});
emit('crates/razer-pages/src/features/aether_strip_data.json',{product_id:784,page:'CUSTOMIZED',labels,translations,initial});
emit('docs/re/aether-strip-current-evidence.json',{method:'Acorn literal and JSX SVG extraction; downloaded JavaScript never evaluated',generator_sha256:hash(fs.readFileSync(__filename)),source_files:[...receipt.source_files,{path:cssPath,sha256:hash(css)}],translationReceipts,initial_source:{offset:initialNode.start,end:initialNode.end},contracts,geometry,assets,media,header:{mounted_on_all_pages:true,sync_disabled_pages:['HELP','CUSTOMIZED'],dropdown_disabled_pages:['CUSTOMIZED'],has_gamer_room:true,fragments:headerFragments},components:[...receipt.pages[0].components.filter(c=>c.offset>=4594111).map(({path,offset,end,source})=>({path,offset,end,source})),...['Sg','Gl','ol','El','sl'].map(symbol=>{const n=defs.get(symbol);return {path:sourcePath,symbol,offset:n.start,end:n.end,source:source.slice(n.start,n.end)}})]});
console.log('Extracted Aether Light Strip layout, four shapes, four side markers, action icons and 10 current locales.');
