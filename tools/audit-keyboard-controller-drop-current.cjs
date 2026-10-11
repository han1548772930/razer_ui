// Static source spans only. Never evaluate vendor JavaScript.
const fs = require('fs'), crypto = require('crypto'), acorn = require('acorn');
const hash = s => crypto.createHash('sha256').update(s).digest('hex');
const key = n => n?.name ?? n?.value;
function walk(n, visit) {
  if (!n?.type || visit(n) === false) return;
  for (const v of Object.values(n)) for (const c of Array.isArray(v) ? v : v?.type ? [v] : []) walk(c, visit);
}
function module(s, id) {
  const hit = new RegExp('(?:^|[,\\{])' + id + ':').exec(s);
  if (!hit) throw Error('Missing module ' + id);
  let node = acorn.parseExpressionAt(s, hit.index + hit[0].length, {ecmaVersion:'latest'});
  if (node.type === 'SequenceExpression') node = node.expressions[0];
  const vars = new Map(), exports = new Map();
  walk(node.body, n => {
    if (n !== node.body && /Function|Class/.test(n.type)) return false;
    if (n.type === 'VariableDeclarator') vars.set(key(n.id), n.init);
    if (n.type === 'CallExpression' && key(n.callee.property) === 'd' && n.arguments[1]?.type === 'ObjectExpression')
      for (const p of n.arguments[1].properties) exports.set(key(p.key), p.value.body);
  });
  return {node, vars, exports, get(name) {
    let n = exports.get(name);
    if (n?.type === 'Identifier') n = vars.get(n.name);
    if (!n) throw Error('Missing export ' + id + '.' + name);
    return n;
  }};
}
const records = [], policies = [];
function literal(n,m) {
  if(n.type==='Literal')return n.value;
  if(n.type==='Identifier')return literal(m.vars.get(n.name),m);
  if(n.type==='UnaryExpression' && n.operator==='!')return !literal(n.argument,m);
  if(n.type==='ArrayExpression')return n.elements.flatMap(x=>x.type==='SpreadElement'?literal(x.argument,m):[literal(x,m)]);
  if(n.type==='ObjectExpression')return Object.fromEntries(n.properties.map(p=>[key(p.key),literal(p.value,m)]));
  if(n.type==='CallExpression' && n.callee.name==='structuredClone' && n.arguments.length===1)return literal(n.arguments[0],m);
  throw Error('Nonliteral source configuration '+n?.type);
}
function catalog(n,m) {
  if(n.type==='Identifier')return catalog(m.vars.get(n.name),m);
  if(n.type==='CallExpression' && n.callee.name==='structuredClone')return catalog(n.arguments[0],m);
  if(n.type==='ArrayExpression')return n.elements.flatMap(x=>x.type==='SpreadElement'?catalog(x.argument,m):[literal(x.properties.find(p=>key(p.key)==='content').value,m)]);
  throw Error('Unknown controller catalog '+n?.type);
}
for (const pid of [678,679,688]) {
  const root = `local-ui-reverse/source/official/apps.razer.com/synapse/products/${pid}/ui/static/js/`;
  const names = fs.readdirSync(root).filter(f => f.startsWith('main.') && f.endsWith('.js'));
  if (names.length !== 1) throw Error('Missing/ambiguous main for ' + pid);
  const path = root + names[0], s = fs.readFileSync(path, 'utf8');
  const receipt = n => ({utf16_range:[n.start,n.end],slice_sha256:hash(s.slice(n.start,n.end)),source:s.slice(n.start,n.end)});
  const helpers = [], methods = [], calls = [];
  for (const [id, exports] of [[3342,['z']], [84350,['getButtons']], [31867,['fH']], [69937,['MJm','ddG']], [61350,['setMappingList','setMappingUpdated']], [13254,['VE0','ho4']], [29267,['n3']], [78193,['UNSUPPORT_HYPERSHIFT_DRAG_MAPPING','OBM_PRESET_PROFILES_HOT_KEYS']]]) {
    const m = module(s,id);
    for (const name of exports) helpers.push({module:id,name,...receipt(m.get(name))});
    if ([3342,84350,61350,78193].includes(id)) helpers.push({module:id,name:'complete-module',...receipt(m.node)});
  }
  const tree = acorn.parse(s,{ecmaVersion:'latest'});
  walk(tree,n=>{
    if (n.type === 'SwitchCase' && ['VE0','ho4'].includes(key(n.test?.property))) calls.push({role:'reducer-case',...receipt(n)});
    if (n.type === 'AssignmentExpression' && n.left.object?.type === 'ThisExpression') {
      const name = key(n.left.property), text = s.slice(n.start,n.end);
      if ((['onMouseUp','onMouseDown'].includes(name) && text.includes('controller-button')) ||
          (['setMappingList','onSetMappingList','checkNotRemapped','getKeyValue','assignKeyboardGroup','assignHyperShiftGroup'].includes(name))) methods.push({name,...receipt(n)});
    }
    if (n.type === 'Property' && key(n.key) === 'setMappingList') calls.push(receipt(n));
  });
  if (methods.filter(m=>m.name==='onMouseUp' && m.source.includes('getData')).length !== 2) throw Error('Missing/ambiguous key shape drop handlers ' + pid);
  if (!calls.length) throw Error('Missing submission prop chain ' + pid);
  const mainNotRemapped = methods.filter(m=>m.name==='checkNotRemapped' && m.source.includes('validCounterKey'));
  if(mainNotRemapped.length!==1)throw Error('Missing/ambiguous mounted MapKeyboard notRemapped '+pid);
  records.push({product_id:pid,main:{path,sha256:hash(s)},helpers,methods,calls});
  const device = module(s,78193), choices = module(s,29267);
  const excluded = device.exports.has('MAPPING_DRAG_EXCLUDED_INPUT_IDS') ? literal(device.get('MAPPING_DRAG_EXCLUDED_INPUT_IDS'),device) : [];
  policies.push({product_id:pid,excluded_inputs:excluded,unsupported_mapping_inputs:device.exports.has('UNSUPPORTED_MAPPING_KEYS')?literal(device.get('UNSUPPORTED_MAPPING_KEYS'),device):[],hyper_excluded_buttons:[...literal(device.get('UNSUPPORT_HYPERSHIFT_DRAG_MAPPING'),device),...literal(device.get('OBM_PRESET_PROFILES_HOT_KEYS'),device)],controller_choices:catalog(choices.get('n3'),choices)});
}
fs.writeFileSync('docs/re/keyboard-controller-drop-current-source.json',JSON.stringify({method:'Inert Acorn AST source spans. Both shape drop handlers and caller props retained independently for every product.',records},null,2)+'\n');
if(process.argv.includes('--prepare'))fs.writeFileSync('crates/razer-pages/src/features/keyboard_controller_drop_data.json',JSON.stringify(policies)+'\n');
console.log(JSON.stringify(records.map(r=>({pid:r.product_id,methods:r.methods.length,calls:r.calls.length,helpers:r.helpers.length}))));
