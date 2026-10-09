// Current Profiles application transfer UI and file decoder, parsed as data only.
const fs=require('node:fs'),path=require('node:path');
const {Source,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),s=new Source('synapse/profiles');
const check=process.argv.includes('--check'),read=p=>fs.readFileSync(path.join(root,p),'utf8');
function output(p,bytes){const file=path.join(root,p);if(check){if(!fs.readFileSync(file).equals(Buffer.from(bytes)))throw Error('Stale '+p);}else fs.writeFileSync(file,bytes);}
const names=['Ua','Ia','Da','Ea','fa','ra','la','Aa','Na','ya','ja','oa','aa','ca','_a'];
const receipts=names.map(name=>({module:43,name,...s.receipt(43,s.binding(43,name))}));
for(const [id,names] of [[1867,['D','u','_']],[7207,['N','U','B','R','F','C','y']]])
 for(const name of names)receipts.push({module:id,name,...s.receipt(id,s.binding(id,name))});
for(const id of [5371,2937,8193])receipts.push({module:id,name:'module',...s.receipt(id,s.module(id).fn)});
const fact=(condition,why)=>{if(!condition)throw Error(why);};
const source=name=>receipts.find(r=>r.module===43&&r.name===name).source;
for(const term of ['isProfileSelected:!0','af06d371-861f-4b98-8d78-bfaef5cfdebf','selectedMacroData:[]'])
 fact((source('Ia')+source('Ua')).includes(term),'Changed transfer contract '+term);
fact(source('Ea').includes('decode(e.payload)')&&source('Ea').includes('e.hash!==i'),'Changed profile envelope');
fact(receipts.find(r=>r.module===1867&&r.name==='D').source.includes('delete e.hash,delete e.gamemode'),'Changed hash exclusions');
const manifest=JSON.parse(read(s.directory+'/asset-manifest.json'));
const css=[];
for(const relative of [...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css'))){
 const file=s.directory+'/'+relative.replace(/^\.\//,'');
 const rules=parseCSS(read(file)).filter(r=>/ImportExportModal_|^\.cloud-|^\.slide-|^\.import-profile-btn-group|^\.thx-btn|\.warning:before/.test(r.selector));
 if(rules.length)css.push({path:file,sha256:hash(read(file)),rules});
}
const assets=[];
for(const [key,name] of [['icon_close.svg','close'],['icon_folder_grey.svg','folder'],['icon_config_macro-1.svg','macro'],['icon_cone.svg','cone'],['warning.svg','warning']]){
 const relative=manifest.files['static/media/'+key];fact(relative,'Missing asset '+key);
 const file=s.directory+'/'+relative.replace(/^\.\//,''),out='assets/synapse/profiles-transfer-'+name+'.svg',bytes=fs.readFileSync(path.join(root,file));
 output(out,bytes);assets.push({source:file,output:out,sha256:hash(bytes)});
}
output('assets/synapse/profiles-transfer-embedded.rs','&[\n'+assets.map(a=>'    ("synapse/'+path.basename(a.output)+'", include_bytes!("'+path.basename(a.output)+'")),').join('\n')+'\n]\n');
const labels={};for(const key of ['ov8','s2B','GCq','M88','mYs','mcz','c2H','$w6','QN3','bOp','yE7','l1q','BXZ','u4O','v3x'])labels[key]=s.literal(4693,s.exported(4693,key));
for(const locale of ['en','zh-CN']){const values=JSON.parse(read('locales/'+locale+'.json'));for(const key of Object.values(labels))fact(typeof values[key]==='string','Missing '+locale+'/'+key);}
const targets={},targetSources=[];
const mw=JSON.parse(read('docs/re/middleware-device-bindings-current.json'));
const cache=new Map();
for(const product of mw.products){
 if(product.status!=='device_info_identified'||product.device_info_candidates.length!==1)continue;
 const candidate=product.device_info_candidates[0],receipt=candidate.receipt;
 if(!cache.has(receipt.path))cache.set(receipt.path,read(receipt.path));
 const text=cache.get(receipt.path);fact(hash(text)===receipt.sha256&&text.slice(receipt.offset,receipt.end)===receipt.source,'Changed current DeviceInfo '+product.product_id);
 targets[product.product_id]=candidate.values;targetSources.push({product_id:product.product_id,...receipt});
}
output('crates/razer-app-pages/src/profiles_page/transfer_data.json',JSON.stringify({labels,targets},null,2)+'\n');
const native=['crates/razer-app-pages/src/profiles_page/transfer.rs','crates/razer-app-pages/src/profiles_page/transfer_codec.rs','crates/razer-app-pages/src/profiles_page/transfer_view.rs','crates/razer-app-pages/src/profiles_page/devices.rs'];
output('docs/re/profiles-transfer-current-evidence.json',JSON.stringify({method:'Current source AST/CSS/asset inspection only; no vendor JavaScript execution',receipts,css,assets,targetSources,
 native:native.filter(p=>fs.existsSync(path.join(root,p))).map(path=>({path,sha256:hash(read(path))})),
 boundaries:['UI selections and decoded import rows are local transfer intents; no device command is sent.','Native draft profiles are not encoded as vendor profiles. Full product conversion/write-out remains separate work.','Cloud is the current source in-development branch; no fabricated cloud data.']},null,2)+'\n');
console.log(`Profiles transfer: ${receipts.length} AST, ${css.reduce((n,s)=>n+s.rules.length,0)} CSS, ${assets.length} assets.`);
