// Current 1383 artwork data and components, interpreted as literal AST only.
const fs=require('fs'),path=require('path');
const {Source,walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),directory='.ref/devices/1383';
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
const manifest=JSON.parse(read(`${directory}/asset-manifest.json`));
const source=Object.create(Source.prototype);
Object.assign(source,{directory,files:[...new Set(Object.values(manifest.files))].filter(f=>f.includes('/static/js/')&&f.endsWith('.js')).map(f=>`${directory}/${f.slice(f.indexOf('static/'))}`),modules:new Map(),texts:new Map(),parsed:new Set()});
const receipts=[],assets=[],icons=[];
const binding=(module,symbol)=>{const n=source.binding(module,symbol);receipts.push({module,symbol,...source.receipt(module,n)});return n;};
const property=(node,name)=>{const p=node.properties.filter(p=>key(p.key)===name);if(p.length!==1)throw Error(`Expected property ${name}`);return p[0].value;};
function value(module,node){
 if(node.type==='Identifier')return value(module,source.binding(module,node.name));
 if(node.type==='ArrayExpression')return node.elements.map(n=>value(module,n));
 if(node.type==='ObjectExpression')return Object.fromEntries(node.properties.map(p=>[key(p.key),value(module,p.value)]));
 if(node.type==='BinaryExpression'&&node.operator==='+'&&node.right.type==='Literal'&&node.right.value.startsWith('static/media/'))return node.right.value;
 return source.literal(module,node);
}
const fps=value(46472,binding(46472,'U'));if(fps!==15)throw Error('Changed 1383 animation FPS');
const branch=binding(22502,'O');if(branch.type!=='ConditionalExpression'||source.snippet(22502,branch.test)!=='15===f')throw Error('Changed preset branch');
const animations=value(22502,branch.consequent).map(({id,name})=>({id,name,asset:`synapse/audio-oled-1383-${id}.webp`}));
const images=value(37769,binding(37769,'r')).map(({id,name})=>({id,name,asset:`synapse/audio-oled-1383-${id}.png`}));
const emoteNodes=binding(8816,'i');
const emotes=emoteNodes.elements.map(node=>{
 const id=value(8816,property(node,'id')),name=value(8816,property(node,'name')),loader=property(node,'src');
 const expression=source.snippet(8816,loader),match=/^\(\)=>n\.e\((\d+)\)\.then\(n\.t\.bind\(n,(\d+),17\)\)$/.exec(expression);
 if(!match)throw Error(`Changed emote lazy loader ${id}`);
 const module=Number(match[2]),assignments=[];walk(source.module(module).fn,n=>{if(n.type==='AssignmentExpression'&&key(n.left.property)==='exports')assignments.push(n.right);});
 if(assignments.length!==1)throw Error(`Ambiguous emote module ${id}`);
 const src=value(module,assignments[0]),asset=`synapse/audio-oled-1383-emote-${id}.webp`;
 assets.push({id,src,output:asset,receipt:source.receipt(module,assignments[0]),loader:source.receipt(8816,loader)});
 return {id,name,asset};
});
const defaultEmote=binding(8816,'s');
if(!source.snippet(8816,defaultEmote).includes('face-tongue-animated'))throw Error('Changed default emote');
const defaults=binding(79826,'T');
for(const [name,presets] of [['animation',animations],['image',images]]){
 const state=property(defaults,name),list=property(state,'list').elements;
 if(value(79826,property(state,'selectedIdx'))!==0||list.length!==presets.length)throw Error(`Changed default ${name} list`);
 list.forEach((node,index)=>{if(value(79826,property(node,'id'))!==presets[index].id||value(79826,property(node,'enabled'))!==true||value(79826,property(node,'custom'))!==false)throw Error(`Changed default slot ${name}:${index}`);});
}
const labels={};for(const symbol of ['scg','jJm','Qt7','PYB','Qg_','djP','VLI','bOp','pJk','pc9','Qj6','CHf','SKm','Yre','boQ','avz'])labels[symbol]=source.literal(54693,source.exported(54693,symbol));
for(const symbol of ['Mv','au','iu','Su','Cu','Vu','Pu','rh','nh','oh','hh','fh'])binding(51278,symbol);
for(const symbol of ['p','h','c','d','m'])binding(58837,symbol);
const tooltipModule=source.module(58837);
receipts.push({module:58837,symbol:'module/defaultProps',...source.receipt(58837,tooltipModule.fn)});
if(value(58837,source.binding(58837,'d'))!=='bottom-right'||!source.snippet(58837,tooltipModule.fn).includes('h.defaultProps={position:d,className:""}'))throw Error('Changed artwork tooltip default position');
const escape=v=>String(v).replaceAll('&','&amp;').replaceAll('"','&quot;').replaceAll('<','&lt;');
const attr=k=>({className:'class',strokeWidth:'stroke-width',strokeLinecap:'stroke-linecap',strokeLinejoin:'stroke-linejoin',clipPath:'clip-path',fillRule:'fill-rule',clipRule:'clip-rule',xmlnsXlink:'xmlns:xlink'}[k]||k);
function svg(n){
 if(!n||n.type==='ConditionalExpression')return '';
 if(n.type==='LogicalExpression'||n.type==='AssignmentExpression')return svg(n.right);
 if(n.type==='Literal')return n.value==null?'':escape(n.value);
 if(n.type!=='CallExpression'||n.callee.property?.name!=='createElement')throw Error('Nonliteral SVG child');
 const [tag,props,...children]=n.arguments,obj=props.type==='CallExpression'?props.arguments[0]:props;
 if(tag.type!=='Literal')throw Error('Nonliteral SVG tag');let attrs='';
 if(obj.type==='ObjectExpression')for(const p of obj.properties){const k=key(p.key);if(['ref','aria-labelledby','nonce'].includes(k))continue;if(p.value.type!=='Literal')throw Error('Nonliteral SVG attribute');attrs+=` ${attr(k)}="${escape(p.value.value)}"`;}
 else if(!(obj.type==='Literal'&&obj.value==null))throw Error('Nonliteral SVG props');
 return `<${tag.value}${attrs}>${children.map(svg).join('')}</${tag.value}>`;
}
// The memo/forwardRef wrappers name the literal SVG function. Resolve the
// variable's AST dependency in the same current module, not another product.
for(const [module,symbol,id] of [[51278,'hh','replace'],[51278,'fh','reset'],[51278,'Qp','zoom-out'],[51278,'Kp','zoom-in'],[51278,'bd','info'],[51278,'Uh','not-found'],[49077,'l','search']]){
 let node=source.binding(module,symbol),roots=[];const visited=new Set();
 const scan=n=>walk(n,n=>{if(n.type==='CallExpression'&&n.callee.property?.name==='createElement'&&n.arguments[0]?.value==='svg')roots.push(n);if(n.type==='Identifier'&&!visited.has(n.name)){visited.add(n.name);try{const binding=source.binding(module,n.name);if(binding!==n&&binding.type!=='Literal')scan(binding);}catch{}}});
 scan(node);roots=[...new Map(roots.map(n=>[n.start,n])).values()];if(roots.length!==1)throw Error(`Ambiguous ${id} icon: ${roots.length}`);
 icons.push({id,output:`synapse/audio-oled-artwork-${id}.svg`,svg:svg(roots[0])+'\n',receipt:source.receipt(module,roots[0])});
}
const cssPaths=[...new Set(Object.values(manifest.files))].filter(f=>f.includes('/static/css/')&&f.endsWith('.css')).map(f=>`${directory}/${f.slice(f.indexOf('static/'))}`);
const css=cssPaths.flatMap(file=>parseCSS(read(file)).filter(r=>/CustomizeAnimation_|CustomizeImage_|CustomizeEmote_|CustomizeModal_|animation-crop|zoom-level-slider|reset-cropper-btn|\.cropper-|^\.tooltip-razer/.test(r.selector)).map(r=>({path:file,sha256:hash(read(file)),...r})));
const data={product_id:1383,fps,labels,animations,images,emotes,default_emote:'face-tongue-animated'};
const evidence={method:'Current 1383 manifest, literal AST and CSS only; no vendor JavaScript executed.',manifest:{path:`${directory}/asset-manifest.json`,sha256:hash(read(`${directory}/asset-manifest.json`))},receipts,assets,icons,css};
const check=process.argv.slice(2).join(' ')==='--check';if(process.argv.length>2&&!check)throw Error('Unknown argument');
for(const [file,payload] of [['crates/razer-pages/src/features/audio_oled_artwork_data.json',data],['docs/re/audio-oled-artwork-current-evidence.json',evidence]]){const bytes=JSON.stringify(payload,null,2)+'\n';if(check){if(read(file)!==bytes)throw Error(`Stale ${file}`);}else fs.writeFileSync(path.join(root,file),bytes);}
console.log(`1383 artwork: ${animations.length} animation slots, ${images.length} image slots, ${emotes.length} lazy emotes and ${icons.length} literal SVGs resolved.`);
