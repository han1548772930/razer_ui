// Static parsing and inert SVG preparation only; never evaluates vendor JS.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {Source,walk,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),directory='.ref/devices/2636';
const manifest=JSON.parse(fs.readFileSync(path.join(root,directory,'asset-manifest.json'),'utf8'));
const s=Object.assign(Object.create(Source.prototype),{directory,files:[...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.js')).map(f=>directory+'/'+f.replace(/^\.\//,'')),modules:new Map(),texts:new Map(),parsed:new Set()});
const file=directory+'/static/js/main.82d8a835.js',raw=s.text(file);
if(hash(raw)!=='a5b49ece13b13202b65e97d565b77ca61d22c8c6e14f9ec560dcee68d1155f9a')throw Error('2636 source changed; re-audit');
const receipts=[];
const methods=new Set(['zeroDeadzoneWarning','lowDeadzoneWarning','updateLowDeadzone','handleThumbstickChange','onRecalibrate','onContinue','onCloseDialog','onToggle','deadzoneDecs']);
walk(acorn.parse(raw,{ecmaVersion:'latest'}),n=>{
 if(n.start<6668000||n.start>6675000||n.type!=='AssignmentExpression'||!methods.has(n.left?.property?.name))return;
 receipts.push({symbol:n.left.property.name,path:file,offset:n.start,end:n.end,source:raw.slice(n.start,n.end)});
});
for(const method of methods)if(receipts.filter(r=>r.symbol===method).length!==1)throw Error('Missing method '+method);
for(const id of [99661,50151,27875])receipts.push({symbol:'module:'+id,...s.receipt(id,s.module(id).fn)});
const labels={};
for(const key of ['Ygv','I22','kTy','cJK','_sP','AGU','INJ','YGF','fDp','tPc']){
 labels[key]=s.literal(54693,s.exported(54693,key));
 receipts.push({symbol:'label:'+key,...s.receipt(54693,s.binding(54693,s.exported(54693,key).name))});
}
const css=[];
for(const relative of [...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css'))){
 const file=directory+'/'+relative.replace(/^\.\//,''),text=fs.readFileSync(path.join(root,file),'utf8');
 const rules=parseCSS(text).filter(r=>/alert-backdrop|deadzone-(info|header|title|content)|sensitivity-container.*arrow|customize-setting-button|box-sizing|body-wrapper/.test(r.selector));
 if(rules.length)css.push({path:file,sha256:hash(text),rules});
}
const assets=[];const check=process.argv.includes('--check');
function output(file,data){const target=path.join(root,file);if(check){if(!fs.readFileSync(target).equals(Buffer.from(data)))throw Error('Stale '+file);}else fs.writeFileSync(target,data);}
for(const [name,dest] of [['info_solid.svg','gamepad-2636-info.svg'],['icon_arrow_left_thin.svg','history-back.svg'],['icon_close.svg','mapping-close.svg']]){
 const source=directory+'/'+manifest.files['static/media/'+name].replace(/^\.\//,''),bytes=fs.readFileSync(path.join(root,source)),out='assets/synapse/'+dest;
 if(dest.startsWith('gamepad-'))output(out,bytes);else if(!bytes.equals(fs.readFileSync(path.join(root,out))))throw Error('Shared asset differs '+dest);
 assets.push({source,output:out,sha256:hash(bytes)});
}
const icon=s.snippet(27875,s.module(27875).fn);
const d=/d:"([^"]+)"/.exec(icon)?.[1];if(!d||!icon.includes('.st0{fill:#fd8611;}'))throw Error('Warning SVG changed');
const svg=`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 20 20"><path fill="#fd8611" d="${d}"/></svg>\n`;
output('assets/synapse/gamepad-2636-warning.svg',svg);
assets.push({source:file,module:27875,output:'assets/synapse/gamepad-2636-warning.svg',sha256:hash(svg)});
output('assets/synapse/gamepad-2636-embedded.rs','&[\n'+['info','warning'].map(n=>`    ("synapse/gamepad-2636-${n}.svg", include_bytes!("gamepad-2636-${n}.svg") as &[u8]),`).join('\n')+'\n]\n');
output('docs/re/gamepad-2636-dialog-current-evidence.json',JSON.stringify({product_id:2636,source:{path:file,sha256:hash(raw)},receipts,labels,css,assets},null,2)+'\n');
console.log(`2636 dialog: ${receipts.length} AST receipts, ${css.length} CSS files, ${assets.length} asset receipts`);
