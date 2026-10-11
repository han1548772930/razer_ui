// Static webpack/Acorn receipts. No downloaded code is evaluated or loaded.
const fs = require('fs'), acorn = require('acorn'), crypto = require('crypto');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
function walk(node, visit) {
  if (!node?.type) return;
  visit(node);
  for (const v of Object.values(node)) {
    if (Array.isArray(v)) for (const child of v) walk(child, visit);
    else if (v?.type) walk(v, visit);
  }
}
const ids = new Set([1418, 22130, 28927, 36840, 85654, 22167, 76760, 13205, 26091, 47399, 70035, 69427, 83207, 87887, 93305, 94417, 47973, 35924]);
for (const pid of [2676, 2684]) {
  const dir = `local-ui-reverse/source/official/apps.razer.com/synapse/products/${pid}/mw`;
  const main = fs.readdirSync(dir).filter(p => /^main\..*\.js$/.test(p));
  if (main.length !== 1) throw Error('Ambiguous middleware bundle ' + pid);
  const source = `${dir}/${main[0]}`, bytes = fs.readFileSync(source), raw = bytes.toString('utf8');
  const tree = acorn.parse(raw, {ecmaVersion: 'latest'}), modules = [], entry_features = [], runtime_chunks = [];
  walk(tree, node => {
    if (node.type === 'CallExpression' && node.callee?.property?.name === 'useFeature') entry_features.push({offset:node.start,end:node.end,source:raw.slice(node.start,node.end)});
    if (node.type !== 'AssignmentExpression' || node.left?.property?.name !== 'u' || node.right?.type !== 'ArrowFunctionExpression') return;
    const maps = [];
    walk(node.right, n => {
      if (n.type === 'ObjectExpression' && n.properties.every(p=>p.type==='Property' && p.value.type==='Literal')) maps.push(Object.fromEntries(n.properties.map(p=>[p.key.value ?? p.key.name,p.value.value])));
    });
    if (maps.length !== 2 || !raw.slice(node.right.start,node.right.end).endsWith('+".js")')) throw Error('Unexpected chunk filename syntax');
    for (const id of [2620,2661,6747,9611]) runtime_chunks.push({chunk:id,file:`${maps[0][id] ?? id}.${maps[1][id]}.js`,runtime_offset:node.start,runtime_end:node.end,runtime_source:raw.slice(node.start,node.end)});
  });
  walk(tree, node => {
    if (node.type !== 'Property' || !ids.has(node.key.value) || !['ArrowFunctionExpression', 'FunctionExpression'].includes(node.value.type)) return;
    const value = node.value, imports = [], methods = [], assignments = [], export_maps = [];
    walk(value, item => {
      if (item.type === 'VariableDeclarator' && item.init?.type === 'CallExpression' && item.init.arguments[0]?.type === 'Literal' && typeof item.init.arguments[0].value === 'number') imports.push({symbol:item.id.name,module:item.init.arguments[0].value});
      if (item.type === 'CallExpression' && item.callee?.property?.name === 'd' && item.arguments[1]?.type === 'ObjectExpression') export_maps.push(raw.slice(item.arguments[1].start,item.arguments[1].end));
      if (item.type === 'MethodDefinition') methods.push({name:item.key.name ?? item.key.value,offset:item.start,end:item.end,source:raw.slice(item.start,item.end)});
      if (item.type === 'AssignmentExpression' && item.left?.object?.type === 'ThisExpression') assignments.push({name:item.left.property.name,offset:item.start,end:item.end,source:raw.slice(item.start,item.end)});
    });
    modules.push({module:node.key.value,offset:value.start,end:value.end,sha256:hash(raw.slice(value.start,value.end)),imports,export_maps,methods,assignments,source:raw.slice(value.start,value.end)});
  });
  if (modules.length !== ids.size) throw Error(`Missing module ${pid}: ${modules.length}/${ids.size}`);
  const out = {product_id:pid,source,sha256:hash(bytes),bytes:bytes.length,parser:'Acorn static syntax only',offset_units:'UTF-16 code units',entry_features,runtime_chunks,modules};
  fs.writeFileSync(`docs/re/gamepad-${pid}-calibration-middleware-live-source.json`, JSON.stringify(out,null,2)+'\n');
  console.log(JSON.stringify({product_id:pid,source,sha256:out.sha256,modules:modules.map(m=>({module:m.module,offset:m.offset,end:m.end,exports:m.export_maps}))}));
}
