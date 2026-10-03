// Parse current product bundles as data. Never execute downloaded JavaScript.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const hash = b => crypto.createHash('sha256').update(b).digest('hex');
function walk(n, visit) { if (!n?.type || visit(n) === false) return; for (const v of Object.values(n)) if (Array.isArray(v)) v.forEach(c => walk(c, visit)); else if (v?.type) walk(v, visit); }
function literal(n) {
  if (n?.type === 'Literal') return n.value;
  if (n?.type === 'ArrayExpression') return n.elements.map(literal);
  if (n?.type === 'UnaryExpression' && n.operator === '!') return !literal(n.argument);
  if (n?.type === 'ObjectExpression') return Object.fromEntries(n.properties.map(p => [p.key.name ?? p.key.value, literal(p.value)]));
  throw Error('Not literal: ' + n?.type);
}
function exportsOf(module) {
  const values = new Map(), refs = new Map();
  walk(module, n => {
    if (n.type === 'VariableDeclarator' && n.id.type === 'Identifier') { try { values.set(n.id.name, literal(n.init)); } catch {} }
    if (n.type === 'CallExpression' && n.callee.property?.name === 'd' && n.arguments[1]?.type === 'ObjectExpression') for (const p of n.arguments[1].properties) {
      const body = p.value.body;
      const ref = body.type === 'BlockStatement' ? body.body.find(n => n.type === 'ReturnStatement')?.argument : body;
      if (ref?.type === 'Identifier') refs.set(p.key.name ?? p.key.value, ref.name);
    }
  });
  return Object.fromEntries([...refs].filter(([k,v]) => values.has(v)).map(([k,v]) => [k, values.get(v)]));
}
function emit(p, v) { const out = JSON.stringify(v, null, 2) + '\n'; if (process.argv.includes('--check')) { if (read(p) !== out) throw Error('Stale ' + p); } else fs.writeFileSync(path.join(root, p), out); }
const escapeXml=value=>String(value).replace(/&/g,'&amp;').replace(/"/g,'&quot;').replace(/</g,'&lt;');
function svgElement(call) {
  if(call?.type!=='CallExpression'||!['jsx','jsxs'].includes(call.callee.expressions?.at(-1)?.property?.name))return '';
  const tag=call.arguments[0]?.value;if(typeof tag!=='string')return '';
  const props={};walk(call.arguments[1],n=>{if(n.type==='ObjectExpression'){for(const p of n.properties){if(p.type==='Property')props[p.key.name??p.key.value]=p.value;}return false;}});
  let attributes='';for(const [key,value]of Object.entries(props)){if(key==='children'||key==='ref'||key.startsWith('on'))continue;try{const v=literal(value);if(['string','number'].includes(typeof v))attributes+=' '+({className:'class',clipPath:'clip-path',strokeWidth:'stroke-width',strokeLinecap:'stroke-linecap',strokeLinejoin:'stroke-linejoin'}[key]??key)+'="'+escapeXml(v)+'"';}catch{}}
  const children=props.children?.type==='ArrayExpression'?props.children.elements:[props.children];
  return '<'+tag+attributes+'>'+children.map(svgElement).join('')+'</'+tag+'>';
}
const specs = [], evidence = [];
for (const pid of [3884, 3886]) {
  const receipt = JSON.parse(read(`docs/re/pending-product-${pid}-source.json`));
  for (const f of receipt.source_files) if (hash(read(f.path)) !== f.sha256) throw Error('Changed ' + f.path);
  const sourcePath = receipt.source_files[0].path, source = read(sourcePath), ast = acorn.parse(source, {ecmaVersion:'latest'});
  const modules = new Map();
  walk(ast, n => { if (n.type === 'Property' && typeof n.key.value === 'number' && ['ArrowFunctionExpression', 'FunctionExpression'].includes(n.value.type)) modules.set(n.key.value, n.value); });
  const components = receipt.pages[0].components.filter(c => c.offset > (pid === 3884 ? 4496000 : 3986000));
  const labelNames = [...new Set(components.flatMap(c => [...c.source.matchAll(pid === 3884 ? /OT\.([\w$]+)/g : /Uo\.([\w$]+)/g)].map(m => m[1])))];
  const allLabels = exportsOf(modules.get(pid === 3884 ? 4693 : 8098));
  const labels = Object.fromEntries(labelNames.map(k => { if (!allLabels[k]) throw Error('Missing label ' + k); return [k, allLabels[k]]; }));
  const keys = new Set([...Object.values(labels),'TEXT_FAN','TEXT_LED_STRIP','TEXT_PORT']), translations = {}, translation_receipts = [];
  // Locale tables are literal CommonJS export dictionaries in these bundles.
  for (const [id, module] of modules) {
    const candidates=[], bindings=new Map();
    walk(module,n=>{if(n.type==='VariableDeclarator'&&n.init?.type==='Literal')bindings.set(n.id.name,n.init.value);});
    walk(module,n=>{if(n.type!=='ObjectExpression'||!n.properties.some(p=>keys.has(p.key?.name??p.key?.value)))return;const values={};for(const p of n.properties){const k=p.key?.name??p.key?.value;if(!keys.has(k))continue;let v=p.value;if(v?.type==='ArrowFunctionExpression')v=v.body;if(v?.type==='FunctionExpression')v=v.body.body.find(s=>s.type==='ReturnStatement')?.argument;const value=v?.type==='Literal'?v.value:bindings.get(v?.name);if(typeof value==='string')values[k]=value;}if(Object.keys(values).length>5)candidates.push({values,offset:n.start,end:n.end});});
    for(const candidateTable of candidates){
    const values=candidateTable.values;
    let locale;
    for (const candidate of ['en','zh-CN','zh-TW','de','es','fr','ja','kr','pt-BR','ru']) {
      const dictionary = JSON.parse(read(`locales/${candidate}.json`));
      const pairs = Object.entries(values).filter(([k,v]) => keys.has(k) && typeof v === 'string');
      if (pairs.filter(([k,v]) => dictionary[k] === v).length > pairs.length / 2) { locale = candidate; break; }
    }
    if (!locale) continue;
    translations[locale] = Object.fromEntries(Object.entries(values).filter(([k,v]) => keys.has(k) && typeof v === 'string'));
    translation_receipts.push({locale,module:id,offset:candidateTable.offset,end:candidateTable.end});
    }
  }
  const constants = [];
  walk(ast, n => {
    if (n.type === 'VariableDeclarator' && n.start > (pid === 3884 ? 3900000 : 3800000)) {
      try { const value = literal(n.init); if ((Array.isArray(value) && JSON.stringify(value) === '[6,8,9,10,12,15,16,18,20,22,24,25,32,40]') || (value && typeof value === 'object' && 'BLE_MOBIL' in value)) constants.push({name:n.id.name,value,offset:n.start,end:n.end}); } catch {}
    }
  });
  const metadata = [];
  walk(ast,n=>{if(n.type==='ObjectExpression' && n.properties.some(p=>p.key?.name==='productId' && p.value?.value===pid)){try{metadata.push({value:literal(n),offset:n.start,end:n.end});}catch{}}});
  if(metadata.length!==1)throw Error('Missing product metadata');
  const manifestPath = `.ref/devices/${pid}/asset-manifest.json`, manifest = JSON.parse(read(manifestPath));
  const assetModules = pid === 3884 ? [2966,941,7408,8519,5766] : [239,2266,3718,5907,9817];
  const assets = assetModules.map((id,ix) => {
    const module = modules.get(id), text = source.slice(module.start,module.end), request = text.match(/static\/media\/[^"']+/)?.[0];
    const name = ['fan','strip-2','strip-3','strip-4','strip-1'][ix];
    if(!request){let data;walk(module,n=>{if(n.type==='Literal'&&typeof n.value==='string'&&n.value.startsWith('data:image/'))data=n.value;});if(!data)throw Error('Invalid embedded asset '+id);return {module:id,bundle:sourcePath,data,output:`assets/synapse/wireless-argb-${pid}-${name}.png`};}
    if (!Object.values(manifest.files).some(v => v.replace(/^\.\//,'') === request)) throw Error('Invalid asset '+id);
    return {module:id,source:`.ref/devices/${pid}/`+request,url:`https://apps.razer.com/synapse/products/${pid}/ui/`+request,output:`assets/synapse/wireless-argb-${pid}-${name}.png`};
  });
  for (const name of ['prd-3x','detecting']) {
    const matches = [...new Set(Object.values(manifest.files))].filter(v=>new RegExp('/'+name+'\\.[a-f0-9]+\\.(avif|svg)$').test(v));
    if(matches.length!==1)throw Error('Missing '+name);
    const request=matches[0].replace(/^\.\//,'');
    assets.push({source:`.ref/devices/${pid}/`+request,url:`https://apps.razer.com/synapse/products/${pid}/ui/`+request,output:`assets/synapse/wireless-argb-${pid}-${name}.`+(name==='detecting'?'svg':'png')});
  }
  const css = fs.readdirSync(path.join(root,`.ref/devices/${pid}/static/css`)).filter(f=>f.endsWith('.css')).map(f=>{
    const p=`.ref/devices/${pid}/static/css/`+f,s=read(p);
    return {path:p,sha256:hash(s),rules:s.split('}').filter(r=>/customize-(container|center|scroll)|port-(container|infomation|items|name|input|add-bend)|number-led|total-LED|wireless-status|warning-container|message-container|product__|led-img|chroma-studio-message/.test(r.split('{')[0])).map(r=>r+'}')};
  });
  const iconOffsets=pid===3884?{auto_on:4497226,auto_off:4496627,refresh:4497885,warning:4498341,power:4512637,rename:4505333,remove:4504118}:{auto_on:3986778,auto_off:3986046,refresh:3987570,warning:3988141,power:4007329,rename:3999520,remove:3998045};
  for(const [name,offset]of Object.entries(iconOffsets)){
    const component=components.find(c=>c.offset===offset);if(!component)throw Error('Missing icon '+name);
    const local=acorn.parse('('+component.source+')',{ecmaVersion:'latest'});let svg;
    walk(local,n=>{if(!svg&&n.type==='CallExpression'&&n.arguments[0]?.value==='svg')svg=svgElement(n);});
    if(!svg)throw Error('No SVG '+name);
    // Preserve source CSS artwork colors and shapes inside each isolated SVG.
    const styles=css.flatMap(c=>c.rules).filter(r=>r.includes('.icon-')&&!r.includes(':hover')&&!r.includes(':active')&&!r.includes('port-item')).map(r=>r.replaceAll('#multipleBrightness .customize-scroll .customize-center .product ','').replaceAll('.icon-detection-wrapper ','').replaceAll('.icon-refreshing-wrapper ','').replace(/(?:-webkit-)?animation:[^;}]+;?/g,'').replace(/transition:[^;}]+;?/g,'')).join('');
    if(name==='power')svg=svg.replace('<svg','<svg class="icon-power"');
    if(name==='auto_off'&&!/^<svg[^>]*class=/.test(svg))svg=svg.replace('<svg','<svg class="icon-detection"');
    if(!/^<svg[^>]*xmlns=/.test(svg))svg=svg.replace('<svg','<svg xmlns="http://www.w3.org/2000/svg"');
    svg=svg.replace('</svg>','<style>'+styles+'</style></svg>');
    assets.push({bundle:sourcePath,offset,end:component.end,data:'data:image/svg+xml;base64,'+Buffer.from(svg).toString('base64'),output:`assets/synapse/wireless-argb-${pid}-${name}.svg`});
    if(name==='power')assets.push({bundle:sourcePath,offset,end:component.end,data:'data:image/svg+xml;base64,'+Buffer.from(svg.replace('class="icon-power"','class="icon-power icon-power--power-off"')).toString('base64'),output:`assets/synapse/wireless-argb-${pid}-power_off.svg`});
  }
  const reducerSlices = [];
  for (const needle of pid===3884?['case Yt:','case Wt:','case wt:','Ff=(e,']:['case Qr:','case Jr:','case $r:','g7=function']) {
    const offset=source.indexOf(needle);if(offset<0)throw Error('Missing reducer '+needle);
    reducerSlices.push({offset,source:source.slice(offset,offset+1800)});
  }
  specs.push({product_id:pid,page:'TAB_CUSTOMIZE',metadata:metadata[0].value,labels,translations,fan_values:constants.find(c=>Array.isArray(c.value)).value,assets});
  const profile_bar=[];
  for(const needle of ['isEnableProfileBar','setProfileDropdownState','enableSwitchProfile:','enableSwitchProfile='])for(const match of source.matchAll(new RegExp(needle.replace(/[.*+?^${}()|[\]\\]/g,'\\$&'),'g')))profile_bar.push({needle,offset:Math.max(0,match.index-160),source:source.slice(Math.max(0,match.index-160),match.index+400)});
  const persistence=[];
  for(const needle of pid===3884?['this.loadActiveProfileSettings=()=>','const eH=(e,E)=>','portsReducer:']:['r.loadActiveProfileSettings=function()','var k7=function(e,t)','portsReducer:']){
    const start=source.indexOf(needle);if(start<0)throw Error('Missing persistence '+needle);
    const offset=needle==='portsReducer:'?Math.max(0,start-250):start;
    persistence.push({needle,offset,source:source.slice(offset,offset+(needle.includes('loadActive')?3100:1100))});
  }
  evidence.push({product_id:pid,source_files:receipt.source_files,label_module:pid===3884?4693:8098,labels,translation_receipts,metadata,constants,css,profile_bar,persistence,manifest:{path:manifestPath,sha256:hash(read(manifestPath))},components:components.map(({path,offset,end,source})=>({path,offset,end,source})),reducerSlices});
}
emit('src/features/wireless_argb_data.json',specs);
emit('docs/re/wireless-argb-current-evidence.json',{method:'Acorn literal and mounted source extraction; vendor code never executed',generator_sha256:hash(fs.readFileSync(__filename)),products:evidence});
console.log('Extracted current wireless ARGB products, resources, states and constraints.');
