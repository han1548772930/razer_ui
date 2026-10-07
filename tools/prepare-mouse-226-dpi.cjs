// Current 226 stage rows and piecewise slider. No reference code execution.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {Source, walk, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), directory = '.ref/devices/226';
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const manifestPath = directory + '/asset-manifest.json', manifest = JSON.parse(read(manifestPath));
const source = Object.assign(Object.create(Source.prototype), {directory,
  files: [...new Set(Object.values(manifest.files))].filter(f => f.endsWith('.js')).map(f => directory + '/' + f.slice(2)),
  modules: new Map(), texts: new Map(), parsed: new Set()});
const page = JSON.parse(read('docs/re/mouse-page-source.json')).products.find(p => p.product_id === 226).pages.find(p => p.key === 'TAB_PERFORMANCE');
const text = read(page.path), ast = acorn.parse(text, {ecmaVersion:'latest'});
if (hash(text) !== page.sha256) throw Error('Re-audit current Performance source');
const receipts = ['Ft','Wt','es','ts','ls','cs','Ms','ys','wi','Ri'].map(symbol => {
  const c = page.components.find(c => c.symbol === symbol);
  if (!c || text.slice(c.offset, c.end) !== c.source) throw Error('Stale component '+symbol);
  return {symbol, path:page.path, sha256:hash(text), offset:c.offset, end:c.end, source:c.source};
});
const config = source.module(1057), values = {};
walk(config.fn, node => {
  if (node.type === 'ObjectExpression' && ['minDPI','maxDPI','dpiStep'].every(key => node.properties.some(p => p.key?.name === key))) {
    for (const key of ['minDPI','maxDPI','dpiStep']) values[key] = source.literal(1057, node.properties.find(p => p.key?.name === key).value);
  }
});
if (values.minDPI !== 100 || values.maxDPI !== 50000 || values.dpiStep !== 1 || config.exports.has('SENSITIVITY_RANGE_VALUES')) throw Error('Re-audit product range');
receipts.push({symbol:'CONFIG', ...source.receipt(1057, config.fn)});
const env = {Bt:values.minDPI, Zt:values.maxDPI, Gt:values.dpiStep};
const mainPath=directory+'/'+manifest.files['main.js'].slice(2), mainText=read(mainPath);
let editingEnabledDefault;
walk(acorn.parse(mainText,{ecmaVersion:'latest'}),node=>{
 if(node.type==='VariableDeclarator' && ['Pe','ve'].includes(node.id?.name) && node.start>184000 && node.end<188000){
  if(node.id.name==='Pe'){
   editingEnabledDefault=source.literal(1057,node.init).enableStages;
   if(editingEnabledDefault!==true)throw Error('Re-audit editing default');
  }
  receipts.push({symbol:node.id.name,path:mainPath,sha256:hash(mainText),offset:node.start,end:node.end,source:mainText.slice(node.start,node.end)});
 }
});
for(const name of ['Pe','ve'])if(receipts.filter(r=>r.symbol===name).length!==1)throw Error('Missing DPI reducer '+name);
const labels = {};
for (const symbol of ['AFG','Q7f','gz7','IWp']) {
  const node=source.exported(4693,symbol);
  labels[symbol]=source.literal(4693,node);
  receipts.push({symbol:'label:'+symbol,...source.receipt(4693,source.binding(4693,node.name))});
}
function literal(n) {
  if (n.type === 'Literal') return n.value;
  if (n.type === 'Identifier' && Object.hasOwn(env,n.name)) return env[n.name];
  if (n.type === 'ArrayExpression') return n.elements.map(literal);
  if (n.type === 'ObjectExpression') return Object.fromEntries(n.properties.map(p=>[p.key.name ?? p.key.value,literal(p.value)]));
  if (n.type === 'BinaryExpression') {
    const a=literal(n.left), b=literal(n.right);
    if(n.operator==='*')return a*b;
    if(n.operator==='/')return a/b;
    if(n.operator==='-')return a-b;
  }
  throw Error('Nonliteral range expression '+n.type);
}
let segments;
walk(ast, node => {
  if (node.type === 'AssignmentExpression' && node.left?.property?.name === 'rangeValues'
      && node.start >= receipts[0].offset && node.end <= receipts[0].end) {
    if (segments) throw Error('Duplicate range assignment');
    segments=literal(node.right.right);
  }
  if (node.type === 'VariableDeclarator' && ['Bt','Zt','Gt','Ss','vs','Cs','Es','Ds','_s'].includes(node.id?.name)
      && node.start > 133000 && node.end < 150000) receipts.push({symbol:node.id.name,path:page.path,sha256:hash(text),offset:node.start,end:node.end,source:text.slice(node.start,node.end)});
});
if (!segments || segments.length !== 5) throw Error('Missing default ranges');
for(const [index,segment] of segments.entries()){
 const previous=segments[index-1];
 if(![segment.from,segment.to,segment.fromValue,segment.toValue,segment.step].every(Number.isFinite)
   || segment.to<=segment.from || segment.toValue<=segment.fromValue || segment.step<=0
   || (previous ? segment.from!==previous.to || segment.fromValue!==previous.toValue : segment.from!==0 || segment.fromValue!==values.minDPI)
   || (segment.tracks||[]).some(track=>!Number.isFinite(track)||track<=segment.from||track>=segment.to)
   || Math.abs((segment.to-segment.from)/(segment.toValue-segment.fromValue)*values.dpiStep-segment.step)>1e-12)
  throw Error('Invalid segmented DPI capability at segment '+index);
}
if(segments.at(-1).to!==100 || segments.at(-1).toValue!==values.maxDPI)throw Error('Incomplete DPI capability range');
const css=[];
for(const relative of [...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css'))){
 const file=directory+'/'+relative.slice(2), text=read(file);
 const rules=parseCSS(text).filter(r=>/^\.stage|^\.stages|^\.slider|range-custom|range-item|#drag-image|^\.icon-sensitivity-xy|^\.h1-body/.test(r.selector));
 if(rules.length)css.push({path:file,sha256:hash(text),rules});
}
const names=[['icon_sensitivity_xy.svg','sensitivity-xy.svg'],['icon_sensitivity_xy_active.svg','sensitivity-xy-active.svg'],['icon_sensitivity_xy_disabled.svg','sensitivity-xy-disabled.svg'],['icon_draggable_large.svg','dpi-draggable.svg'],...['red','green','blue','cyan','yellow'].map((name,i)=>[name+'.svg',`stage-${i+1}.svg`])];
const assets=names.map(([name,target])=>{
 const relative=manifest.files['static/media/'+name], file=directory+'/'+relative.slice(2), output='assets/synapse/'+target;
 const bytes=fs.readFileSync(path.join(root,file));
 if(!bytes.equals(fs.readFileSync(path.join(root,output))))throw Error('Asset differs: '+file);
 return {source:file,source_url:'https://apps.razer.com/synapse/products/226/ui/'+relative.slice(2),output,sha256:hash(bytes)};
});
function output(file,value){const text=JSON.stringify(value,null,2)+'\n';if(process.argv.includes('--check')){if(read(file)!==text)throw Error('Stale '+file);}else fs.writeFileSync(path.join(root,file),text);}
// The runtime dispatches by declared capability; this source receipt registers
// only the product whose mounted Ft implementation and reducer were audited.
output('src/features/mouse_dpi_grid_data.json',[{product_id:226,min:values.minDPI,max:values.maxDPI,step:values.dpiStep,editing_enabled_default:editingEnabledDefault,segments}]);
output('docs/re/mouse-226-dpi-current-evidence.json',{product_id:226,method:'AST/CSS and finite arithmetic evaluation only; UTF-16 offsets',manifest:{path:manifestPath,sha256:hash(read(manifestPath))},receipts,labels,css,assets});
console.log(`226 stages/grid: ${receipts.length} AST receipts, ${css.reduce((n,c)=>n+c.rules.length,0)} CSS rules, ${assets.length} assets`);
