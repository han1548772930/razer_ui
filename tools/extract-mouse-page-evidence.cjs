// Parse current product page components as data; never execute downloaded code.
const fs=require('fs'),path=require('path'),crypto=require('crypto'),acorn=require('acorn');
const root=path.resolve(__dirname,'..');
const input=JSON.parse(fs.readFileSync(path.join(root,'docs/re/mouse-product-source.json'),'utf8'));
const sha=s=>crypto.createHash('sha256').update(s).digest('hex');
const walk=(n,f)=>{if(!n?.type||f(n)===false)return;for(const v of Object.values(n)){if(Array.isArray(v)){for(const x of v)if(x?.type)walk(x,f);}else if(v?.type)walk(v,f);}};
const key=n=>n?.name??n?.value;
const result=[];
for(const product of input.products) {
 const files=new Map();
 const pages=[];
 for(const nav of product.navigation) {
  if(!files.has(nav.source)) {
   const source=fs.readFileSync(path.join(root,nav.source),'utf8');
   if(sha(source)!==nav.sha256)throw Error(`Changed ${nav.source}`);
   const ast=acorn.parse(source,{ecmaVersion:'latest'});
   const scopes=[];
   function scopeWalk(n,scope) {
    if(!n?.type)return;
    if(n.type==='VariableDeclarator'&&n.id.type==='Identifier')scope?.defs.set(n.id.name,n.init);
    if(n.type==='FunctionDeclaration'||n.type==='ClassDeclaration')scope?.defs.set(n.id.name,n);
    if(/FunctionExpression|FunctionDeclaration/.test(n.type)||n.type==='Program'){
     scope={parent:scope,start:n.start,end:n.end,defs:new Map()};scopes.push(scope);
    }
    for(const v of Object.values(n)) {
     if(Array.isArray(v)){for(const x of v)if(x?.type)scopeWalk(x,scope);}
     else if(v?.type)scopeWalk(v,scope);
    }
   }
   scopeWalk(ast,null);
   files.set(nav.source,{source,scopes});
  }
  const {source,scopes}=files.get(nav.source);
  let scope=scopes.filter(s=>s.start<nav.owner_offset&&s.end>nav.offset).sort((a,b)=>(a.end-a.start)-(b.end-b.start))[0];
  const lookup=name=>{let s=scope;while(s){if(s.defs.has(name))return s.defs.get(name);s=s.parent;}return null;};
  for(const item of nav.items) {
   if(!item.component)continue;
   const name=/\.jsx(?:s)?\)\(([^,]+)/.exec(item.component)?.[1];
   const components=[],seen=new Set();
   function trace(name) {
    if(!name||seen.has(name))return;seen.add(name);
    const n=lookup(name);if(!n)return;
    if(n.type==='CallExpression')for(const a of n.arguments)if(a.type==='Identifier')trace(a.name);
    if(n.type==='Identifier')trace(n.name);
    const jsx=[];
    walk(n,c=>{
     if(c.type==='CallExpression'&&c.callee.type==='SequenceExpression'&&['jsx','jsxs'].includes(key(c.callee.expressions.at(-1)?.property))){
      const component=c.arguments[0];
      const props=c.arguments[1];
      if(component?.type==='Identifier')trace(component.name);
      const literalProps={};
      for(const p of props?.properties??[])if(p.type==='Property'&&['Literal','UnaryExpression','ArrayExpression'].includes(p.value.type))literalProps[key(p.key)]=source.slice(p.value.start,p.value.end);
      jsx.push({component:source.slice(component.start,component.end),offset:c.start,props:literalProps});
     }
    });
    components.push({symbol:name,offset:n.start,end:n.end,source:source.slice(n.start,n.end),jsx});
   }
   trace(name);
   pages.push({key:item.name?.value,path:nav.source,sha256:nav.sha256,nav_offset:item.offset,component:item.component,components});
  }
 }
 result.push({product_id:product.product_id,pages});
}
fs.writeFileSync(path.join(root,'docs/re/mouse-page-source.json'),JSON.stringify({schema_version:1,scanner_sha256:sha(fs.readFileSync(__filename)),products:result},null,2)+'\n');
console.log(`Traced mounted mouse pages for ${result.length} products.`);
