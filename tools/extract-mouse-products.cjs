// Static source extraction only. Never import/eval downloaded JavaScript.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const sha = s => crypto.createHash('sha256').update(s).digest('hex');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const inventory = JSON.parse(read('docs/re/unimplemented-products.json'));
const products = inventory.products.filter(p => p.family === 'mouse' || p.product_id === 162);
products.push({product_id:182, name:'Razer DeathAdder V3 Pro', navigation:[]});
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
  // Cobra's bundle uses Babel function exports, unlike the arrow-form scanner
  // below. Reuse the maintained general CONFIG parser's exact module receipt.
  if(product.product_id === 162) {
    const entry = JSON.parse(read('docs/re/source-product-configs.json')).products.find(p=>p.product_id===162);
    const config = entry.config, source = read(config.path), info = config.exports.DeviceInfo;
    if(!config.path.startsWith('.ref/devices/162/') || sha(source)!==config.sha256
      || info.productId!==163 || info.category!=='MOUSE' || info.deviceName!=='Razer Cobra') throw Error('Cobra identity changed');
    const parsed = acorn.parseExpressionAt(source, config.offset, {ecmaVersion:'latest'});
    const module = parsed.type==='SequenceExpression'?parsed.expressions[0]:parsed;
    if(module.end!==config.end) throw Error('Cobra CONFIG range changed');
    const groupStart = source.indexOf('3138:function');
    if(groupStart<0 || !source.includes('payload:s.X[e].group.buttonList')) throw Error('Cobra button group consumer changed');
    const groupModule = acorn.parseExpressionAt(source,groupStart+'3138:'.length,{ecmaVersion:'latest'});
    let groupNode;
    walk(groupModule,n=>{if(n.type==='VariableDeclarator' && n.id.name==='r' && n.init?.type==='ArrayExpression') groupNode=n.init;});
    const groups=decode(groupNode,new Map(),source);
    if(!Array.isArray(groups) || groups.length!==1 || groups[0].group.buttonList.length!==8) throw Error('Cobra groups changed');
    receipts.push({product_id:162,name:product.name,config,navigation:product.navigation,
      groups:groups.map(g=>g.group),group_source:{path:config.path,offset:groupNode.start,end:groupNode.end,sha256:sha(source.slice(groupNode.start,groupNode.end))}});
    continue;
  }
  const dir=`.ref/devices/${product.product_id}/static/js`;
  const files=fs.readdirSync(path.join(root,dir)).filter(p=>p.endsWith('.js'));
  const candidates=[];
  for(const file of files) {
    const source=read(`${dir}/${file}`);
    const match=/DeviceInfo:\(\)=>[A-Za-z_$][\w$]*/.exec(source);
    if(!match || !/minDPI:\d/.test(source)) continue;
    const heads=[...source.slice(0,match.index).matchAll(/(?:^|[,\{])(\d+):((?:\([^)]*\)|[A-Za-z_$][\w$]*)=>|function\()/g)];
    const head=heads.at(-1);
    if(!head) throw Error(`No module head ${product.product_id}/${file}`);
    const start=head.index+head[0].indexOf(':')+1;
    const parsed=acorn.parseExpressionAt(source,start,{ecmaVersion:'latest'});
    const node=parsed.type==='SequenceExpression'?parsed.expressions[0]:parsed;
    if(node.end<match.index) throw Error(`Wrong module ${file}`);
    const bindings=new Map(), exports=new Map();
    walk(node.body,n=>{
      if(n!==node.body && /Function|Class/.test(n.type)) return false;
      if(n.type==='VariableDeclarator'&& n.id.type==='Identifier') bindings.set(n.id.name,n.init);
      if(n.type==='CallExpression' && n.callee.type==='MemberExpression' && key(n.callee.property)==='d' && n.arguments[1]?.type==='ObjectExpression') {
        for(const p of n.arguments[1].properties) {
          const b=p.value?.body;
          if(b?.type==='Identifier') exports.set(key(p.key),b);
        }
      }
    });
    const values={};
    for(const [name,exp] of exports) {
      if(/^(DeviceInfo|DEVICE_NAME|DEVICE_SUPPORTED_MATS|DEFAULTPROFILE|BUTTON_LIST|DKMKEYS|DEFAULTMAPPINGS|POLLING|HYPER|QUICK|EFFECT|SCROLL|SMART|POWER|LOW|LIGHT|SENSOR|CALIBRATION|MATS_PRESETS|LIFTOFF|smartTracking)/.test(name)) values[name]=decode(exp,bindings,source);
    }
    if(!values.DeviceInfo || ![values.DeviceInfo.productId,values.DeviceInfo.dongleId,values.DeviceInfo.bleId].includes(product.product_id)) continue;
    candidates.push({path:`${dir}/${file}`,sha256:sha(source),module_id:Number(head[1]),offset:node.start,end:node.end,exports:values});
  }
  if(candidates.length!==1) throw Error(`${product.product_id}: ${candidates.length} config modules`);
  receipts.push({product_id:product.product_id,name:product.name,config:candidates[0],navigation:product.navigation});
}
const output={schema_version:1,source_date:'2026-10-02',extraction:'Acorn parses module expressions; only literals, unary primitives and lexical references are decoded. Unresolved expressions remain evidence, never executed.',scanner_sha256:sha(fs.readFileSync(__filename)),products:receipts};
fs.writeFileSync(path.join(root,'docs/re/mouse-product-source.json'),JSON.stringify(output,null,2)+'\n');
console.log(`Extracted ${receipts.length} current mouse CONFIG modules.`);
