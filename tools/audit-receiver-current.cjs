// Current product 179 mounted Customize, CSS and inline SVGs. No vendor execution.
const fs=require('fs'),path=require('path');
const {Source,walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),check=process.argv.includes('--check');
const read=p=>fs.readFileSync(path.join(root,p),'utf8');
const s=Object.create(Source.prototype);s.directory='.ref/devices/179';
const manifest=JSON.parse(read(s.directory+'/asset-manifest.json'));
s.files=[...new Set(Object.values(manifest.files))].filter(f=>/^\.\/static\/js\/[^/]+\.js$/.test(f)).map(f=>s.directory+'/'+f.slice(2));
s.modules=new Map;s.texts=new Map;s.parsed=new Set;
const receipt=(id,name)=>s.receipt(id,s.binding(id,name));
const save=(file,output)=>{if(check){if(read(file)!==output)throw Error('Stale '+file);}else{fs.mkdirSync(path.dirname(path.join(root,file)),{recursive:true});fs.writeFileSync(path.join(root,file),output)}};
const cssFile=s.directory+'/static/css/main.c778525e.css',css=read(cssFile);
const roots=['mE','OE','Te','$e','AE','Ie','eE','rE','sE','TE','G','se','x'];
const snippets=Object.fromEntries(roots.map(name=>[name,receipt(9473,name)]));
const supportingComponents={widget:receipt(7693,'l'),spinner:receipt(603,'t'),closeIcon:receipt(7731,'_'),radio:receipt(9473,'l')};
if(!supportingComponents.widget.source.includes('this.state.showTip&&(0,o.createPortal)')||!supportingComponents.widget.source.includes('o-=r.bottom-n.bottom+10'))throw Error('Receiver widget portal changed');
if(!snippets.mE.source.includes('direction:"left"')||!snippets.mE.source.includes('uppercase:!1'))throw Error('Receiver root changed');
if(s.snippet(3249,s.binding(3249,'t'))!=='()=>-1!==navigator.userAgent.indexOf("Windows",0)')throw Error('Windows branch changed');
const labelExports=['FeU','A4y','JSN','bDV','ags','orY','DJE','cbq','q2H','JhZ','q0f','UVv'];
const labels=Object.fromEntries(labelExports.map(k=>[k,s.literal(4693,s.exported(4693,k))]));
const names={'className':'class','xmlnsXlink':'xmlns:xlink','shapeRendering':'shape-rendering','textRendering':'text-rendering'};
const esc=v=>String(v).replace(/&/g,'&amp;').replace(/"/g,'&quot;').replace(/</g,'&lt;');
function tree(n){
 if(n.type!=='CallExpression'||n.arguments[0]?.type!=='Literal'||n.arguments[1]?.type!=='ObjectExpression')throw Error('Nonliteral SVG JSX');
 const attrs={},children=[];
 for(const p of n.arguments[1].properties){const k=key(p.key);if(k==='children'){const values=p.value.type==='ArrayExpression'?p.value.elements:[p.value];children.push(...values.map(tree));}else attrs[names[k]||k]=s.literal(9473,p.value);}
 return {tag:n.arguments[0].value,attrs,children};
}
function xml(n){return `<${n.tag}${Object.entries(n.attrs).map(([k,v])=>` ${k}="${esc(v)}"`).join('')}>${n.children.map(xml).join('')}</${n.tag}>`;}
const assets=[],modes=[];
for(const [mode,name]of [[1,'rE'],[2,'sE'],[3,'TE']]){
 const node=s.binding(9473,name),original=tree(node.body),base=structuredClone(original),layers=[];
 function remove(n){n.children=n.children.filter(c=>{const anim=c.children.find(a=>a.tag==='animate');if(anim){const layer=structuredClone(c);layer.children=layer.children.filter(a=>a.tag!=='animate');layer.attrs.opacity='1';layers.push({node:layer,animation:anim.attrs});return false;}remove(c);return true;});}remove(base);
 const emit=(suffix,node)=>{const output=`assets/synapse/receiver/indicator-${mode}-${suffix}.svg`,content=xml(node)+'\n';save(output,content);assets.push({source:s.module(9473).file,source_sha256:hash(s.text(s.module(9473).file)),source_offset:node.start??s.binding(9473,name).start,output,sha256:hash(content),preparation:'Static JSX SVG serialization; SMIL separated into native opacity layers'});return output.replace('assets/','');};
 const native={mode,base:emit('base',base),layers:[]};
 for(const [index,layer]of layers.entries()){
  const svg={tag:original.tag,attrs:original.attrs,children:[{...original.children[0],children:[layer.node]}]};
  native.layers.push({asset:emit(`layer-${index}`,svg),...layer.animation});
 }
 modes.push(native);
}
const rules=parseCSS(css).filter(r=>/^(\.body-widgets|\.body-widget-tip-portal|\.widget-col|\.widget-prod|\.body-wrapper\s|\.widget \.titleRow|\.widget \.help|\.indicator-led-container|\.radio-item|\[type=radio\]|\.img-text \.external|\.img-text \.multipairing|\.mt5|\.mb5|\.spinner-razer|\.modal_backDrop__BYKQl|\.modal_modal__UO3fZ|\.HyperPollingWirelessUma_loading__4WRAI)/.test(r.selector));
const staticAssets=[['icon_close.svg','assets/synapse/mapping-close.svg'],['tooltip_questionmark.svg','assets/synapse/automation-tooltip_questionmark.svg'],['spinner.svg','assets/synapse/gr-device-spinner.svg']].map(([key,prepared])=>{
 const file=s.directory+'/'+manifest.files['static/media/'+key].slice(2),bytes=fs.readFileSync(path.join(root,file));
 if(hash(bytes)!==hash(fs.readFileSync(path.join(root,prepared))))throw Error('Receiver shared asset mismatch: '+key);
 return {path:file,sha256:hash(bytes),prepared,source:bytes.toString('utf8')};
});
const native=['crates/razer-pages/src/features/source_controls/receiver.rs','crates/razer-pages/src/features/source_controls/receiver_indicator_data.json','crates/razer-pages/src/features/source_controls.rs','crates/razer-widgets/src/source_tooltip.rs','crates/razer-widgets/src/surface.rs'].map(path=>({path,sha256:hash(read(path))}));
const evidence={product_id:179,method:'Acorn module scope, current mounted Windows root, literal SVG JSX and CSS rules; no vendor JS execution',generator_sha256:hash(fs.readFileSync(__filename)),manifest:{path:s.directory+'/asset-manifest.json',sha256:hash(read(s.directory+'/asset-manifest.json'))},html:{path:s.directory+'/index.html',sha256:hash(read(s.directory+'/index.html'))},windows_predicate:receipt(3249,'t'),components:snippets,supporting_components:supportingComponents,static_assets:staticAssets,labels,css:{path:cssFile,sha256:hash(css),rules},assets,native};
save('crates/razer-pages/src/features/source_controls/receiver_indicator_data.json',JSON.stringify(modes,null,2)+'\n');
save('docs/re/receiver-current-evidence.json',JSON.stringify(evidence,null,2)+'\n');
console.log(`Current receiver: ${rules.length} CSS rules, ${assets.length} SVG layers, Windows mounted pairing and indicator verified.`);
