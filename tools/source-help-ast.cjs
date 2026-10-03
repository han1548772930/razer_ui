// Parse current product JavaScript as Acorn data. Never evaluate downloaded code.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const hash = s => crypto.createHash('sha256').update(s).digest('hex');
const key = n => n?.name ?? n?.value;
const catalog = JSON.parse(read('docs/re/unimplemented-products.json')).products;
const selected = process.argv.slice(2).filter(v=>/^\d+$/.test(v)).map(Number);
const shard=process.argv.slice(2).find(v=>v.startsWith('--shard='))?.slice(8).split('/').map(Number);
function walk(n, f) {
  if (!n?.type || f(n) === false) return;
  for (const v of Object.values(n)) {
    if (Array.isArray(v)) { for (const c of v) if (c?.type) walk(c, f); }
    else if (v?.type) walk(v, f);
  }
}
function inspect(product, pageKeys = ['HELP']) {
  const file = product.config.path;
  const directory=path.posix.dirname(file);
  const chunks=fs.readdirSync(path.join(root,directory)).filter(f=>f.endsWith('.js')&&!f.startsWith('trans-')).sort().map(f=>directory+'/'+f);
  const files = [...new Set([file,...product.navigation.map(n=>n.source)])];
  const parts=[], parsed=[], texts=new Map(), missingModules=new Set();let source='';
  const scopes = new WeakMap(), modules = new Map(), declarations = [], objects = [], nodes = new Map();
  function loadFile(path) {
    if(parts.some(p=>p.path===path))return;
    const text=texts.get(path)??read(path), expected=path===file?product.config.sha256:product.navigation.find(n=>n.source===path)?.sha256 ?? hash(text);
    if(hash(text)!==expected)throw Error(`Changed source ${path}`);
    const start=source.length;
    parts.push({path,sha256:expected,start,end:start+text.length});source+=text+'\n;\n';
    const ast=acorn.parse(text,{ecmaVersion:'latest'});
    walk(ast,n=>{n.start+=start;n.end+=start;});
    parsed.push(ast);visit(ast,null,null);
  }
  function moduleFor(id) {
    if(modules.has(id))return modules.get(id);
    if(missingModules.has(id))return null;
    // Lazily parse only files exporting an imported module. Regex is a file
    // locator; Acorn and lexical export resolution supply the evidence.
    const pattern=new RegExp(`(?:^|[,{])${id}:`);
    for(const path of chunks) {
      if(parts.some(p=>p.path===path))continue;
      let text=texts.get(path);if(text===undefined){text=read(path);texts.set(path,text);}
      if(!pattern.test(text))continue;
      loadFile(path);if(modules.has(id))return modules.get(id);
    }
    missingModules.add(id);return null;
  }
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
      for (const p of n.arguments[1].properties) if (p.value.body) { let value=p.value.body; if(value.type==='BlockStatement'&&value.body.length===1&&value.body[0].type==='ReturnStatement')value=value.body[0].argument; exports.set(key(p.key),value); }
      scope.exports.set(n.arguments[0].name, exports);
    }
    for (const v of Object.values(n)) {
      if (Array.isArray(v)) { for (const c of v) if (c?.type) visit(c, scope, module); }
      else if (v?.type) visit(v, scope, module);
    }
  }
  for(const path of files)loadFile(path);
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
        const owner=moduleFor(object.arguments[0].value);
        const target=owner?.scope.exports.get(owner.fn.params[1]?.name)?.get(key(n.property));
        if(target) return resolve(target,seen);
      }
    }
    return n;
  }
  function decode(n, seen=new Set()) {
    if(!n || seen.has(n)) throw Error('Unresolved literal');
    const r=resolve(n);
    if(r!==n) return decode(r,new Set([...seen,n]));
    const value=c=>decode(c,new Set([...seen,n]));
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
    if(n.type==='CallExpression' && n.callee.type==='Identifier' && n.callee.name==='structuredClone' && n.arguments.length===1) return value(n.arguments[0]);
    throw Error(`Nonliteral ${n.type}: ${source.slice(n.start,n.end).slice(0,100)}`);
  }
  function receipt(n) {const part=parts.find(p=>p.start<=n.start&&p.end>=n.end);return {path:part.path,offset:n.start-part.start,end:n.end-part.start,source:source.slice(n.start,n.end)};}
  const pages=[], referencedArrays=new Set();
  for(const nav of product.navigation) for(const item of nav.items) {
    if(!pageKeys.includes(item.name?.value))continue;
    const part=parts.find(p=>p.path===nav.source);
    const navNode=objects.find(n=>n.start===item.offset+part.start);
    if(!navNode)throw Error(`Missing navigation ${item.offset}`);
    let component=navNode.properties.find(p=>['component','renderComponent'].includes(key(p.key)))?.value;
    let route=null;
    if(!component) {
      const owner=nodes.get(nav.owner_offset+part.start), matches=[];
      walk(owner,n=>{if(n.type==='Property'&&n.computed)try{if(decode(n.key)===item.name.value)matches.push(n);}catch{}});
      if(matches.length!==1)throw Error(`Unresolved render route ${product.product_id}/${item.name.value}: ${matches.length}`);
      component=matches[0].value;route=receipt(matches[0]);
    }
    const seen=new Set(),components=[];
    function trace(node) {
      node=resolve(node);if(!node||seen.has(node))return;seen.add(node);
      // Some current roots (notably Hue HOME) choose an onboarding/workspace
      // JSX expression directly. Preserve both branches without evaluating the
      // runtime condition; neither branch is a callable component by itself.
      if(node.type==='ConditionalExpression') {trace(node.consequent);trace(node.alternate);return;}
      if(node.type==='LogicalExpression') {trace(node.left);trace(node.right);return;}
      // A webpack module table is data, never a React component. Unresolved
      // members and parameter aliases must not pull a whole bundle into a page.
      if(!/FunctionExpression|FunctionDeclaration|ArrowFunctionExpression|ClassExpression|ClassDeclaration|CallExpression/.test(node.type))return;
      if(node.type==='CallExpression') for(const a of node.arguments) if(a.type==='Identifier'||a.type==='CallExpression'||/Function|Class/.test(a.type))trace(a);
      const jsx=[];
      // Follow a declared lazy module's default export, never execute import().
      walk(node, child=>{
        if(child.type==='CallExpression' && key(child.callee.property)==='bind' && child.arguments[1]?.type==='Literal') {
          const owner=moduleFor(child.arguments[1].value);
          const target=owner?.scope.exports.get(owner.fn.params[1]?.name)?.get('default');
          if(target)trace(target);
        }
      });
      walk(node,n=>{
        if(n.type==='Identifier') {const target=resolve(n);if(target?.type==='ArrayExpression')referencedArrays.add(target);}
        if(n.type==='CallExpression'&&n.callee.type==='SequenceExpression'&&['jsx','jsxs'].includes(key(n.callee.expressions.at(-1)?.property))) {
          const props=n.arguments[1];
          const properties={}, expressions={};
          for(const p of props?.properties??[]) {
            if(p.type!=='Property'||key(p.key)==='children')continue;
            try{properties[key(p.key)]=decode(p.value);}catch{expressions[key(p.key)]=source.slice(p.value.start,p.value.end);}
          }
          jsx.push({component:source.slice(n.arguments[0].start,n.arguments[0].end),props:properties,expressions,offset:n.start});
          if(n.arguments[0].type!=='Literal')trace(n.arguments[0]);
        }
      });
      if(jsx.length || /Function|Class/.test(node.type))components.push({...receipt(node),jsx,render: /Class/.test(node.type)?node.body.body.filter(m=>key(m.key)==='render').map(m=>receipt(m.value)):[]});
    }
    trace(component);
    const default_props=[];
    for(const ast of parsed)walk(ast,n=>{
      if(n.type==='AssignmentExpression'&&key(n.left.property)==='defaultProps'&&seen.has(resolve(n.left.object))) {
        const values={};
        for(const p of n.right.properties??[])try{values[key(p.key)]=decode(p.value);}catch{}
        default_props.push({values,...receipt(n)});
      }
    });
    pages.push({key:item.name.value,path:nav.source,offset:item.offset,component:receipt(component),route,components,default_props});
  }
  return {product_id:product.product_id,name:product.name,source_files:parts.map(({path,sha256})=>({path,sha256})),pages};
}
module.exports = { inspect };
if (require.main === module) {
const results=[];
for(const product of catalog.filter((p,index)=>(!selected.length||selected.includes(p.product_id))&&(!shard||index%shard[1]===shard[0]))) {
  const nav=product.navigation.find(n=>n.items.some(i=>i.name?.value==='HELP'));
  if(!nav) { results.push({product_id:product.product_id,name:product.name,source_files:[],pages:[]});continue; }
  results.push(inspect({...product,config:{path:nav.source,sha256:nav.sha256}}));
  console.log(`Parsed Help ${product.product_id}`);
}
fs.writeFileSync(path.join(root,shard?`.work/source-help-${shard[0]}.json`:'docs/re/source-help-evidence.json'),JSON.stringify({schema_version:1,scanner_sha256:hash(fs.readFileSync(__filename)),products:results},null,2)+'\n');
console.log(`Parsed ${results.length} Help products without executing source.`);
}
