// Current product 1383 System Info editor, statically parsed without evaluation.
const fs=require('fs'),path=require('path');
const {Source,walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),directory='.ref/devices/1383';
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
const manifestPath=`${directory}/asset-manifest.json`,manifest=JSON.parse(read(manifestPath));
const source=Object.create(Source.prototype);
source.directory=directory;
source.files=[...new Set(Object.values(manifest.files))].filter(f=>f.includes('/static/js/')&&f.endsWith('.js')).map(f=>`${directory}/${f.slice(f.indexOf('static/'))}`);
source.modules=new Map();source.texts=new Map();source.parsed=new Set();
const owner=51278,receipts=[],assets=[];
const prop=(node,name)=>{const matches=node.properties.filter(p=>key(p.key)===name);if(matches.length!==1)throw Error(`Expected property ${name}`);return matches[0].value;};
function binding(id,name){const node=source.binding(id,name);receipts.push({module:id,symbol:name,...source.receipt(id,node)});return node;}
function value(id,node){
 if(node.type==='Identifier')return value(id,source.binding(id,node.name));
 if(node.type==='ArrayExpression')return node.elements.flatMap(n=>n.type==='SpreadElement'?value(id,n.argument):[value(id,n)]);
 if(node.type==='ObjectExpression')return Object.fromEntries(node.properties.map(p=>{if(p.type!=='Property'||p.computed)throw Error('Nonliteral property');return[key(p.key),value(id,p.value)];}));
 if(node.type==='BinaryExpression'&&node.operator==='+'&&node.right.type==='Literal'&&/^static\/media\//.test(node.right.value)&&node.left.type==='MemberExpression'&&key(node.left.property)==='p')return node.right.value;
 if(node.type==='MemberExpression'){
  if(node.object.type==='Identifier'){
   const imported=source.binding(id,node.object.name);
   if(imported.type==='CallExpression'&&imported.callee.name===source.module(id).fn.params[2].name&&imported.arguments[0].type==='Literal'){
    const target=imported.arguments[0].value;return value(target,source.exported(target,key(node.property)));
   }
  }
  return value(id,node.object)[node.computed?value(id,node.property):key(node.property)];
 }
 return source.literal(id,node);
}
function asset(name,src,id,node){const ext=src.startsWith('data:')?'png':'svg',output=`synapse/audio-oled-1383-system-${name}.${ext}`;assets.push({id:name,src,output,receipt:source.receipt(id,node)});return output;}
const tagNode=binding(96373,'p'),tags=value(96373,tagNode),configNode=binding(46472,'Te');
if(tags.length!==9||tags.at(-1).id!=='headsetBattery')throw Error('1383 tag catalog changed');
const tagAssets=tags.map(tag=>{const node=tag.id==='headsetBattery'?configNode:tagNode;const id=tag.id==='headsetBattery'?46472:96373;return {...tag,previewAsset:asset(`tag-${tag.id}`,tag.previewSrc,id,node)};});
const defaults=value(79826,prop(binding(79826,'T'),'system'));
const titles=value(96373,binding(96373,'A')),intervals=value(96373,binding(96373,'d')),dateFormats=value(owner,binding(owner,'fv'));
const editor=binding(owner,'_v');
for(const name of ['gv','xv','uv','hv','vv','hp','av','Mv','Tn','oh'])binding(owner,name);
const dots=[];walk(editor,n=>{if(n.type==='Literal'&&typeof n.value==='string'&&n.value.startsWith('data:image/png;base64,'))dots.push(asset(`dot-${dots.length}`,n.value,owner,n));});
if(dots.length!==2)throw Error('Preview dots changed');
const batteries=['lv','dv','cv','mv','pv'].map((symbol,index)=>{const n=binding(owner,symbol);return asset(`battery-${index*25}`,value(owner,n),owner,n);});
const labels={};for(const symbol of ['G9b','$yX','VLI','OcK','btK','RtR','OLp','tI_','qu5','hIg','ylX','SJi','EyM','eWG','pJk','bOp'])labels[symbol]=source.literal(54693,source.exported(54693,symbol));
const escape=v=>String(v).replaceAll('&','&amp;').replaceAll('"','&quot;').replaceAll('<','&lt;');
const attrName=k=>({className:'class',strokeWidth:'stroke-width',strokeLinecap:'stroke-linecap',strokeLinejoin:'stroke-linejoin',clipPath:'clip-path',fillRule:'fill-rule',clipRule:'clip-rule',xmlnsXlink:'xmlns:xlink'}[k]||k);
function svg(n){
 if(!n||n.type==='ConditionalExpression')return '';
 if(n.type==='LogicalExpression'||n.type==='AssignmentExpression')return svg(n.right);
 if(n.type==='Literal')return n.value==null?'':escape(n.value);
 if(n.type!=='CallExpression'||n.callee.property?.name!=='createElement')throw Error('Nonliteral SVG child');
 const [tag,props,...children]=n.arguments,obj=props.type==='CallExpression'?props.arguments[0]:props;if(tag.type!=='Literal')throw Error('Nonliteral SVG tag');
 let attrs='';if(obj.type==='ObjectExpression')for(const p of obj.properties){const k=key(p.key);if(['ref','aria-labelledby','nonce'].includes(k))continue;if(p.value.type!=='Literal')throw Error('Nonliteral SVG attribute');attrs+=` ${attrName(k)}="${escape(p.value.value)}"`;}
 else if(!(obj.type==='Literal'&&obj.value==null))throw Error('Nonliteral SVG props');
 return `<${tag.value}${attrs}>${children.map(svg).join('')}</${tag.value}>`;
}
const icons=[];for(const [id,name,symbol]of [[67444,'add','r'],[owner,'delete','av']]){
 const node=binding(id,symbol),roots=[];walk(node,n=>{if(n.type==='CallExpression'&&n.callee.property?.name==='createElement'&&n.arguments[0]?.value==='svg')roots.push(n);});if(roots.length!==1)throw Error('SVG root changed');
 icons.push({id:name,output:`synapse/audio-oled-1383-system-${name}.svg`,svg:svg(roots[0])+'\n',receipt:source.receipt(id,node)});
}
const cssPaths=[...new Set(Object.values(manifest.files))].filter(f=>f.includes('/static/css/')&&f.endsWith('.css')).map(f=>`${directory}/${f.slice(f.indexOf('static/'))}`);
const css=cssPaths.flatMap(file=>parseCSS(read(file)).filter(r=>/CustomizeSystemInfo_|DisplayWidget_|CustomizeModal_|^\.disabled|\[tooltip\]|info-icon|\.info/.test(r.selector)).map(r=>({path:file,sha256:hash(read(file)),...r})));
const helpRules=cssPaths.flatMap(file=>parseCSS(read(file)).filter(r=>/^\.tooltip_parent(?: |:)|^\.drop-tips/.test(r.selector)).map(r=>({path:file,sha256:hash(read(file)),...r})));
css.push(...helpRules);binding(61050,'s');
const help=helpRules.find(r=>r.selector==='.tooltip_parent .help'&&r.declarations.includes('tooltip_questionmark.96138d2f.svg'));
if(!help)throw Error('Current inline help glyph changed');
const helpSource=read(help.path),helpEnd=helpSource.indexOf('}',help.offset)+1;
assets.push({id:'help',src:'static/media/tooltip_questionmark.96138d2f.svg',output:'synapse/audio-oled-1383-system-help.svg',receipt:{path:help.path,sha256:help.sha256,offset:help.offset,end:helpEnd,source:helpSource.slice(help.offset,helpEnd)}});
const data={product_id:1383,default:defaults,tags:tagAssets,slide_titles:titles,intervals,date_formats:dateFormats,labels,dots,batteries,icons:{...Object.fromEntries(icons.map(i=>[i.id,i.output])),help:'synapse/audio-oled-1383-system-help.svg'}};
const evidence={method:'1383 current manifest, Acorn AST literal/export resolution, CSS receipts; no downloaded code execution. Offsets: UTF-16 code units.',manifest:{path:manifestPath,sha256:hash(read(manifestPath))},receipts,assets,icons,css};
const flags=process.argv.slice(2);if(flags.some(f=>f!=='--check'))throw Error('Unknown argument');
for(const [file,content]of [['src/features/audio_oled_system_data.json',data],['docs/re/audio-oled-system-source.json',evidence]]){
 const out=JSON.stringify(content,null,2)+'\n';if(flags.includes('--check')){if(read(file)!==out)throw Error(`Stale ${file}`);}else fs.writeFileSync(path.join(root,file),out);
}
console.log('1383 System Info: nine tags, three fixed slides, defaults, source editor and CSS resolved.');
