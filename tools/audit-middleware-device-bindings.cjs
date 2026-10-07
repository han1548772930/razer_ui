// Catalog-wide current middleware DeviceInfo/feature inventory. AST data only.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {CurrentMiddlewareSource}=require('./current-middleware-source.cjs');
const {walk,key,hash}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..'),read=f=>JSON.parse(fs.readFileSync(path.join(root,f),'utf8'));
const plan=read('.ref/middleware/bootstrap-requests.json'),products=[];
function data(node,text){
  if(node.type==='Literal')return node.value;
  if(node.type==='UnaryExpression'&&node.operator==='!'&&node.argument.type==='Literal')return !node.argument.value;
  if(node.type==='ArrayExpression')return node.elements.map(n=>data(n,text));
  if(node.type==='ObjectExpression')return Object.fromEntries(node.properties.map(p=>p.type==='Property'&&!p.computed?[key(p.key),data(p.value,text)]:['unresolved_spread',text.slice(p.start,p.end)]));
  return {unresolved_expression:text.slice(node.start,node.end)};
}
function decode(source,module,node){
  try{return source.literal(module,node)}catch(error){return {unresolved_expression:source.snippet(module,node)}}
}
for(const entry of plan.products){
  const row={product_id:entry.product_id,status:'unresolved',feature_calls:entry.feature_calls||[],bootstrap:entry.bootstrap||null};
  try{
    if(entry.status==='unresolved_entrypoint')throw Error(entry.reason);
    const source=new CurrentMiddlewareSource(entry.product_id);
    source.parse(source.mainFile);
    for(const file of entry.files)source.parse(file);
    const infos=[], exportedInfos=[];
    for(const[id,module]of source.modules){
      if(!module.exports.has('DeviceInfo'))continue;
      let node=source.exported(id,'DeviceInfo');const seen=new Set();
      while(node.type==='Identifier'&&!seen.has(node.name)){seen.add(node.name);node=source.binding(id,node.name);}
      if(node.type!=='ObjectExpression')continue;
      const values={};
      for(const name of ['deviceName','productId','dongleId','bleId','vendorId','claimInterface','category']){
        const property=node.properties.find(p=>p.type==='Property'&&key(p.key)===name);
        if(property)values[name]=decode(source,id,property.value);
      }
      const info={module:id,values,receipt:source.receipt(id,node)};
      exportedInfos.push(info);
      if(['productId','dongleId','bleId'].some(k=>values[k]===entry.product_id||Array.isArray(values[k])&&values[k].includes(entry.product_id)))infos.push(info);
    }
    row.module_count=source.modules.size;
    row.device_info_candidates=infos;
    row.exported_device_infos=exportedInfos;
    row.inline_identity_candidates=[];
    row.feature_bindings=[];
    for(const file of [source.mainFile,...entry.files]){
      const text=source.text(file),tree=acorn.parse(text,{ecmaVersion:'latest'});
      walk(tree,node=>{
        if(!infos.length&&node.type==='ObjectExpression'&&node.properties.some(p=>key(p.key)==='masterGuideURL')&&
          node.properties.some(p=>key(p.key)==='productId'&&Number.isInteger(p.value.value))){
          const values={};
          for(const name of ['productId','dongleId','bleId','vendorId','claimInterface','category']){
            const property=node.properties.find(p=>key(p.key)===name);
            if(property)values[name]=data(property.value,text);
          }
          row.inline_identity_candidates.push({values,receipt:{path:file,sha256:hash(text),offset:node.start,end:node.end,source:text.slice(node.start,node.end)}});
        }
        if(node.type!=='CallExpression'||node.callee.type!=='MemberExpression'||key(node.callee.property)!=='useFeature'||node.arguments[0]?.value!=='rzDeviceType')return;
        const owner=[...source.modules.values()].filter(m=>m.file===file&&m.fn.start<=node.start&&m.fn.end>=node.end).sort((a,b)=>(a.fn.end-a.fn.start)-(b.fn.end-b.fn.start))[0];
        const value=argument=>owner?decode(source,owner.id,argument):data(argument,text);
        row.feature_bindings.push({class_key:value(node.arguments[1]),options:node.arguments[2]?value(node.arguments[2]):{},receipt:{path:file,sha256:hash(text),offset:node.start,end:node.end,source:text.slice(node.start,node.end)}});
      });
    }
    const factories=[];
    for(const[id,module]of source.modules){
      for(const[name,node]of module.definitions){
        if(!node||!['ArrowFunctionExpression','FunctionExpression'].includes(node.type))continue;const text=source.snippet(id,node);
        if(text.includes('getFeatures("rzDeviceType")')&&text.includes('.connectDevice()'))factories.push({module:id,name,receipt:source.receipt(id,node)});
      }
    }
    row.factories=factories;
    row.acquisition=source.acquisition;
    row.status=infos.length===1?'device_info_identified':'device_info_unresolved';
  }catch(error){row.reason=error.message;}
  products.push(row);
  if(products.length%50===0)console.log(`Static middleware bindings ${products.length}/${plan.products.length}`);
}
const counts={};for(const product of products)counts[product.status]=(counts[product.status]||0)+1;
const output={method:'Manifest-scoped static AST inventory, no vendor execution',scope_products:products.length,counts,products};
const destination=path.join(root,'docs/re/middleware-device-bindings-current.json'),outputText=JSON.stringify(output,null,2)+'\n';
if(process.argv.includes('--check')){if(fs.readFileSync(destination,'utf8')!==outputText)throw Error('Stale middleware binding inventory');}
else fs.writeFileSync(destination,outputText);
console.log(JSON.stringify(counts));
