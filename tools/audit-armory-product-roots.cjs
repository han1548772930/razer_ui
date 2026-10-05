// Parse each product's own Armory root as data. No reference JavaScript executes.
const fs=require('fs'),acorn=require('acorn');
const {hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const audit=JSON.parse(fs.readFileSync('docs/re/display-mode-audit.json','utf8'));
const ids=process.argv.slice(2).map(Number);
const selected=ids.length?ids:[1303,1304,1313,3893,3894,3907];
const key=n=>n?.name??n?.value;
function walk(node,fn){if(!node?.type||fn(node)===false)return;for(const v of Object.values(node))
  if(Array.isArray(v))v.forEach(n=>walk(n,fn));else if(v?.type)walk(v,fn);}
const products=[];
for(const product of audit.products.filter(p=>selected.includes(p.product_id))){
  const branch=product.modes.find(m=>m.mode==='armory'&&m.kind==='root');if(!branch)continue;
  const source=fs.readFileSync(product.source,'utf8');if(hash(source)!==product.sha256)throw Error('Source hash mismatch');
  const ast=acorn.parse(source,{ecmaVersion:'latest'}),scopes=new WeakMap(),conditionals=[],initializers=[],lazyModules=[];
  function visit(node,scope){if(!node?.type)return;
    if(['ClassDeclaration','FunctionDeclaration'].includes(node.type))scope?.defs.set(node.id.name,node);
    if(['Program','FunctionDeclaration','FunctionExpression','ArrowFunctionExpression'].includes(node.type)){
      scope={parent:scope,defs:new Map()};for(const p of node.params??[])if(p.type==='Identifier')scope.defs.set(p.name,null);
    }
    scopes.set(node,scope);
    if(product.product_id===3894&&node.type==='Property'&&key(node.key)===7855&&node.value?.type==='ArrowFunctionExpression')
      lazyModules.push({module_id:7855,offset:node.start,end:node.end,source:source.slice(node.start,node.end)});
    if(node.type==='VariableDeclarator'&&node.id.type==='Identifier')scope.defs.set(node.id.name,node.init);
    if(node.type==='VariableDeclarator'&&node.init?.type==='ObjectExpression'){
      const runtime=node.init.properties.find(p=>key(p.key)==='runtimeData')?.value;
      if(runtime?.type==='ObjectExpression'&&runtime.properties.some(p=>key(p.key)==='cpuTemp'))
        initializers.push({symbol:node.id.name,offset:node.init.start,end:node.init.end,source:source.slice(node.init.start,node.init.end)});
    }
    if(node.type==='ConditionalExpression')conditionals.push(node);
    for(const v of Object.values(node))if(Array.isArray(v))v.forEach(n=>visit(n,scope));else if(v?.type)visit(v,scope);
  }
  visit(ast,null);
  function resolve(n,seen=new Set()){
    if(!n||seen.has(n))return n;seen.add(n);
    if(n.type==='Identifier'){let scope=scopes.get(n);while(scope){if(scope.defs.has(n.name))return resolve(scope.defs.get(n.name),seen)??n;scope=scope.parent;}}
    return n;
  }
  const conditional=conditionals.find(n=>n.test.start<=branch.offset&&n.test.end>branch.offset);
  if(!conditional)throw Error(`Missing root ${product.product_id}`);
  const seen=new Set(),components=[],bindings=[];
  function trace(n,depth=0){if(!n||depth>12)return;const symbol=n.type==='Identifier'?n.name:null;
    n=resolve(n);if(!n||seen.has(n))return;seen.add(n);
    if(n.type==='CallExpression'){
      if(symbol)bindings.push({symbol,offset:n.start,end:n.end,source:source.slice(n.start,n.end)});
      for(const arg of n.arguments)if(['Identifier','ClassExpression','FunctionExpression','ArrowFunctionExpression'].includes(arg.type))trace(arg,depth+1);
      return;
    }
    let render=n;if(['ClassDeclaration','ClassExpression'].includes(n.type))render=n.body.body.find(m=>key(m.key)==='render')?.value;
    if(!render||!/Function/.test(render.type))return;
    components.push({symbol,offset:n.start,end:n.end,source:source.slice(n.start,n.end),render_source:source.slice(render.start,render.end)});
    walk(render,child=>{if(child.type==='CallExpression'&&child.callee.type==='SequenceExpression'
      &&['jsx','jsxs'].includes(key(child.callee.expressions.at(-1)?.property))){const target=child.arguments[0];if(target.type!=='Literal')trace(target,depth+1);}
      if(child.type==='Identifier'){const r=resolve(child);if(r!==child&&['Literal','TemplateLiteral','BinaryExpression'].includes(r?.type)){
        if(!bindings.some(b=>b.offset===r.start))bindings.push({symbol:child.name,offset:r.start,end:r.end,source:source.slice(r.start,r.end)});
      }}
    });
  }
  const root=conditional.consequent;trace(root.arguments?.[0]);
  const manifest=JSON.parse(fs.readFileSync(`.ref/devices/${product.product_id}/asset-manifest.json`,'utf8'));
  const imageModules=new Map();
  const modulePattern=/(?<![\w])(\d+):\([^)]*\)=>\{(?:"use strict";)?\w+\.exports=\w+\.p\+"(static\/media\/[^"\\]+)";?\}/g;
  for(const relative of [...new Set(Object.values(manifest.files))].filter(p=>p.endsWith('.js'))){
    const file=`.ref/devices/${product.product_id}/${relative.replace(/^\.\//,'')}`;
    if(!fs.existsSync(file))continue;
    const text=fs.readFileSync(file,'utf8');
    for(const m of text.matchAll(modulePattern))imageModules.set(Number(m[1]),{path:m[2],module_source:file,module_offset:m.index,module_sha256:hash(text)});
  }
  const imagePattern=new RegExp('"(\\./'+product.product_id+'_(\\d+)/img_prods/[^"\\\\]+)":\\[(\\d+),[^\\]]+\\]','g');
  const images=[...source.matchAll(imagePattern)].map(m=>{
    const media=imageModules.get(Number(m[3]));
    if(!media)throw Error(`Missing image module ${product.product_id}:${m[3]}`);
    const local=`.ref/devices/${product.product_id}/${media.path}`;
    return{request:m[1],edition_id:Number(m[2]),module_id:Number(m[3]),context_offset:m.index,...media,
      url:`https://apps.razer.com/synapse/products/${product.product_id}/ui/${media.path}`,local,available:fs.existsSync(local)};
  });
  const cssPaths=[...new Set(Object.values(manifest.files))].filter(p=>/\/static\/css\/[^/]+\.css$/.test(p));
  const css=cssPaths.map(p=>{const path=`.ref/devices/${product.product_id}/${p.replace(/^\.\//,'')}`,text=fs.readFileSync(path,'utf8');
    return{path,sha256:hash(text),rules:parseCSS(text).filter(r=>/armory|widget-prod|prod-img|dot-bg|cpu|gpu|cool|pump|gauge|monitor|system-info|body-wrapper|main-container|temperature|frequency|speed|CoolerInfo|FirmwareWarning/.test(r.selector))};});
  products.push({product_id:product.product_id,name:product.name,source:product.source,sha256:product.sha256,
    root_offset:conditional.start,root_condition:source.slice(conditional.test.start,conditional.test.end),
    root_source:source.slice(root.start,root.end),components,bindings,initializers,lazy_modules:lazyModules,images,css});
  console.log(`${product.product_id}: ${components.length} component bodies, ${bindings.length} static bindings`);
}
fs.writeFileSync('docs/re/armory-product-roots-current-evidence.json',JSON.stringify({method:'Lexically resolved Acorn AST; downloaded source never executed.',products},null,2)+'\n');
