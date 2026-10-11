// Parse current official middleware as syntax data. Never load vendor modules.
const fs = require('fs'), acorn = require('acorn'), crypto = require('crypto');
const sha = v => crypto.createHash('sha256').update(v).digest('hex');
function walk(n, visit) {
  if (!n?.type || visit(n) === false) return;
  for (const v of Object.values(n)) {
    if (Array.isArray(v)) v.forEach(c => walk(c, visit));
    else if (v?.type) walk(v, visit);
  }
}
// Deliberately limited AST decoder; no eval, VM or vendor function execution.
function data(n, env = {}) {
  if (n.type === 'Literal') return n.value;
  if (n.type === 'Identifier' && Object.hasOwn(env, n.name)) return env[n.name];
  if (n.type === 'ArrayExpression') return n.elements.map(x => data(x, env));
  if (n.type === 'ObjectExpression') return Object.fromEntries(n.properties.map(p => [p.key.name ?? p.key.value, data(p.value, env)]));
  if (n.type === 'UnaryExpression' && n.operator === '!') return !data(n.argument, env);
  if (n.type === 'UnaryExpression' && n.operator === '-') return -data(n.argument, env);
  if (n.type === 'CallExpression' && n.callee.name === 'parseInt') return Number.parseInt(data(n.arguments[0],env), data(n.arguments[1],env));
  throw Error(`Unsupported literal AST ${n.type}`);
}
const products = [], allData = {};
for (const pid of [190,678,679,688]) {
  const dir = `local-ui-reverse/source/official/apps.razer.com/synapse/products/${pid}/mw`;
  const modules = new Map(), files = new Map();
  for (const name of fs.readdirSync(dir).filter(n => n.endsWith('.js'))) {
    const path = dir + '/' + name, bytes = fs.readFileSync(path), raw = bytes.toString('utf8');
    const tree = acorn.parse(raw, {ecmaVersion:'latest'});
    files.set(path,{source:path,sha256:sha(bytes),bytes:bytes.length});
    walk(tree, n => {
      if (n.type === 'Property' && Number.isInteger(n.key.value) && ['ArrowFunctionExpression','FunctionExpression'].includes(n.value.type)) {
        if (modules.has(n.key.value)) throw Error(`Ambiguous module ${pid}/${n.key.value}`);
        modules.set(n.key.value, {id:n.key.value,path,raw,node:n});
        return false;
      }
    });
  }
  const factoryId = pid === 190 ? 71616 : 69183, reexportId = pid === 190 ? 97518 : 32226;
  const factory = modules.get(factoryId), reexport = modules.get(reexportId);
  if (!factory || !reexport || !reexport.raw.slice(reexport.node.start,reexport.node.end).includes(`(${factoryId})`)) throw Error('Active factory not independently proved');
  const dependencies = new Map();
  walk(factory.node.value, n => {
    if (n.type === 'VariableDeclarator' && n.init?.type === 'CallExpression' && Number.isInteger(n.init.arguments[0]?.value)) dependencies.set(n.id.name,n.init.arguments[0].value);
  });
  const callerId = pid === 190 ? 25947 : 42124;
  const caller = modules.get(callerId);
  if (!caller || !caller.raw.slice(caller.node.start,caller.node.end).includes(`(${reexportId})`)) throw Error('Actual task maker import not proved');
  const selected = new Set([callerId,factoryId,reexportId,...dependencies.values()]);
  let keyModule, hashModule, eventModule, constantsModule;
  for (const id of dependencies.values()) {
    const m = modules.get(id); if (!m) throw Error(`Missing dependency ${pid}/${id}`);
    const text=m.raw.slice(m.node.start,m.node.end);
    if (text.includes('t.KEYS=')) keyModule=m;
    if (text.includes('t.getHash=')) hashModule=m;
    if (text.includes('t.constants_exports=')) constantsModule=m;
    if (text.includes('TurboEventsFactory_default=')) {
      walk(m.node.value,n=>{if(n.type==='CallExpression'&&Number.isInteger(n.arguments[0]?.value)&&modules.has(n.arguments[0].value)){const d=modules.get(n.arguments[0].value);if(d.raw.slice(d.node.start,d.node.end).includes('DoubleClickEvent=class')){eventModule=d;selected.add(d.id)}}});
    }
  }
  if (!keyModule || !hashModule || !eventModule || !constantsModule) throw Error('Missing actual dependency role');
  walk(hashModule.node.value,n=>{if(n.type==='CallExpression'&&Number.isInteger(n.arguments[0]?.value)&&modules.has(n.arguments[0].value))selected.add(n.arguments[0].value)});
  const env = {};
  walk(keyModule.node.value,n=>{if(n.type==='VariableDeclarator'&&n.init?.type==='ArrayExpression')env[n.id.name]=data(n.init)});
  let keys;
  walk(keyModule.node.value,n=>{if(n.type==='AssignmentExpression'&&n.left.property?.name==='KEYS')keys=env[n.right.name]});
  const turboClasses = {};
  walk(eventModule.node.value,n=>{if(n.type==='AssignmentExpression'&&n.right.type==='ClassExpression'){
    let value; walk(n.right,c=>{if(c.type==='ReturnStatement'&&c.argument?.type==='ArrayExpression')value=data(c.argument)});
    if (!value) throw Error('Unresolved turbo class'); turboClasses[n.left.property.name]=value;
  }});
  const defaultTurboIds = [];
  walk(factory.node.value,n=>{if(n.type==='AssignmentExpression'&&n.left.property?.name==='generateDefaultTurbos')walk(n.right,c=>{if(c.type==='ArrayExpression'&&c.elements.every(e=>typeof e.value==='string'))defaultTurboIds.push(...data(c))})});
  const factoryModule = modules.get(dependencies.get('x')), turboEvents = {};
  walk(factoryModule.node.value,n=>{if(n.type==='SwitchCase'&&typeof n.test?.value==='string'){
    let klass;walk(n,c=>{if(c.type==='MemberExpression'&&turboClasses[c.property.name])klass=c.property.name});
    if (klass)turboEvents[n.test.value]=turboClasses[klass];
  }});
  const constantsEnv={};
  walk(constantsModule.node.value,n=>{if(n.type==='VariableDeclarator'&&n.init){try{constantsEnv[n.id.name]=data(n.init,constantsEnv)}catch{}}});
  const constants={};
  walk(constantsModule.node.value,n=>{if(n.type==='AssignmentExpression'&&n.left.object?.name==='t'&&n.right.type==='Identifier'&&Object.hasOwn(constantsEnv,n.right.name))constants[n.left.property.name]=constantsEnv[n.right.name]});
  const receipts=[...selected].map(id=>{
    const m=modules.get(id);if(!m)throw Error(`Missing selected module ${id}`);
    const source=m.raw.slice(m.node.start,m.node.end);
    return {module_id:id,source_file:m.path,offset:m.node.start,end:m.node.end,slice_sha256:sha(source),source};
  });
  const details={keys,default_turbo_ids:defaultTurboIds,turbo_events:turboEvents,constants};
  allData[pid]=details;
  products.push({product_id:pid,active_task_maker_module:callerId,active_reexport_module:reexportId,active_factory_module:factoryId,
    dependency_roles:{keys:keyModule.id,hash:hashModule.id,turbo_events:eventModule.id,constants:constantsModule.id},
    files:[...new Set(receipts.map(r=>r.source_file))].map(p=>files.get(p)),receipts,
    decoded_data_sha256:sha(JSON.stringify(details))});
}
fs.writeFileSync('docs/re/mapping-engine-current-source.json',JSON.stringify({method:'Acorn static syntax inspection and restricted literal AST decoding; vendor JS never executed',offset_units:'UTF-16 code units; end exclusive',products},null,2)+'\n');
fs.writeFileSync('assets/data/mapping-engine-current-data.json',JSON.stringify(allData,null,2)+'\n');
console.log(JSON.stringify(products.map(p=>({pid:p.product_id,modules:p.receipts.length,roles:p.dependency_roles,decoded_data_sha256:p.decoded_data_sha256}))));
