// Static current mounted MapJoyStick parser. Never evaluates vendor JavaScript.
const fs = require('fs'), crypto = require('crypto'), acorn = require('acorn');
const key = n => n?.name ?? n?.value;
const hash = s => crypto.createHash('sha256').update(s).digest('hex');
function walk(n, visit) { if (!n?.type || visit(n) === false) return; for (const v of Object.values(n)) for (const c of Array.isArray(v) ? v : v?.type ? [v] : []) walk(c, visit); }
function module(s, id) {
  const h = new RegExp('(?:^|[,\\{])' + id + ':').exec(s); if (!h) throw Error('Missing module ' + id);
  let node = acorn.parseExpressionAt(s, h.index + h[0].length, {ecmaVersion:'latest'}); if (node.type === 'SequenceExpression') node = node.expressions[0];
  const vars = new Map(), exports = new Map();
  walk(node.body, n => { if (n !== node.body && /Function|Class/.test(n.type)) return false; if (n.type === 'VariableDeclarator') vars.set(key(n.id), n.init); if (n.type === 'CallExpression' && key(n.callee.property) === 'd' && n.arguments[1]?.type === 'ObjectExpression') for (const p of n.arguments[1].properties) exports.set(key(p.key), p.value.body); });
  return {node, vars, get(name) { const n = exports.get(name); return n?.type === 'Identifier' ? vars.get(n.name) : n; }};
}
function literal(n, labels) {
  if (n.type === 'Literal') return n.value;
  if (n.type === 'ArrayExpression') return n.elements.map(n => literal(n, labels));
  if (n.type === 'ObjectExpression') return Object.fromEntries(n.properties.map(p => [key(p.key), literal(p.value, labels)]));
  if (n.type === 'MemberExpression') return literal(labels.get(key(n.property)), labels);
  throw Error('Not a static literal ' + n.type);
}
const records = [], data = [];
for (const pid of [678, 679, 688]) {
  const root = `local-ui-reverse/source/official/apps.razer.com/synapse/products/${pid}/ui/static/`;
  const mainPath = root + 'js/' + fs.readdirSync(root+'js').find(f => f.startsWith('main.'));
  const main = fs.readFileSync(mainPath, 'utf8'), labels = module(main, 54693);
  const chunkPath = root+'js/'+fs.readdirSync(root+'js').find(f => f.startsWith('MapJoyStick.'));
  const chunk = fs.readFileSync(chunkPath, 'utf8'), mounted = module(chunk, 59184);
  const source = (s, n) => ({byte_offset:Buffer.byteLength(s.slice(0,n.start)),char_range:[n.start,n.end],source:s.slice(n.start,n.end)});
  const arrays = [...mounted.vars.values()].filter(n => n.type === 'ArrayExpression');
  const buttons = literal(arrays.find(n => n.elements.length === 25), labels);
  const directions = literal(arrays.find(n => n.elements.length === 19), labels);
  const methods = []; walk(mounted.node, n => { if (n.type === 'AssignmentExpression' && n.left.object?.type === 'ThisExpression') methods.push({name:key(n.left.property),...source(chunk,n)}); if(n.type==='MethodDefinition') methods.push({name:key(n.key),...source(chunk,n)}); });
  const sharedWidgets=[27734,49412,97278].map(id=>({module:id,...source(main,module(main,id).node)}));
  const helpers = []; for (const [id,names] of [[29267,['uw','rq']],[54693,['n56','PNH','Dcc']],[27734,['A']],[49412,['A']]]) { const m = module(main,id); for(const name of names) helpers.push({module:id,name,...source(main,m.get(name))}); }
  const sharedPath=root+'js/'+fs.readdirSync(root+'js').find(f=>f.startsWith('2383.')),shared=fs.readFileSync(sharedPath,'utf8');
  const sharedTree=acorn.parse(shared,{ecmaVersion:'latest'}), callers=[];walk(sharedTree,n=>{if(n.type==='Property'&&n.computed&&n.value.type==='ArrowFunctionExpression'&&shared.slice(n.start,n.end).includes('59184'))callers.push(source(shared,n));});
  const css=[]; for(const f of fs.readdirSync(root+'css')) { const path=root+'css/'+f,s=fs.readFileSync(path,'utf8');const rules=[...s.matchAll(/[^{}]*(?:\.joystick(?:\s|\{|,)|\.s3-dropdown|\.s3-options|\.radio-new)[^{}]*\{[^}]*\}/g)].map(m=>({byte_offset:Buffer.byteLength(s.slice(0,m.index)),source:m[0]}));if(rules.length)css.push({path,sha256:hash(s),rules}); }
  const arrowPath=root+'media/icon_expand.55a47b0c.svg',embeddedPath='assets/synapse/expand.svg';const arrow=fs.readFileSync(arrowPath),embedded=fs.readFileSync(embeddedPath);if(arrow.toString('utf8').replace(/\r\n/g,'\n')!==embedded.toString('utf8').replace(/\r\n/g,'\n'))throw Error('Current dropdown arrow changed beyond line endings');
  records.push({resources:[{path:arrowPath,sha256:hash(arrow),embedded_path:embeddedPath,embedded_sha256:hash(embedded),verified_byte_equal:arrow.equals(embedded),verified_equal_except_line_endings:true}],product_id:pid,main:{path:mainPath,sha256:hash(main)},chunk:{path:chunkPath,sha256:hash(chunk),module:59184,module_source:source(chunk,mounted.node),methods},shared:{path:sharedPath,sha256:hash(shared),callers},helpers,shared_widgets:sharedWidgets,css});
  data.push({product_id:pid,buttons,directions,button_label:literal(labels.get('n56'),labels),direction_label:literal(labels.get('PNH'),labels),placeholder:literal(labels.get('Dcc'),labels)});
}
fs.writeFileSync('docs/re/keyboard-joystick-current-source.json',JSON.stringify({method:'Static current JS/CSS AST; mounted caller, original raw methods and byte hashes.',records},null,2)+'\n');
if(process.argv.includes('--prepare'))fs.writeFileSync('crates/razer-pages/src/features/keyboard_mapping_joystick_data.json',JSON.stringify(data));
console.log(JSON.stringify(data));
