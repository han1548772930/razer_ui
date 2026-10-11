// Statically parse original device and transport modules. Never evaluate them.
const fs=require('fs'),acorn=require('acorn'),crypto=require('crypto');
const hash=s=>crypto.createHash('sha256').update(s).digest('hex');
function walk(n,f){if(!n?.type)return;f(n);for(const v of Object.values(n)){if(Array.isArray(v))for(const c of v)walk(c,f);else if(v?.type)walk(v,f);}}
for(const pid of [2676,2684]){
 const mw=JSON.parse(fs.readFileSync(`docs/re/gamepad-${pid}-calibration-middleware-live-source.json`,'utf8'));
 const acq=JSON.parse(fs.readFileSync(`docs/re/gamepad-${pid}-calibration-device-chunks-current-acquisition.json`,'utf8'));
 const modules=[];
 let transaction_receipt;
 for(const source of [mw.source,...acq.receipts.map(r=>r.path)]){
  const bytes=fs.readFileSync(source),raw=bytes.toString('utf8'),tree=acorn.parse(raw,{ecmaVersion:'latest'});
  const ids=source===mw.source?new Set([87969,68560,14397,76912,88382,9077,41863]):null;
  walk(tree,node=>{
   if(node.type!=='Property'||typeof node.key.value!=='number'||!['ArrowFunctionExpression','FunctionExpression'].includes(node.value.type)||ids&&!ids.has(node.key.value))return;
   const methods=[],declarations=[],imports=[],exports=[];
   walk(node.value,item=>{
    if(item.type==='MethodDefinition'){
     methods.push({name:item.key.name??item.key.value,offset:item.start,end:item.end,source:raw.slice(item.start,item.end)});
     if(node.key.value===87969&&item.key.name==='_getTransactionId'){
      const returned=item.value.body.body[0]?.argument;
      if(returned?.type!=='SequenceExpression'||returned.expressions.at(-1)?.type!=='UpdateExpression'||returned.expressions.at(-1).operator!=='++'||returned.expressions.at(-1).prefix!==false)throw Error('Transaction no longer uses post-increment');
      transaction_receipt={source,source_sha256:hash(bytes),offset:item.start,end:item.end,source_expression:raw.slice(item.start,item.end),initial_counter:0,returned_counter_sequence:'0..30 repeatedly',return_uses_post_increment:true};
     }
    }
    if(item.type==='VariableDeclarator'){
     declarations.push({name:item.id.name,offset:item.start,end:item.end,source:raw.slice(item.start,item.end)});
     if(item.init?.type==='CallExpression'&&typeof item.init.arguments[0]?.value==='number')imports.push({symbol:item.id.name,module:item.init.arguments[0].value});
    }
    if(item.type==='CallExpression'&&item.callee?.property?.name==='d'&&item.arguments[1]?.type==='ObjectExpression')exports.push(raw.slice(item.arguments[1].start,item.arguments[1].end));
   });
   modules.push({module:node.key.value,source,source_sha256:hash(bytes),offset:node.value.start,end:node.value.end,sha256:hash(raw.slice(node.value.start,node.value.end)),imports,exports,methods,declarations,module_source:raw.slice(node.value.start,node.value.end)});
  });
 }
 if(!modules.some(m=>m.module===98773)||!modules.some(m=>m.module===78786)||!modules.some(m=>m.module===87969))throw Error('Missing device chain');
 if(!transaction_receipt)throw Error('Missing post-increment transaction receipt');
 fs.writeFileSync(`docs/re/gamepad-${pid}-calibration-device-live-source.json`,JSON.stringify({product_id:pid,parser:'Acorn static syntax only',offset_units:'UTF-16 code units',transaction_receipt,modules},null,2)+'\n');
 console.log(JSON.stringify({product_id:pid,modules:modules.map(m=>({module:m.module,source:m.source,offset:m.offset,end:m.end}))}));
}
