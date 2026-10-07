// Identify the actual entrypoint import feeding product feature setup; AST only.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {CurrentMiddlewareSource}=require('./current-middleware-source.cjs');
const {walk,key,hash}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..');
const inventory=JSON.parse(fs.readFileSync(path.join(root,'.ref/middleware/CATALOG-ACQUISITION.json'),'utf8'));
const requests=[],products=[];
for(const product of inventory.products){
  try{
    const source=new CurrentMiddlewareSource(product),text=source.text(source.mainFile),tree=acorn.parse(text,{ecmaVersion:'latest'});source.mainTree=tree;
    const featureCalls=[],chains=[],awaitedImports=[];
    walk(tree,n=>{
      if(n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&key(n.callee.property)==='useFeature'&&n.arguments[0]?.value==='rzDeviceType')featureCalls.push(n);
      if(n.type==='VariableDeclarator'&&n.id.type==='Identifier'&&n.init?.type==='MemberExpression'&&key(n.init.property)==='default'&&
        ['YieldExpression','AwaitExpression'].includes(n.init.object.type)&&n.init.object.argument?.type==='CallExpression')awaitedImports.push(n.init.object.argument);
      if(n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&key(n.callee.property)==='then'&&['FunctionExpression','ArrowFunctionExpression'].includes(n.arguments[0]?.type)){
        const callback=text.slice(n.arguments[0].start,n.arguments[0].end);
        const parameter=n.arguments[0].params[0];
        const defaultBinding=parameter?.type==='ObjectPattern'?parameter.properties.find(p=>key(p.key)==='default')?.value?.name:null;
        let invokesDefault=false;
        if(defaultBinding)walk(n.arguments[0].body,child=>{if(child.type==='CallExpression'&&child.callee.name===defaultBinding&&child.arguments.length===0)invokesDefault=true;});
        if(callback.includes('useFeature(')||callback.includes('injectToDependencyList(')||invokesDefault)chains.push(n);
      }
    });
    const chain=chains.sort((a,b)=>b.start-a.start)[0]||awaitedImports.sort((a,b)=>b.start-a.start)[0];
    if(!chain){
      if(featureCalls.length===0)throw Error('No product setup or protocol feature');
      products.push({product_id:product,status:'bundled_entrypoint',chunks:[],files:[],feature_calls:featureCalls.map(n=>({path:source.mainFile,sha256:hash(text),offset:n.start,end:n.end,source:text.slice(n.start,n.end)})),acquisition:source.acquisition});
      continue;
    }
    const chunks=new Set();
    walk(chain,n=>{if(n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&key(n.callee.property)==='e'&&Number.isInteger(n.arguments[0]?.value))chunks.add(n.arguments[0].value);});
    const files=[...chunks].map(id=>source.chunkFile(id));
    for(const file of files)requests.push({product_id:product,file:path.basename(file)});
    const receipt=node=>({path:source.mainFile,sha256:hash(text),offset:node.start,end:node.end,source:text.slice(node.start,node.end)});
    products.push({product_id:product,status:'bootstrap_identified',chunks:[...chunks],files,feature_calls:featureCalls.map(receipt),bootstrap:receipt(chain),acquisition:source.acquisition});
  }catch(error){products.push({product_id:product,status:'unresolved_entrypoint',reason:error.message});}
}
const output={method:'Static Acorn AST of current main source; no vendor execution',requests,products};
fs.writeFileSync(path.join(root,'.ref/middleware/bootstrap-requests.json'),JSON.stringify(output,null,2)+'\n');
console.log(JSON.stringify({products:products.length,requests:requests.length,bundled:products.filter(p=>p.status==='bundled_entrypoint').length,unresolved:products.filter(p=>p.status==='unresolved_entrypoint').map(p=>({product_id:p.product_id,reason:p.reason}))},null,2));
