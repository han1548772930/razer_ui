// Parse current webpack data. No vendor module is loaded or evaluated.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const read = name => fs.readFileSync(path.join(root,name),'utf8');
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const product = JSON.parse(read('crates/razer-pages/src/features/keyboard_products_data.json')).find(p=>p.product_id===691);
const file = product.source_files.find(f=>/\/main\./.test(f.path));
const source = read(file.path);
if(hash(source)!==file.sha256) throw Error('Current OLED source changed');
const ast = acorn.parse(source,{ecmaVersion:'latest'});
function walk(n, visit) {
  if(!n?.type || visit(n)===false) return;
  for(const value of Object.values(n)) {
    if(Array.isArray(value)) value.forEach(child=>walk(child,visit));
    else if(value?.type) walk(value,visit);
  }
}
const modules = new Map();
walk(ast,n=>{
  if(n.type==='Property' && [22502,37769,9483,79826].includes(n.key.value) && /Function/.test(n.value.type)) {
    modules.set(n.key.value,n.value); return false;
  }
});
function definitions(id) {
  const defs=new Map(), module=modules.get(id);
  if(!module) throw Error('Missing module '+id);
  walk(module.body,n=>{
    if(n.type==='VariableDeclarator' && n.id.type==='Identifier') defs.set(n.id.name,n.init);
    if(/Function|Class/.test(n.type)) return false;
  });
  return defs;
}
function literal(n,defs,stack=[]) {
  if(n.type==='Literal') return n.value;
  if(n.type==='UnaryExpression' && n.operator==='!') return !literal(n.argument,defs,stack);
  if(n.type==='Identifier' && defs.has(n.name) && !stack.includes(n.name)) return literal(defs.get(n.name),defs,[...stack,n.name]);
  if(n.type==='ArrayExpression') return n.elements.map(e=>literal(e,defs,stack));
  if(n.type==='ObjectExpression') return Object.fromEntries(n.properties.map(p=>{
    if(p.type!=='Property' || p.computed) throw Error('Nonliteral property');
    return [p.key.name ?? p.key.value,literal(p.value,defs,stack)];
  }));
  if(n.type==='BinaryExpression' && n.operator==='+' && n.left.type==='MemberExpression' && n.left.object.name==='n' && n.left.property.name==='p') return literal(n.right,defs,stack);
  throw Error('Unsupported static expression '+source.slice(n.start,n.end).slice(0,90));
}
const animationDefs=definitions(22502), animation=animationDefs.get('y');
if(animation.type!=='ConditionalExpression' || source.slice(animation.test.start,animation.test.end)!=='15===O') throw Error('Animation FPS branch changed');
const animations=literal(product.config.OLED_ANIMATION_FPS===15?animation.consequent:animation.alternate,animationDefs);
const images=literal(definitions(37769).get('o'),definitions(37769));
const manifest=JSON.parse(read('.ref/devices/691/asset-manifest.json')).files;
const declared=new Set(Object.values(manifest).map(value=>value.replace('/synapse/products/691/ui/','')));
for(const animation of animations) if(!declared.has(animation.src)) throw Error('Undeclared animation asset');
for(const image of images) {
  if(!image.src.startsWith('data:image/png;base64,') && !declared.has(image.src))
    throw Error('Undeclared image asset');
}
const banner=definitions(9483);
const fonts=literal(banner.get('o'),banner), sizes=literal(banner.get('i'),banner);
if(animations.length!==6 || images.length!==10) throw Error('Preset count changed');
for(const [kind,items] of [['animation',animations],['image',images]]) {
  items.forEach((item,i)=>{
    if(item.id!==`${kind}-${i+1}` || typeof item.src!=='string') throw Error('Preset order changed');
  });
}
const defaults=definitions(79826).get('O');
const fields=Object.fromEntries(defaults.properties.map(p=>[p.key.name,p.value]));
const home=literal(fields.homeScreenDisplay,new Map());
const media=literal(fields.media,new Map());
// Read reducer defaults independently of the preset assets. Calls converting
// URLs to data URLs are checked as syntax only and are never evaluated.
const presetDefaults={};
for(const [kind,items,alias] of [['animation',animations,'a'],['image',images,'r']]) {
  const node=fields[kind];
  const props=Object.fromEntries(node.properties.map(p=>[p.key.name,p.value]));
  const list=props.list.elements.map((entry,ix)=>{
    const props=Object.fromEntries(entry.properties.map(p=>[p.key.name,p.value]));
    if(source.slice(props.src.start,props.src.end)!==`D(${alias}.A[${ix}].src)`)
      throw Error('Preset source binding changed');
    const value=Object.fromEntries(['id','custom','enabled'].map(k=>[k,literal(props[k],new Map())]));
    if(value.id!==items[ix].id || value.custom || !value.enabled) throw Error('Preset default changed');
    return value;
  });
  if(list.length!==items.length) throw Error('Reducer preset count changed');
  presetDefaults[kind]={selectedIdx:literal(props.selectedIdx,new Map()),list};
}
const page=JSON.parse(read('docs/re/keyboard-product-pages.json')).products.find(p=>p.product_id===691).pages.find(p=>p.key==='OLED');
const pageSource=read(page.path);
if(hash(pageSource)!==page.sha256) throw Error('OLED editor source changed');
const mediaComponent=page.components.find(c=>c.symbol==='Os');
let pageModule;
walk(acorn.parse(pageSource,{ecmaVersion:'latest'}),n=>{
  if(n.type==='Property'&&Number.isInteger(n.key.value)&&/Function/.test(n.value.type)&&n.start<mediaComponent.offset&&n.end>mediaComponent.end)pageModule=n.value;
});
const pageDefs=new Map();
walk(pageModule.body,n=>{
  if(/Function|Class/.test(n.type))return false;
  if(n.type==='VariableDeclarator'&&n.id.type==='Identifier')pageDefs.set(n.id.name,n.init);
});
const visualizers=literal(pageDefs.get('Ss'),pageDefs).map((src,ix)=>({id:`visualizer-${ix+1}`,src}));
if(visualizers.length!==3||visualizers.some(x=>!declared.has(x.src)))throw Error('Media visualizer assets changed');
const editorEvidence=['wt','Vt','jt','Ht','pi','ve','Os','Ns'].map(symbol=>{
  const c=page.components.find(c=>c.symbol===symbol);
  if(!c || pageSource.slice(c.offset,c.end)!==c.source) throw Error('Editor receipt changed');
  if(['wt','Vt'].includes(symbol) && !['structuredClone(','.findIndex(e=>e.enabled)','switchDisabled:1==='].every(s=>c.source.includes(s)))
    throw Error('Preset state rules changed');
  return {symbol,path:page.path,offset:c.offset,end:c.end,sha256:hash(c.source)};
});
const output={product_id:691,source:file,animation_fps:product.config.OLED_ANIMATION_FPS ?? null,
  animations,images,visualizers,banner_fonts:fonts,banner_sizes:sizes,home_default:home,media_default:media,
  preset_defaults:presetDefaults,editor_evidence:editorEvidence,
  evidence:[...modules].map(([module,n])=>({module,offset:n.start,end:n.end,sha256:hash(source.slice(n.start,n.end))})),
  limitations:['Native preset selection, local import/crop/custom reset and crop-placement draft metadata are mounted; GIF processing and device transport remain unavailable.',
    'Media and system sample values in the source must not be represented as live device readings.']};
fs.writeFileSync(path.join(root,'crates/razer-pages/src/features/keyboard_oled_editor_data.json'),JSON.stringify(output)+'\n');
console.log(`Extracted ${animations.length} animation presets, ${images.length} image presets, ${fonts.length} fonts and ${sizes.length} sizes from current modules.`);
