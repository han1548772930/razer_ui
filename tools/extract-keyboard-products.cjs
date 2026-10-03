// Read current downloaded keyboard sources as Acorn AST data. Never load/eval them.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const catalog = JSON.parse(fs.readFileSync(path.join(root, 'docs/re/unimplemented-products.json'), 'utf8'));
const mouse = process.argv.includes('--mouse');
const mouseReceipts = mouse ? JSON.parse(fs.readFileSync(path.join(root, 'docs/re/mouse-product-source.json'), 'utf8')).products : [];
const selected = process.argv.slice(2).filter(value => value !== '--mouse').map(Number);
const products = catalog.products.filter(p => (mouse ? mouseReceipts.some(r=>r.product_id===p.product_id) : p.family==='keyboard_keypad') && (!selected.length || selected.includes(p.product_id)));
const out = path.join(root, mouse ? 'assets/synapse/mouse-products/configs' : 'assets/synapse/keyboard-products');
fs.mkdirSync(out, {recursive: true});
const key = n => n?.name ?? n?.value;
const hash = s => crypto.createHash('sha256').update(s).digest('hex');
function walk(n, visit) {
  if (!n?.type || visit(n) === false) return;
  for (const v of Object.values(n)) {
    if (Array.isArray(v)) for (const c of v) walk(c, visit);
    else if (v?.type) walk(v, visit);
  }
}
function ownWalk(n, visit) {
  walk(n, c => c !== n && /Function|Class/.test(c.type) ? false : visit(c));
}
function inspect(product) {
  const pid = product.product_id;
  const files = new Set(product.navigation.map(n => n.source));
  const folder = path.join(root, `.ref/devices/${pid}`);
  for (const file of fs.readdirSync(path.join(folder, 'static/js'))) {
    if (/^main\..*\.js$/.test(file)) files.add(`.ref/devices/${pid}/static/js/${file}`);
  }
  const modules = new Map(), evidence = [];
  for (const file of files) {
    const source = fs.readFileSync(path.join(root, file), 'utf8');
    evidence.push({path: file, sha256: hash(source)});
    const ast = acorn.parse(source, {ecmaVersion:'latest'});
    walk(ast, node => {
      if (node.type !== 'Property' || !Number.isInteger(key(node.key)) || !/Function/.test(node.value.type)) return;
      const fn = node.value;
      const mod = {id:key(node.key), file, source, node:fn, vars:new Map(), namespaces:new Map(), require:fn.params[2]?.name, output:fn.params[1]?.name};
      ownWalk(fn.body, n => {
        if (n.type === 'VariableDeclarator' && n.id.type === 'Identifier') mod.vars.set(n.id.name, n.init);
        if (n.type === 'CallExpression' && n.callee.type === 'MemberExpression' && key(n.callee.property)==='d' && n.arguments[1]?.type==='ObjectExpression') {
          const namespace=n.arguments[0]?.name;
          if (!namespace) return;
          const exports=mod.namespaces.get(namespace) ?? new Map();
          for (const prop of n.arguments[1].properties) {
            let body=prop.value?.body;
            if(body?.type==='BlockStatement' && body.body[0]?.type==='ReturnStatement') body=body.body[0].argument;
            if(body) exports.set(key(prop.key),body);
          }
          mod.namespaces.set(namespace,exports);
        }
      });
      if (!modules.has(mod.id)) modules.set(mod.id,mod);
      return false;
    });
  }
  const memo=new Map();
  function evaluate(n,m,seen=new Set()) {
    if (!n) throw Error('No literal');
    const id=`${m.id}:${n.start}:${n.end}`;
    if(memo.has(id)) return memo.get(id);
    if(seen.has(id)) throw Error('Cyclic literal '+id+' '+n.type+' '+(n.name??''));
    seen=new Set([...seen,id]);
    const value=(c,owner=m)=>evaluate(c,owner,seen);
    let result;
    switch(n.type) {
      case 'Literal': result=n.value;break;
      case 'Identifier': if(!m.vars.has(n.name)) throw Error('Unbound '+n.name+' in '+m.id); result=value(m.vars.get(n.name));break;
      case 'UnaryExpression':
        if(n.operator==='!') result=!value(n.argument);
        else if(n.operator==='-') result=-value(n.argument);
        else throw Error('Unsupported unary');
        break;
      case 'ArrayExpression': result=n.elements.flatMap(c=>c.type==='SpreadElement'?value(c.argument):[value(c)]);break;
      case 'TemplateLiteral': result=n.quasis.map((q,i)=>q.value.cooked+(i<n.expressions.length?String(value(n.expressions[i])):'')).join('');break;
      case 'ConditionalExpression': result=value(n.test)?value(n.consequent):value(n.alternate);break;
      case 'LogicalExpression': {
        const left=value(n.left);
        if(n.operator==='&&') result=left&&value(n.right);
        else if(n.operator==='||') result=left||value(n.right);
        else if(n.operator==='??') result=left??value(n.right);
        else throw Error('Unsupported logical');
        break;
      }
      case 'BinaryExpression': {
        const a=value(n.left),b=value(n.right);
        switch(n.operator) {
          case '+':result=a+b;break;case '-':result=a-b;break;
          case '*':result=a*b;break;case '/':result=a/b;break;
          case '===':result=a===b;break;case '!==':result=a!==b;break;
          case '<':result=a<b;break;case '>':result=a>b;break;
          default:throw Error('Unsupported binary '+n.operator);
        }
        break;
      }
      case 'ObjectExpression':
        result={};
        for(const p of n.properties) {
          if(p.type==='SpreadElement') Object.assign(result,value(p.argument));
          else if(p.type==='Property') result[p.computed?value(p.key):key(p.key)]=value(p.value);
          else throw Error('Nonliteral property');
        }
        break;
      case 'MemberExpression': {
        const field=n.computed?value(n.property):key(n.property);
        if(n.object.type==='Identifier') {
          const v=m.vars.get(n.object.name);
          const imported=v?.type==='CallExpression' && v.callee.name===m.require && v.arguments[0]?.value;
          if(Number.isInteger(imported)) {
            const owner=modules.get(imported);
            if(!owner) throw Error('Module missing '+imported);
            result=value(owner.namespaces.get(owner.output)?.get(field),owner);break;
          }
          const exported=m.namespaces.get(n.object.name)?.get(field);
          if(exported) {result=value(exported);break;}
        }
        const object=value(n.object);
        if(!Object.hasOwn(object,field)) throw Error('Unknown member');
        result=object[field];break;
      }
      case 'SequenceExpression': result=value(n.expressions.at(-1));break;
      case 'CallExpression': {
        // Only pure literal string/array operations are interpreted. No source
        // function, imported helper, callback, constructor or getter is called.
        if(n.callee.type!=='MemberExpression') throw Error('Nonliteral call');
        const method=key(n.callee.property),receiver=value(n.callee.object);
        const args=n.arguments.map(c=>value(c));
        if(method==='concat' && typeof receiver==='string' && args.every(x=>['string','number','boolean'].includes(typeof x))) result=receiver.concat(...args);
        else if(method==='concat' && Array.isArray(receiver)) result=receiver.concat(...args);
        else if(method==='toLowerCase' && typeof receiver==='string' && !args.length) result=receiver.toLowerCase();
        else throw Error('Nonliteral method '+method);
        break;
      }
      default: throw Error('Nonliteral '+n.type);
    }
    memo.set(id,result);return result;
  }
  function exported(m,name) {return m.namespaces.get(m.output)?.get(name);}
  const configs=[];
  const expectedId=mouse?mouseReceipts.find(p=>p.product_id===pid)?.config.exports.DeviceInfo.productId:pid;
  for(const m of modules.values()) {
    const node=exported(m,'DeviceInfo');
    if(!node) continue;
    try {const info=evaluate(node,m);if(info.productId===expectedId) configs.push({m,info});}catch{
      // Optional context/functions can be nonliteral while productId remains
      // directly literal. Never evaluate the omitted functions.
      const definition=node.type==='Identifier'?m.vars.get(node.name):node;
      const productId=definition?.properties?.find(p=>key(p.key)==='productId')?.value;
      try {if(evaluate(productId,m)===expectedId)configs.push({m,info:{productId:expectedId}});}catch{}
    }
  }
  if(configs.length!==1) throw Error(`${pid}: expected one exact DeviceInfo, got ${configs.length}`);
  const {m:config,info}=configs[0];
  const configValues={};
  const unresolved=[];
  for(const [name,node] of config.namespaces.get(config.output)) {
    if(/get|^CHROMA_OBJECT$|^FIRMWARE_REPORT_IDS$/.test(name)) continue;
    try {configValues[name]=evaluate(node,config);}catch(e) {
      const definition=node.type==='Identifier'?config.vars.get(node.name):node;
      // Keep independently literal properties when a later optional field is
      // computed. The unresolved fields remain visible in the audit record.
      if(definition?.type==='ObjectExpression') {
        const partial={};
        for(const p of definition.properties) if(p.type==='Property'&&!p.computed) {
          try{partial[key(p.key)]=evaluate(p.value,config);}catch{}
        }
        configValues[name]=partial;
      }
      unresolved.push({name,expression:config.source.slice(definition?.start??node.start,definition?.end??node.end),reason:e.message});
    }
  }
  const layouts=[];
  const layoutErrors=[];
  const groupModules=[];
  function groupForNamespace(namespace,m) {
    const node=m.namespaces.get(namespace)?.get('groupList');
    if(node) return {groups:evaluate(node,m),module:m.id,range:[node.start,node.end]};
    const imported=m.vars.get(namespace);
    if(imported?.type==='CallExpression' && imported.callee.name===m.require) {
      const owner=modules.get(imported.arguments[0]?.value);
      const group=owner && exported(owner,'groupList');
      if(group) return {groups:evaluate(group,owner),module:owner.id,range:[group.start,group.end]};
    }
    return null;
  }
  for(const m of modules.values()) {
    if(![...m.namespaces.values()].some(e=>e.has('groupList'))) continue;
    groupModules.push({module:m.id,namespaces:[...m.namespaces].filter(([k,v])=>v.has('groupList')).map(([k])=>k)});
    walk(m.node.body,n=>{
      if(n.type!=='SwitchStatement') return;
      let pending=[];
      for(const branch of n.cases) {
        if(branch.test) {
          try {const id=evaluate(branch.test,m);if(Number.isInteger(id)) pending.push({id,name:key(branch.test.property)});}catch{}
        }
        let found;
        for(const statement of branch.consequent) {
          if(statement.type==='ExpressionStatement' && statement.expression.type==='AssignmentExpression' && statement.expression.right.type==='Identifier') {
            try {found=groupForNamespace(statement.expression.right.name,m);}catch(e){layoutErrors.push({module:m.id,reason:e.message});}
            if(found) break;
          }
        }
        if(found) {
          for(const item of pending) {
            // JavaScript selects the first duplicate case, even if later cases
            // advertise another unused geometry for the same physical layout.
            if(!layouts.some(l=>l.layout_id===item.id)) layouts.push({layout_id:item.id,layout_name:item.name,source:m.file,resolver_range:[n.start,n.end],...found});
          }
          pending=[];
        } else if(branch.consequent.length) pending=[];
      }
    });
  }
  const manifest=JSON.parse(fs.readFileSync(path.join(folder,'manifest.json'),'utf8'));
  const declared=manifest.layoutId ?? configValues.AVAILABLE_LAYOUT_ID ?? [];
  const baseId=mouse&&modules.has(1368)?1368:21368;
  const base=modules.get(baseId);
  let defaultGroups=null;
  if(base && exported(base,'groupList')) {
    const node=exported(base,'groupList');
    try{defaultGroups={groups:evaluate(node,base),source:base.file,module:base.id,range:[node.start,node.end]};}catch(e){layoutErrors.push({module:baseId,reason:e.message});}
  }
  const result={schema_version:1,product_id:pid,name:product.name,source_files:evidence,
    source_config:{module:config.id,path:config.file,range:[config.node.start,config.node.end]},
    config:configValues,unresolved_config:unresolved,declared_layout_ids:declared,
    navigation:product.navigation,group_modules:groupModules,layout_errors:layoutErrors,default_groups:defaultGroups,layouts:layouts.sort((a,b)=>a.layout_id-b.layout_id)};
  fs.writeFileSync(path.join(out,`${pid}.json`),JSON.stringify(result));
  console.log(JSON.stringify({pid,config:config.id,layouts:layouts.length,declared:declared.length,unresolved:unresolved.map(x=>x.name)}));
}
let failed=false;
for(const product of products) {
  try {inspect(product);} catch(e) {console.error(`${product.product_id}: ${e.stack}`);failed=true;}
}
if(failed) process.exitCode=1;
