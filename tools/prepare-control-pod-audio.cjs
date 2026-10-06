// Static current 1382 MapAudio extraction. Never executes vendor JavaScript.
const fs=require('fs'),path=require('path');
const {Source,hash,walk}=require('./webpack-source.cjs');
const acorn=require('acorn');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),directory='.ref/devices/1382';
const manifest=JSON.parse(fs.readFileSync(path.join(root,directory,'asset-manifest.json'),'utf8'));
const s=Object.assign(Object.create(Source.prototype),{directory,files:[...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.js')).map(f=>directory+'/'+f.replace(/^\.\//,'')),modules:new Map(),texts:new Map(),parsed:new Set()});
s.literal=function(id,node,seen){
 if(node.type==='CallExpression'&&node.callee.type==='MemberExpression'&&node.callee.object.type==='Literal'&&node.callee.object.value===''&&node.callee.property.name==='concat')return node.arguments.map(n=>this.literal(id,n,seen)).join('');
 return Source.prototype.literal.call(this,id,node,seen);
};
const receipts=[],data={};
for(const [field,key] of [['modes','hW'],['playback','Qn'],['equalizer','Hs'],['presets','ft']]){
 data[field]=s.literal(9267,s.exported(9267,key));
 receipts.push({symbol:key,...s.receipt(9267,s.binding(9267,s.exported(9267,key).name))});
}
data.inputs=s.literal(651,s.exported(651,'DEFAULTMAPPINGS'));
receipts.push({symbol:'DEFAULTMAPPINGS',...s.receipt(651,s.binding(651,s.exported(651,'DEFAULTMAPPINGS').name))});
data.labels={};
for(const key of ['x3E','fYN','diY','RFv','uTW','f_P','Ep9','Lvx','ENf','set','W0P','bOp']){
 data.labels[key]=s.literal(4693,s.exported(4693,key));
 receipts.push({symbol:key,...s.receipt(4693,s.binding(4693,s.exported(4693,key).name))});
}
for(const id of [8191,9412,7734,2321])receipts.push({symbol:'module:'+id,...s.receipt(id,s.module(id).fn)});
const parentFile=directory+'/static/js/main.275d3b94.js',parentRaw=s.text(parentFile);
const parentAst=acorn.parse(parentRaw,{ecmaVersion:'latest'});
const parentMethods=new Set(['displaySaveAlert','dismissSave','saveChanges','dontSave','nextAction','dontSaveAction','renderSaveAlert']);
walk(parentAst,node=>{
 if(node.start<4658000||node.start>4661000||node.type!=='AssignmentExpression'||node.left?.property?.name!=='clickBtn')return;
 receipts.push({symbol:'control-pod:clickBtn',path:parentFile,sha256:hash(parentRaw),offset:node.start,end:node.end,source:parentRaw.slice(node.start,node.end)});
});
if(receipts.filter(r=>r.symbol==='control-pod:clickBtn').length!==1)throw Error('Missing Control Pod click handler');
const navigationMethods=new Set(['changeView','navigateBack','navigateForward']);
walk(parentAst,node=>{
 if(node.start<4694000||node.start>4697000||node.type!=='AssignmentExpression'||!navigationMethods.has(node.left?.property?.name))return;
 receipts.push({symbol:'product-navigation:'+node.left.property.name,path:parentFile,sha256:hash(parentRaw),offset:node.start,end:node.end,source:parentRaw.slice(node.start,node.end)});
});
for(const method of navigationMethods)if(receipts.filter(r=>r.symbol==='product-navigation:'+method).length!==1)throw Error('Missing navigation method '+method);
walk(parentAst,node=>{
 if(node.start<4682000||node.start>4686000||node.type!=='AssignmentExpression'||!parentMethods.has(node.left?.property?.name))return;
 receipts.push({symbol:'mapping-root:'+node.left.property.name,path:parentFile,sha256:hash(parentRaw),offset:node.start,end:node.end,source:parentRaw.slice(node.start,node.end)});
});
for(const method of parentMethods)if(receipts.filter(r=>r.symbol==='mapping-root:'+method).length!==1)throw Error('Missing mapping root method '+method);
receipts.push({symbol:'mapping-parent:913',...s.receipt(913,s.module(913).fn)});
for(const key of ['rq','Ts'])receipts.push({symbol:key,...s.receipt(9267,s.binding(9267,s.exported(9267,key).name))});
const css=[];
for(const relative of [...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css'))){
 const file=directory+'/'+relative.replace(/^\.\//,''),text=fs.readFileSync(path.join(root,file),'utf8');
 const rules=parseCSS(text).filter(r=>/radio|mapping-panel|mapping.*button|keymapping|key-config|dropdown|save-alert|backdrop|thx-btn|keymap-action/.test(r.selector));
 if(rules.length)css.push({path:file,sha256:hash(text),rules});
}
for(const locale of ['en.json','zh-CN.json']){
 const strings=JSON.parse(fs.readFileSync(path.join(root,'locales',locale),'utf8'));
 for(const key of [...Object.values(data.labels),...['modes','playback','equalizer','presets'].flatMap(f=>data[f].map(r=>r.content))])if(typeof strings[key]!=='string')throw Error('Missing label '+key);
}
const check=process.argv.includes('--check');
const closeSource=directory+'/'+manifest.files['static/media/icon_close.svg'].replace(/^\.\//,''),closeOutput='assets/synapse/mapping-close.svg';
const closeBytes=fs.readFileSync(path.join(root,closeSource));
if(!closeBytes.equals(fs.readFileSync(path.join(root,closeOutput))))throw Error('1382 close asset differs from shared source');
const assets=[{source:closeSource,output:closeOutput,sha256:hash(closeBytes)}];
function output(file,text){const target=path.join(root,file);if(check){if(fs.readFileSync(target,'utf8')!==text)throw Error('Stale '+file);}else fs.writeFileSync(target,text);}
output('src/features/control_pod_audio_data.json',JSON.stringify(data,null,2)+'\n');
output('docs/re/control-pod-audio-current-evidence.json',JSON.stringify({product_id:1382,method:'Static webpack parsing; no vendor execution',receipts,css,assets},null,2)+'\n');
console.log(`Control Pod audio: ${receipts.length} source receipts; ${css.length} CSS files`);
