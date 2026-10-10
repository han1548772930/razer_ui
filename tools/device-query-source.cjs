// Shared current-source inspection. Resolves AST bindings; never evaluates code.
const {CurrentMiddlewareSource}=require('./current-middleware-source.cjs');
const {walk,key,hash}=require('./webpack-source.cjs');
const acorn=require('acorn');
const assert=require('assert');
function inspect(plan,boot){
 const source=new CurrentMiddlewareSource(plan.product_id),receipts=[];
 for(const file of [source.mainFile,...boot.files,...plan.files])source.parse(file);
 for(const receipt of [plan.factory,plan.device_info.receipt,boot.bootstrap,plan.feature.receipt]){
  const text=source.text(receipt.path);assert.equal(hash(text),receipt.sha256);assert.equal(text.slice(receipt.offset,receipt.end),receipt.source);
 }
 const resolve=(id,node)=>{
  const seen=new Set();
  for(;;){assert(node,'Unresolved node');const token=id+':'+node.start;assert(!seen.has(token),'Circular source alias');seen.add(token);
   if(node.type==='Identifier'){node=source.binding(id,node.name);continue;}
   if(node.type==='SequenceExpression'){node=node.expressions.at(-1);continue;}
   if(node.type==='MemberExpression'&&node.object.type==='Identifier'){
    const imported=source.module(id).definitions.get(node.object.name);
    if(imported?.type==='CallExpression'&&Number.isInteger(imported.arguments[0]?.value)){id=imported.arguments[0].value;node=source.exported(id,key(node.property));continue;}
   }return{id,node};
  }
 };
 const record=(label,target)=>{const receipt={label,module:target.id,...source.receipt(target.id,target.node)};receipts.push(receipt);return receipt;};
 const chain=[];let target=resolve(plan.class_module,source.exported(plan.class_module,'default'));
 for(let depth=0;depth<8;depth++){assert(['ClassExpression','ClassDeclaration'].includes(target.node.type));chain.push(target);record('class inheritance',{id:target.id,node:target.node.superClass||target.node});if(!target.node.superClass)break;target=resolve(target.id,target.node.superClass);}
 const method=name=>{const owner=chain.find(c=>c.node.body.body.some(m=>key(m.key)===name));assert(owner,'Missing method '+name);const target={id:owner.id,node:owner.node.body.body.find(m=>key(m.key)===name)};record(name,target);return target;};
 const query=name=>{
  const methodTarget=method(name),delegates=[];
  // Ignore the async/generator wrapper, which also receives `this` but
  // contains FunctionExpression and void arguments. Actual source delegates
  // forward the method's value parameters, including the DPI setter's three.
  walk(methodTarget.node,n=>{if(n.type==='CallExpression'&&n.arguments[0]?.type==='ThisExpression'&&['Identifier','MemberExpression'].includes(n.callee.type)&&n.arguments.slice(1).every(a=>['Identifier','Literal'].includes(a.type)))delegates.push(n);});
  assert.equal(delegates.length,1,'Ambiguous query delegate '+name);
  const helper=resolve(methodTarget.id,delegates[0].callee);record(name+' parser/delegate',helper);
  const sends=[];walk(helper.node,n=>{if(n.type==='CallExpression'&&key(n.callee.property)==='sendCommand')sends.push(n);});assert.equal(sends.length,1,'Ambiguous query sendCommand '+name);
  const header=resolve(helper.id,sends[0].arguments[0]);record(name+' command',header);
  assert.equal(header.node.type,'NewExpression');assert.equal(header.node.callee.name,'Uint8Array');
  const command=source.literal(header.id,header.node.arguments[0]),parsers=[];
  walk(helper.node,n=>{if(n.type==='CallExpression'&&n.callee.type==='Identifier'&&n.arguments.length===1){const binding=source.module(helper.id).definitions.get(n.callee.name);if(binding&&/Function/.test(binding.type)){const parser=resolve(helper.id,n.callee);record(name+' result parser',parser);parsers.push(parser);}}});
  return {methodTarget,helper,header,command,parsers};
 };
 const definitionsContaining=token=>{
  const hits=[];for(const [id,scope]of source.modules)for(const node of scope.definitions.values())if(node&&/Function/.test(node.type)&&source.snippet(id,node).includes(token))hits.push({id,node});return hits;
 };
 const infoExpression=acorn.parseExpressionAt(source.text(plan.device_info.receipt.path),plan.device_info.receipt.offset,{ecmaVersion:'latest'});
 const infoNode=infoExpression.type==='SequenceExpression'?infoExpression.expressions[0]:infoExpression;
 const infoField=name=>{const field=infoNode.properties.find(p=>key(p.key)===name);return field?source.literal(plan.device_info.module,field.value):null;};
 const alternateCommands=()=>{
  const send=method('sendCommand'),headers=[];
  walk(send.node,n=>{
   if(n.type!=='MemberExpression'||!n.computed||n.property.value!==2||n.object.type!=='MemberExpression'||n.object.object.type!=='Identifier')return;
   const binding=source.module(send.id).definitions.get(n.object.object.name);
   if(binding?.type!=='CallExpression'||!Number.isInteger(binding.arguments[0]?.value))return;
   const target=resolve(send.id,n.object);if(target.node.type!=='NewExpression'||target.node.callee.name!=='Uint8Array')return;
   const command=source.literal(target.id,target.node.arguments[0]);
   if(!headers.some(h=>JSON.stringify(h.command)===JSON.stringify(command))){record('alternate transaction command '+key(n.object.property),target);headers.push({command});}
  });
  assert.equal(headers.length,2,'Alternate transaction command gate changed');return headers;
 };
 const features=()=>{
  const tree=acorn.parseExpressionAt(source.text(boot.bootstrap.path),boot.bootstrap.offset,{ecmaVersion:'latest'}),result=[];
  walk(tree,n=>{if(n.type==='CallExpression'&&key(n.callee.property)==='useFeature'&&typeof n.arguments[0]?.value==='string')result.push({name:n.arguments[0].value,value:n.arguments[1]?.value,options:n.arguments[2]});});
  return result;
 };
 return {source,resolve,record,chain,method,query,receipts,definitionsContaining,infoField,alternateCommands,features};
}
// Alpha-normalization preserves actual property names and literal values. It
// replaces namespace export spellings only after resolving their literal data.
// Imported nonliteral symbols remain named, so unknown helper changes fail closed.
function canonical(source,id,node,depth=0){
 const names=new Map(),builtins=new Set(['Uint8Array','Math','isNaN','Array','Object','Promise','console','undefined','Error']);
 const encode=(value,parent,field)=>{
  if(Array.isArray(value))return value.map(v=>encode(v));
  if(!value||typeof value!=='object')return value;
  if(value.type==='Identifier'){
   if((parent?.type==='MemberExpression'&&field==='property'&&!parent.computed)||(parent?.type==='Property'&&field==='key'&&!parent.computed)||(parent?.type==='MethodDefinition'&&field==='key'))return{type:'Identifier',name:value.name};
   if(builtins.has(value.name))return{type:'Identifier',name:value.name};
   if(!names.has(value.name))names.set(value.name,names.size);return{type:'Identifier',name:'v'+names.get(value.name)};
  }
  if(value.type==='MemberExpression'&&value.object.type==='Identifier'){
   const imported=source.module(id).definitions.get(value.object.name);
   if(imported?.type==='CallExpression'&&Number.isInteger(imported.arguments[0]?.value)){
    try{return{type:'ResolvedLiteral',value:source.literal(id,value)}}catch{}
    try{
     const targetId=imported.arguments[0].value;
     let target=source.exported(targetId,key(value.property));
     const seen=new Set();while(target.type==='Identifier'&&!seen.has(target.name)){seen.add(target.name);target=source.binding(targetId,target.name);}
     if(target.type==='NewExpression'&&target.callee.name==='Uint8Array')return{type:'ResolvedTypedArray',value:source.literal(targetId,target.arguments[0])};
     if(depth<2&&(/Function|Class/.test(target.type)||['NewExpression','ObjectExpression'].includes(target.type)))return{type:'ResolvedBinding',shape:hash(canonical(source,targetId,target,depth+1))};
    }catch{}
   }
  }
  const result={};for(const [k,v]of Object.entries(value))if(!['start','end','raw','loc'].includes(k))result[k]=encode(v,value,k);return result;
 };
 return JSON.stringify(encode(node));
}
module.exports={inspect,canonical};
