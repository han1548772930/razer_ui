// Resolve literal parameters from current per-product middleware. No eval.
const fs=require('fs'),acorn=require('acorn'),crypto=require('crypto');
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
function walk(n,f){if(!n||typeof n!=='object')return;if(n.type)f(n);for(const v of Object.values(n)){if(Array.isArray(v))v.forEach(x=>walk(x,f));else if(v&&typeof v==='object')walk(v,f)}}
function literal(n){
 if(n.type==='Literal')return n.value;
 if(n.type==='ObjectExpression')return Object.fromEntries(n.properties.map(p=>[p.key.name??p.key.value,literal(p.value)]));
 if(n.type==='UnaryExpression'&&n.operator==='-')return -literal(n.argument);
 throw Error('Nonliteral parameter '+n.type);
}
const products=[];
for(const pid of [2676,2684]){
 const receipt=JSON.parse(fs.readFileSync(`docs/re/gamepad-${pid}-calibration-middleware-live-source.json`,'utf8'));
 const raw=fs.readFileSync(receipt.source);if(hash(raw)!==receipt.sha256)throw Error('Stale middleware '+pid);
 const source=raw.toString('utf8'),module=receipt.modules.find(m=>m.module===69427);
 if(source.slice(module.offset,module.end)!==module.source)throw Error('Wrong module locator '+pid);
 const name=/ControllerCalibrationParams:\(\)=>([\w$]+)/.exec(module.source)?.[1];
 if(!name)throw Error('Missing params export '+pid);
 const ast=acorn.parseExpressionAt(source,module.offset,{ecmaVersion:'latest'});let candidates=[];
 walk(ast,n=>{if(n.type==='VariableDeclarator'&&n.id.name===name)candidates.push(n.init)});
 if(candidates.length!==1)throw Error('Ambiguous params '+pid);
 const expression=candidates[0],params=literal(expression);
 if(params.Version!==3)throw Error('Unsupported calibration version '+pid);
 products.push({product_id:pid,source:receipt.source,source_sha256:receipt.sha256,
   offset_unit:'UTF-16 code units',offset:expression.start,end:expression.end,
   source_expression:source.slice(expression.start,expression.end),params});
}
fs.writeFileSync('assets/data/controller-calibration-current-params.json',JSON.stringify(products,null,2)+'\n');
console.log(JSON.stringify(products.map(p=>({pid:p.product_id,trigger:p.params.Trigger}))));
