// Product 1383 only. Resolve source data with an explicit AST literal grammar.
// This program never evaluates or imports downloaded JavaScript.
const fs=require('fs'),path=require('path');
const {Source,walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),directory='.ref/devices/1383';
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
const manifest=JSON.parse(read(`${directory}/asset-manifest.json`));
const source=Object.create(Source.prototype);
source.directory=directory;
source.files=[...new Set(Object.values(manifest.files))].filter(file=>file.includes('/static/js/')&&file.endsWith('.js')).map(file=>`${directory}/${file.slice(file.indexOf('static/'))}`);
source.modules=new Map();source.texts=new Map();source.parsed=new Set();
const owner=51278,receipts=[],assets=[],icons=[];
const binding=(id,name)=>{const node=source.binding(id,name);receipts.push({module:id,symbol:name,...source.receipt(id,node)});return node;};
function value(id,node){
 if(node.type==='Identifier')return value(id,source.binding(id,node.name));
 if(node.type==='ArrayExpression')return node.elements.map(n=>value(id,n));
 if(node.type==='ObjectExpression')return Object.fromEntries(node.properties.map(p=>{if(p.type!=='Property'||p.computed)throw Error('Nonliteral property');return[key(p.key),value(id,p.value)];}));
 if(node.type==='BinaryExpression'&&node.operator==='+'&&node.right.type==='Literal'&&/^static\/media\//.test(node.right.value))return node.right.value;
 return source.literal(id,node);
}
function property(node,name){const p=node.properties.filter(p=>key(p.key)===name);if(p.length!==1)throw Error(`Expected property ${name}`);return p[0].value;}
function asset(id,src,module,node,animated=false){
 const output=`synapse/audio-oled-1383-${id}.${animated?'webp':'png'}`;
 assets.push({id,src,output,module,receipt:source.receipt(module,node),animated});return output;
}
const defaults=binding(79826,'T');
const fps=source.literal(46472,binding(46472,'U'));if(fps!==15)throw Error('1383 FPS branch changed');
const animationBranch=binding(22502,'O');
if(animationBranch.type!=='ConditionalExpression'||source.snippet(22502,animationBranch.test)!=='15===f')throw Error('Animation branch changed');
const animations=value(22502,animationBranch.consequent).map(item=>({...item,asset:asset(item.id,item.src,22502,animationBranch,true)}));
const imageNode=binding(37769,'r');
const images=value(37769,imageNode).map(item=>({...item,asset:asset(item.id,item.src,37769,imageNode)}));
const emotes=binding(8816,'i'),emote=emotes.elements.find(n=>value(8816,property(n,'id'))==='face-tongue-animated');
const loader=source.snippet(8816,property(emote,'src'));
if(loader!=='()=>n.e(6418).then(n.t.bind(n,76418,17))')throw Error('Emote lazy branch changed');
const lazy=source.module(76418),assignments=[];
walk(lazy.fn,n=>{if(n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&key(n.left.property)==='exports')assignments.push(n.right);});
if(assignments.length!==1)throw Error('Ambiguous emote asset');
const emoteAsset=asset('emote-default',source.literal(76418,assignments[0]),76418,assignments[0],true);
const bannerNode=binding(9483,'o'),banner=value(9483,bannerNode)[0];
const bannerAsset=asset('banner-default',banner.src,9483,bannerNode);
const visualizers=['Wg','Gg','zg'].map((name,index)=>{const n=binding(owner,name);return asset(`visualizer-${index}`,value(owner,n),owner,n,true);});
const headsetNode=binding(owner,'wv'),headset=[];
walk(headsetNode,n=>{if(n.type==='Literal'&&typeof n.value==='string'&&n.value.startsWith('data:image/png;base64,'))headset.push(asset(`headset-${headset.length}`,n.value,owner,n));});
if(headset.length!==3)throw Error('Headset glyph count changed');
const home=value(79826,property(defaults,'homeScreenDisplay')),media=value(79826,property(defaults,'media'));
const system=property(defaults,'system'),systemInfo=value(79826,property(system,'info'));
const systemSlideNodes=property(system,'slides').elements;
const tagNode=binding(96373,'p'),tags=tagNode.elements[0].argument.elements;
const slides=systemSlideNodes.map(slide=>['left','right'].map(side=>{
 const access=property(slide,side);if(access.type!=='MemberExpression'||!access.computed)throw Error('System tag access changed');
 return value(96373,property(tags[access.property.value],'id'));
}));
const labels={};
for(const name of ['JQR','O0W','Tcb','RG','KTY','Nj5','$Mz','rwQ','FSz','w23','zxY','pJk','$yX','VLI','VKN','CMJ','wv5','_NK','RYc','GOv','wLL','bOp','vs6','XVF'])labels[name]=source.literal(54693,source.exported(54693,name));
for(const name of ['Dv','Hp','Bp','su','_u','Pu','Lu','Mv','Kg','Zg','Hg','vv','Tn','oh','mp','hp','Ap','vp','Cp'])binding(owner,name);
binding(58837,'p');binding(58837,'h');binding(49412,'d');binding(50151,'o');
const escape=v=>String(v).replaceAll('&','&amp;').replaceAll('"','&quot;').replaceAll('<','&lt;');
const attrName=k=>({className:'class',strokeWidth:'stroke-width',strokeLinecap:'stroke-linecap',strokeLinejoin:'stroke-linejoin',clipPath:'clip-path',fillRule:'fill-rule',clipRule:'clip-rule',xmlnsXlink:'xmlns:xlink'}[k]||k);
function svg(n){
 if(!n||n.type==='ConditionalExpression')return '';
 if(n.type==='LogicalExpression'||n.type==='AssignmentExpression')return svg(n.right);
 if(n.type==='Literal')return n.value==null?'':escape(n.value);
 if(n.type!=='CallExpression'||n.callee.property?.name!=='createElement')throw Error('Nonliteral SVG child');
 const [tag,props,...children]=n.arguments,obj=props.type==='CallExpression'?props.arguments[0]:props;
 if(tag.type!=='Literal')throw Error('Nonliteral SVG tag');
 let attrs='';if(obj.type==='ObjectExpression')for(const p of obj.properties){const k=key(p.key);if(['ref','aria-labelledby','nonce'].includes(k))continue;if(p.value.type!=='Literal')throw Error('Nonliteral SVG attribute');attrs+=` ${attrName(k)}="${escape(p.value.value)}"`;}
 else if(!(obj.type==='Literal'&&obj.value==null))throw Error('Nonliteral SVG props');
 return `<${tag.value}${attrs}>${children.map(svg).join('')}</${tag.value}>`;
}
for(const [symbol,id]of [['Ap','requires-synapse'],['vp','edit'],['Cp','apply']]){
 const node=source.binding(owner,symbol),roots=[];walk(node,n=>{if(n.type==='CallExpression'&&n.callee.property?.name==='createElement'&&n.arguments[0]?.value==='svg')roots.push(n);});
 if(roots.length!==1)throw Error('Ambiguous SVG root');
 icons.push({id,output:`synapse/audio-oled-1383-${id}.svg`,svg:svg(roots[0])+'\n',receipt:source.receipt(owner,node)});
}
const cssPaths=[...new Set(Object.values(manifest.files))].filter(f=>f.includes('/static/css/')&&f.endsWith('.css')).map(f=>`${directory}/${f.slice(f.indexOf('static/'))}`);
const css=cssPaths.flatMap(file=>parseCSS(read(file)).filter(rule=>/HomeScreenDisplay_|DisplayWidget_|CustomizeBanner_|CustomizeSystemInfo_|CustomizeMedia_|CustomizeModal_|CustomizeHeadsetStatus_|\.choose-a-mat|\.backdrop|\.widget|\.body-widgets|\.radio-item|type=radio|customize-setting-button|^\.disabled|^\.tooltip-razer/.test(rule.selector)).map(rule=>({path:file,sha256:hash(read(file)),...rule})));
const animationsCSS=[],fonts=[];
for(const file of cssPaths){
 const text=read(file);
 for(const match of text.matchAll(/@keyframes CustomizeBanner_[^{]+\{/g)){
  let end=match.index+match[0].length,depth=1;while(depth&&end<text.length){if(text[end]==='{')depth++;else if(text[end]==='}')depth--;end++;}
  if(depth)throw Error('Unclosed banner keyframes');animationsCSS.push({path:file,sha256:hash(text),offset:match.index,end,source:text.slice(match.index,end)});
 }
 for(const match of text.matchAll(/@font-face\{[^}]+\}/g))if(/font-family:(Roboto|RazerF5);/.test(match[0]))fonts.push({path:file,sha256:hash(text),offset:match.index,end:match.index+match[0].length,source:match[0]});
}
const batteryDefault=value(33397,property(binding(33397,'Rt'),'batteryValue'));
const systemLabelKeys=Object.fromEntries(tags.filter(n=>property(n,'id')).map(n=>[value(96373,property(n,'id')),value(96373,property(n,'deviceLabel'))]));
const data={product_id:1383,labels,home,media,animations:animations.map(({id,name,asset})=>({id,name,asset})),images:images.map(({id,name,asset})=>({id,name,asset})),emote:emoteAsset,banner:bannerAsset,visualizers,headset,battery_default:batteryDefault,system_info:systemInfo,system_label_keys:systemLabelKeys,system_slides:slides,system_interval:value(79826,property(system,'timeBetweenSlides')),banner_text:value(79826,property(property(property(defaults,'banner'),'text'),'value')),banner_font:value(9483,binding(9483,'r'))[0].name,banner_font_size:value(9483,binding(9483,'a'))[8].name};
const evidence={method:'Manifest-declared current 1383 Acorn AST and CSS only. Offsets use UTF-16 code units. No vendor JavaScript executed.',manifest:{path:`${directory}/asset-manifest.json`,sha256:hash(read(`${directory}/asset-manifest.json`))},receipts,assets,icons,shared:[{src:'static/media/icon_close.55fe41f1.svg',output:'assets/synapse/mapping-close.svg'}],css,keyframes:animationsCSS,fonts};
const flags=process.argv.slice(2);if(flags.some(f=>f!=='--check'))throw Error('Unknown argument');
for(const [file,content]of [['crates/razer-pages/src/features/audio_oled_home_data.json',data],['docs/re/audio-oled-home-source.json',evidence]]){
 const out=JSON.stringify(content,null,2)+'\n';if(flags.includes('--check')){if(read(file)!==out)throw Error(`Stale ${file}`);}else fs.writeFileSync(path.join(root,file),out);
}
console.log('1383 OLED home: seven card branches, 15 fps presets, media editor and current CSS resolved statically.');
