// Fresh direct-source predicate receipts. Never evaluate vendor JavaScript.
const fs=require('fs'),crypto=require('crypto');
const products=JSON.parse(fs.readFileSync('crates/razer-pages/src/features/keyboard_products_data.json','utf8'));
const native=fs.readFileSync('crates/razer-pages/src/features/keyboard_products.rs','utf8');
const properties=JSON.parse(fs.readFileSync('crates/razer-pages/src/features/keyboard_properties_data.json','utf8'));
const gaming=JSON.parse(fs.readFileSync('crates/razer-pages/src/features/keyboard_gaming_review_data.json','utf8'));
const hash=bytes=>crypto.createHash('sha256').update(bytes).digest('hex');
const selected=process.argv.slice(2).map(Number).filter(Number.isFinite);
const rows=[];
for(const product of products.filter(product=>!selected.length||selected.includes(product.product_id))){
 const pid=product.product_id,root=`local-ui-reverse/source/official/apps.razer.com/synapse/products/${pid}/ui/static/js`;
 const file=fs.readdirSync(root).find(name=>name.startsWith('main.'));
 const path=root+'/'+file,bytes=fs.readFileSync(path),source=bytes.toString('utf8');
 const row={product_id:pid,native:{properties:properties.find(row=>row.product_id===pid)??null,gaming:gaming.find(row=>row.product_id===pid)??null,analog_layout:product.source_layout??null},source:path,sha256:hash(bytes),receipts:[]};
 const terms=['oledLowBatteryWarningDisplay','OpenKeyboardProperties','OpenGameControllerProperties','className:"widget-col','isFactoryDefaultProfile','snap-tap-widget','snap-tap-wrapper','snap-tap-container','id:"winKey"','id:"menuKey"','id:"copilotKey"','KEY_APPLICATION','DKM_D2','DKM_F6'];
 for(const term of terms){let i=-1;while((i=source.indexOf(term,i+1))>=0){row.receipts.push({term,byte_offset:Buffer.byteLength(source.slice(0,i)),source:source.slice(Math.max(0,i-700),i+1700)});}}
 // Route-owned widgets may live outside main. Retain their exact bytes too.
 for(const chunk of fs.readdirSync(root).filter(name=>name!==file&&name.endsWith('.js'))){const b=fs.readFileSync(root+'/'+chunk);if(!['snap-tap-widget','snap-tap-wrapper','snap-tap-container','OpenKeyboardProperties','id:"winKey"'].some(term=>b.includes(term)))continue;const s=b.toString('utf8');for(const term of terms){let i=-1;while((i=s.indexOf(term,i+1))>=0)row.receipts.push({term,path:root+'/'+chunk,sha256:hash(b),byte_offset:Buffer.byteLength(s.slice(0,i)),source:s.slice(Math.max(0,i-700),i+1700)});}}
 rows.push(row);
}
fs.mkdirSync('.work/keyboard-product-predicates',{recursive:true});
fs.writeFileSync('.work/keyboard-product-predicates/receipts.json',JSON.stringify({method:'Fresh direct current JS reads; offset unit UTF-8 bytes; candidates are not automatically mounted UI',native_predicates:[...native.matchAll(/[^\r\n]*(?:product_id|source_layout|source_mod_tap|source_keyboard_top_padding_percent)[^\r\n]*/g)].map(match=>({line:native.slice(0,match.index).split('\n').length,source:match[0]})),products:rows},null,2));
console.log(JSON.stringify({products:rows.length,receipts:rows.reduce((count,row)=>count+row.receipts.length,0),native_explicit_pid_conditions:[...native.matchAll(/(?:self|this)(?:\.spec)?\.product_id\s*(?:==|!=)\s*\d+/g)].map(match=>match[0])}));
