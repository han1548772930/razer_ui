// Parse current product JavaScript as Acorn data. Never evaluate downloaded code.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const hash = s => crypto.createHash('sha256').update(s).digest('hex');
const key = n => n?.name ?? n?.value;
const catalog = JSON.parse(read('docs/re/source-product-configs.json')).products;
const selected = process.argv.slice(2).map(Number);
function walk(n, f) {
  if (!n?.type || f(n) === false) return;
  for (const v of Object.values(n)) {
    if (Array.isArray(v)) { for (const c of v) if (c?.type) walk(c, f); }
    else if (v?.type) walk(v, f);
  }
}
function inspect(product) {
  const file = product.config.path;
  const directory=path.posix.dirname(file);
  const chunks=fs.readdirSync(path.join(root,directory)).filter(f=>f.endsWith('.js')&&!f.startsWith('trans-')).sort().map(f=>directory+'/'+f);
  const files = [...new Set([file,...product.navigation.map(n=>n.source),...chunks])];
  const parts=[];let source='';
  for(const path of files) {
    const text=read(path), expected=path===file?product.config.sha256:product.navigation.find(n=>n.source===path)?.sha256 ?? hash(text);
    if(hash(text)!==expected)throw Error(`Changed source ${path}`);
    parts.push({path,sha256:expected,start:source.length,end:source.length+text.length});source+=text+'\n;\n';
  }
  const ast = acorn.parse(source, {ecmaVersion: 'latest'});
  const scopes = new WeakMap(), modules = new Map(), declarations = [], objects = [], nodes = new Map();
  function bindPattern(n, scope) {
    if(!n)return;
    if(n.type==='Identifier')scope.defs.set(n.name,null);
    else if(n.type==='AssignmentPattern')bindPattern(n.left,scope);
    else if(n.type==='RestElement')bindPattern(n.argument,scope);
    else if(n.type==='ArrayPattern')for(const item of n.elements)bindPattern(item,scope);
    else if(n.type==='ObjectPattern')for(const p of n.properties)bindPattern(p.type==='RestElement'?p.argument:p.value,scope);
  }
  function visit(n, scope, module) {
    if (!n?.type) return;
    if (n.type === 'Property' && Number.isInteger(key(n.key)) && /Function/.test(n.value.type)) {
      module = {id: key(n.key), fn: n.value}; modules.set(module.id, module);
    }
    if (n.type === 'FunctionDeclaration' || n.type === 'ClassDeclaration') scope?.defs.set(n.id.name, n);
    if (/Function/.test(n.type) || n.type === 'Program') {
      scope = {parent: scope, defs: new Map(), exports: new Map(), module};
      for(const parameter of n.params??[])bindPattern(parameter,scope);
      if (module?.fn === n) module.scope = scope;
    }
    scopes.set(n, scope); nodes.set(n.start, n);
    if (n.type === 'VariableDeclarator' && n.id.type !== 'Identifier') bindPattern(n.id,scope);
    if (n.type === 'VariableDeclarator' && n.id.type === 'Identifier') { scope.defs.set(n.id.name, n.init); declarations.push(n); }
    if (n.type === 'ObjectExpression') objects.push(n);
    if (n.type === 'CallExpression' && n.callee.type === 'MemberExpression' && key(n.callee.property) === 'd' && n.arguments[1]?.type === 'ObjectExpression') {
      const exports = new Map();
      for (const p of n.arguments[1].properties) if (p.value.body) exports.set(key(p.key), p.value.body);
      scope.exports.set(n.arguments[0].name, exports);
    }
    for (const v of Object.values(n)) {
      if (Array.isArray(v)) { for (const c of v) if (c?.type) visit(c, scope, module); }
      else if (v?.type) visit(v, scope, module);
    }
  }
  visit(ast, null, null);
  function lookup(name, scope) { while (scope) { if (scope.defs.has(name)) return scope.defs.get(name); scope = scope.parent; } return null; }
  function resolve(n, seen = new Set()) {
    if (!n || seen.has(n)) return n;
    seen = new Set([...seen,n]);
    const scope = scopes.get(n);
    if (n.type === 'Identifier') return resolve(lookup(n.name, scope), seen) ?? n;
    if (n.type === 'SequenceExpression') return resolve(n.expressions.at(-1),seen);
    if (n.type === 'MemberExpression' && n.object.type === 'Identifier') {
      let s=scope, exported;
      while(s && !exported) {exported=s.exports.get(n.object.name)?.get(key(n.property));s=s.parent;}
      if(exported) return resolve(exported,seen);
      const object=lookup(n.object.name,scope);
      if(object?.type==='CallExpression' && object.arguments[0]?.type==='Literal') {
        const owner=modules.get(object.arguments[0].value);
        const target=owner?.scope.exports.get(owner.fn.params[1]?.name)?.get(key(n.property));
        if(target) return resolve(target,seen);
      }
    }
    return n;
  }
  function decode(n, seen=new Set(), bindings=new Map()) {
    if(n?.type==='Identifier'&&bindings.has(n.name))return bindings.get(n.name);
    if(!n || seen.has(n)) throw Error('Unresolved literal');
    const r=resolve(n);
    if(r!==n) return decode(r,new Set([...seen,n]),bindings);
    const value=c=>decode(c,new Set([...seen,n]),bindings);
    if(n.type==='Literal') return n.value;
    if(n.type==='UnaryExpression') {
      if(n.operator==='!') return !value(n.argument);
      if(n.operator==='-') return -value(n.argument);
      if(n.operator==='void') return null;
    }
    if(n.type==='ArrayExpression') return n.elements.flatMap(c=>c.type==='SpreadElement'?value(c.argument):[value(c)]);
    if(n.type==='ObjectExpression') {
      const out={};
      for(const p of n.properties) {
        if(p.type==='SpreadElement') Object.assign(out,value(p.argument));
        else out[p.computed?value(p.key):key(p.key)]=value(p.value);
      }
      return out;
    }
    if(n.type==='MemberExpression') {
      const object=value(n.object), field=n.computed?value(n.property):key(n.property);
      if(Object.hasOwn(object,field)) return object[field];
    }
    if(n.type==='BinaryExpression') {
      const a=value(n.left),b=value(n.right);
      switch(n.operator){case '+':return a+b;case '-':return a-b;case '*':return a*b;case '/':return a/b;case '===':return a===b;case '!==':return a!==b;case '<':return a<b;case '>':return a>b;}
    }
    if(n.type==='TemplateLiteral')return n.quasis.map((q,i)=>q.value.cooked+(i<n.expressions.length?String(value(n.expressions[i])):'')).join('');
    if(n.type==='CallExpression') {
      const target=resolve(n.callee),helper=target&&source.slice(target.start,target.end);
      // Inspect the transpiler helper; only combine already-decoded data. No
      // downloaded helper function, getter or callback is ever called.
      if(helper&&/Object\.getOwnPropertyDescriptors/.test(helper)&&/Object\.defineProperties/.test(helper)&&/Function/.test(target.type)) {
        const args=n.arguments.map(value);if(args.every(a=>a&&typeof a==='object'&&!Array.isArray(a)))return Object.assign({},...args);
      }
      if(n.callee.type==='MemberExpression') {
        const method=key(n.callee.property);
        if(n.callee.object.name==='Object'&&method==='freeze')return value(n.arguments[0]);
        const receiver=value(n.callee.object);
        if(method==='concat'&&(typeof receiver==='string'||Array.isArray(receiver)))return receiver.concat(...n.arguments.map(value));
        if(['map','find'].includes(method)&&Array.isArray(receiver)) {
          const lambda=n.arguments[0];if(lambda?.type!=='ArrowFunctionExpression'||lambda.body.type==='BlockStatement'||lambda.params.some(p=>p.type!=='Identifier'))throw Error('Nonliteral callback');
          const mapped=receiver.map((item,ix)=>{const child=new Map(bindings);child.set(lambda.params[0].name,item);if(lambda.params[1])child.set(lambda.params[1].name,ix);return decode(lambda.body,new Set(),child);});
          return method==='map'?mapped:receiver[mapped.findIndex(Boolean)];
        }
      }
    }
    if(n.type==='CallExpression' && n.callee.type==='Identifier' && n.callee.name==='structuredClone' && n.arguments.length===1) return value(n.arguments[0]);
    throw Error(`Nonliteral ${n.type}: ${source.slice(n.start,n.end).slice(0,100)}`);
  }
  function receipt(n) {const part=parts.find(p=>p.start<=n.start&&p.end>=n.end);return {path:part.path,offset:n.start-part.start,end:n.end-part.start,source:source.slice(n.start,n.end)};}
  const config=modules.get(product.config.module_id) || [...modules.values()].find(m=>m.fn.start===product.config.offset), values={}, unresolved=[];
  for(const [name,n] of config?.scope.exports.get(config.fn.params[1].name) ?? []) {
    let target=resolve(n);
    if(name==='getDefaultControllerActuation' && target.type==='ArrowFunctionExpression') target=target.body;
    if(/^(get|tutorial|CHROMA_OBJECT)/.test(name)&&name!=='getDefaultControllerActuation')continue;
    try {values[name]=decode(target);} catch(e) {unresolved.push({name,reason:e.message,...receipt(target)});}
  }
  const states=[];
  for(const declaration of declarations) {
    const n=declaration.init;
    if(n?.type!=='ObjectExpression')continue;
    const fields=n.properties.map(p=>key(p.key));
    if(!fields.includes('actionsFromUI') && !fields.some(f=>['speakerVolume','micVolume','powerSaving','audioEnhancement','noiseCancellation','micSideTone','hapticIntensity','samplingRate','micEqualizer','audioEqualizer','fanControllerSetting','smartFanCurveSettings','isPercentageDisplayMode','activeThermalIndicator','thermal','fanPresets','isCelsius'].includes(f)))continue;
    try {states.push({symbol:declaration.id.name,fields,value:decode(n),...receipt(n)});}catch(e){unresolved.push({name:declaration.id.name,reason:e.message,...receipt(n)});}
  }
  const pages=[], referencedArrays=new Set();
  for(const nav of product.navigation) for(const item of nav.items) {
    if(!item.component || item.name?.value==='HELP')continue;
    const part=parts.find(p=>p.path===nav.source);
    const navNode=objects.find(n=>n.start===item.offset+part.start);
    if(!navNode)throw Error(`Missing navigation ${item.offset}`);
    const component=navNode.properties.find(p=>['component','renderComponent'].includes(key(p.key)))?.value;
    const seen=new Set(),components=[];
    function trace(node) {
      node=resolve(node);if(!node||seen.has(node))return;seen.add(node);
      // A webpack module table is data, never a React component. Unresolved
      // members and parameter aliases must not pull a whole bundle into a page.
      if(!/FunctionExpression|FunctionDeclaration|ArrowFunctionExpression|ClassExpression|ClassDeclaration|CallExpression/.test(node.type))return;
      if(node.type==='CallExpression') for(const a of node.arguments) if(a.type==='Identifier'||a.type==='CallExpression'||/Function|Class/.test(a.type))trace(a);
      const jsx=[];
      walk(node,n=>{
        if(n.type==='Identifier') {const target=resolve(n);if(target?.type==='ArrayExpression')referencedArrays.add(target);}
        if(n.type==='CallExpression'&&n.callee.type==='SequenceExpression'&&['jsx','jsxs'].includes(key(n.callee.expressions.at(-1)?.property))) {
          const props=n.arguments[1];
          const properties={}, expressions={};
          for(const p of props?.properties??[]) {
            if(p.type!=='Property')continue;
            try{properties[key(p.key)]=decode(p.value);}catch{expressions[key(p.key)]=source.slice(p.value.start,p.value.end);}
          }
          jsx.push({component:source.slice(n.arguments[0].start,n.arguments[0].end),props:properties,expressions,offset:n.start});
          if(n.arguments[0].type!=='Literal')trace(n.arguments[0]);
        }
      });
      if(jsx.length || /Function|Class/.test(node.type))components.push({...receipt(node),jsx});
    }
    trace(component);
    pages.push({key:item.name.value,path:nav.source,offset:item.offset,components});
  }
  // Retain array declarations referenced by the mounted page, including uneven
  // deadzone steps. Their names are resolved in this source, never borrowed.
  const arrays=[];
  for(const d of declarations) if(d.init?.type==='ArrayExpression'&&referencedArrays.has(d.init)) {
    try {const value=decode(d.init);if(value.length)arrays.push({symbol:d.id.name,value,...receipt(d.init)});}catch{}
  }
  const assignments=[];
  walk(ast,n=>{
    if(n.type!=='ArrayExpression')return;
    const contents=n.elements.map(e=>e?.type==='ObjectExpression'?e.properties.find(p=>key(p.key)==='content')?.value?.value:null);
    if(contents.includes('GLOBAL_SENSITIVITY_CLUTCH')&&contents.every(c=>typeof c==='string'))assignments.push({contents,...receipt(n)});
  });
  const constants={}, scalarObjects=[];
  for(const mod of modules.values()) for(const exports of mod.scope?.exports.values()??[]) for(const [name,node] of exports) {
    try {const v=decode(node);if(v&&typeof v==='object'&&!Array.isArray(v)&&Object.keys(v).some(k=>['RACING','GAMMA_1_4','STRONG','PIP','HDMI1','HDMI_1','QUIET','NORMAL','Smart','Fixed','CPU','TOPLEFT','LOW'].includes(k))) scalarObjects.push({name,value:v,...receipt(resolve(node))});}catch{}
  }
  for(const declaration of declarations) {
    if(declaration.init?.type!=='ObjectExpression'&&declaration.init?.type!=='ArrayExpression')continue;
    try{const v=decode(declaration.init);if(v&&typeof v==='object'&&((Array.isArray(v)&&v.some(x=>x?.temperature!==undefined&&x.fanSpeedValue!==undefined))||Object.keys(v).some(k=>['fanControllerSetting','fanCurve','fanPresets','activeThermalIndicator','fanSpeedValue','isPercentageDisplayMode'].includes(k)))) scalarObjects.push({name:declaration.id.name,value:v,...receipt(declaration.init)});}catch{}
  }
  const buttonGroups=[];
  for(const mod of modules.values()) for(const exports of mod.scope?.exports.values()??[]) {
    const node=exports.get('groupList');
    if(node) try{buttonGroups.push({value:decode(node),...receipt(resolve(node))});}catch(e){unresolved.push({name:'groupList',reason:e.message,...receipt(resolve(node))});}
  }
  return {product_id:product.product_id,name:product.name,source:file,sha256:product.config.sha256,source_files:parts.map(({path,sha256})=>({path,sha256})),config:values,constants,scalarObjects,states,arrays,assignments,buttonGroups,pages,unresolved};
}
const results=catalog.filter(p=>[3858,3880,3893,3900,3907,3921].includes(p.product_id)).map(p=>{console.log(p.product_id);return inspect(p)});
fs.writeFileSync(path.join(root,'docs/re/accessory-system-source.json'),JSON.stringify({schema_version:1,scanner_sha256:hash(fs.readFileSync(__filename)),products:results},null,2)+'\n');
