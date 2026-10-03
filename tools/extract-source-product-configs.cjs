// Static source extraction only. Never import/eval downloaded JavaScript.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const sha = s => crypto.createHash('sha256').update(s).digest('hex');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const inventory = JSON.parse(read('docs/re/unimplemented-products.json'));
const products = inventory.products.filter(p => ['system','camera','gamepad','iot_hue','other'].includes(p.family));
products.sort((a,b) => a.product_id-b.product_id);
const key = n => n?.name ?? n?.value;
function walk(n, visit) {
  if (!n?.type || visit(n) === false) return;
  for (const v of Object.values(n)) {
    if (Array.isArray(v)) { for (const x of v) if (x?.type) walk(x, visit); }
    else if (v?.type) walk(v, visit);
  }
}
function decode(n, bindings, source, seen = new Set()) {
  if (!n) return null;
  if (n.type === 'Literal') return n.value;
  if (n.type === 'UnaryExpression') {
    const v=decode(n.argument,bindings,source,seen);
    if (v === null || typeof v === 'object') return {expression:source.slice(n.start,n.end)};
    if (n.operator==='!') return !v;
    if (n.operator==='-') return -v;
    if (n.operator==='+') return +v;
  }
  if(n.type==='Identifier' && bindings.has(n.name) && !seen.has(n.name)) {
    return decode(bindings.get(n.name),bindings,source,new Set([...seen,n.name]));
  }
  if(n.type==='ArrayExpression') return n.elements.map(x=>decode(x,bindings,source,seen));
  if(n.type==='ObjectExpression') {
    const v={};
    for(const p of n.properties) {
      if(p.type==='Property' && !p.computed) v[key(p.key)]=decode(p.value,bindings,source,seen);
      else v.$spread=source.slice(p.start,p.end);
    }
    return v;
  }
  return {expression:source.slice(n.start,n.end)};
}
const receipts=[];
for(const product of products) {
  const dir=`.ref/devices/${product.product_id}/static/js`;
  const files=fs.readdirSync(path.join(root,dir)).filter(p=>p.endsWith('.js'));
  const candidates=[];
  for(const file of files) {
    const source=read(`${dir}/${file}`);
    const match=/(?<![\w$])DeviceInfo:(?:\(\)=>|function\(\)\{return\s+)[A-Za-z_$][\w$]*/.exec(source);
    if(!match) continue;
    const heads=[...source.slice(0,match.index).matchAll(/(?:^|[,\{])(\d+):((?:\([^)]*\)|[A-Za-z_$][\w$]*)=>|function\()/g)];
    const head=heads.at(-1);
    let node;
    if(!head) {
      const tree=acorn.parse(source,{ecmaVersion:'latest'}), scopes=[];
      walk(tree,n=>{ if(/Function/.test(n.type)&&n.start<=match.index&&n.end>match.index)scopes.push(n); });
      node=scopes.sort((a,b)=>(a.end-a.start)-(b.end-b.start))[0];
      if(!node)throw Error(`No enclosing scope ${file}`);
    } else {
      const start=head.index+head[0].indexOf(':')+1;
      const parsed=acorn.parseExpressionAt(source,start,{ecmaVersion:'latest'});
      node=parsed.type==='SequenceExpression'?parsed.expressions[0]:parsed;
    }
    if(node.end<match.index) throw Error(`Wrong module ${file}`);
    const bindings=new Map(), exports=new Map();
    walk(node.body,n=>{
      if(n!==node.body && /Function|Class/.test(n.type)) return false;
      if(n.type==='VariableDeclarator'&& n.id.type==='Identifier') bindings.set(n.id.name,n.init);
      if(n.type==='CallExpression' && n.callee.type==='MemberExpression' && key(n.callee.property)==='d' && n.arguments[1]?.type==='ObjectExpression') {
        for(const p of n.arguments[1].properties) {
          const body=p.value?.body;
          const b=body?.type==='BlockStatement'&&body.body.length===1&&body.body[0].type==='ReturnStatement'?body.body[0].argument:body;
          if(b?.type==='Identifier') exports.set(key(p.key),b);
        }
      }
    });
    const values={};
    for(const [name,exp] of exports) {
      if(/^(DeviceInfo|DEVICE_NAME|DEVICE_SUPPORTED|DEFAULT|BUTTON|DKM|POLLING|HYPER|QUICK|EFFECT|SCROLL|SMART|POWER|LOW|LIGHT|SENSOR|CALIBRATION|CAMERA|TRIGGER|THUMBSTICK|FAN|GPU|PERFORMANCE|MATS|LIFTOFF|smartTracking)/.test(name)) values[name]=decode(exp,bindings,source);
    }
    if(!values.DeviceInfo) continue;
    candidates.push({path:`${dir}/${file}`,sha256:sha(source),module_id:head?Number(head[1]):null,offset:node.start,end:node.end,exports:values});
  }
  const matching=candidates.filter(c=>[c.exports.DeviceInfo.productId,c.exports.DeviceInfo.dongleId,c.exports.DeviceInfo.bleId].includes(product.product_id));
  if(matching.length===1)candidates.splice(0,candidates.length,...matching);
  if(candidates.length!==1) { receipts.push({product_id:product.product_id,name:product.name,family:product.family,config:null,navigation:product.navigation,candidate_count:candidates.length}); continue; }
  receipts.push({product_id:product.product_id,name:product.name,family:product.family,config:candidates[0],navigation:product.navigation});
}
const output={schema_version:1,source_date:'2026-10-02',extraction:'Acorn parses module expressions; only literals, unary primitives and lexical references are decoded. Unresolved expressions remain evidence, never executed.',scanner_sha256:sha(fs.readFileSync(__filename)),products:receipts};
fs.writeFileSync(path.join(root,'docs/re/source-product-configs.json'),JSON.stringify(output,null,2)+'\n');
console.log(`Extracted ${receipts.length} current product CONFIG records.`);
