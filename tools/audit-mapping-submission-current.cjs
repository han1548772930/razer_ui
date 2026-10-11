// Inspect current source as Acorn syntax data; never run vendor JavaScript.
const fs = require('fs'), acorn = require('acorn'), crypto = require('crypto');
const hash = body => crypto.createHash('sha256').update(body).digest('hex');
function walk(node, visit) {
  if (!node?.type || visit(node) === false) return;
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) value.forEach(child => walk(child, visit));
    else if (value?.type) walk(value, visit);
  }
}
if (process.argv.includes('--declare-chunks')) {
  const products = [];
  for (const pid of [190,678,679,688]) {
    const directory = `local-ui-reverse/source/official/apps.razer.com/synapse/products/${pid}/mw`;
    const mains = fs.readdirSync(directory).filter(name => /^main\.[0-9a-f]{20}\.js$/.test(name));
    if (mains.length !== 1) throw Error(`Ambiguous current main: ${pid}`);
    const source = directory + '/' + mains[0], bytes = fs.readFileSync(source), raw = bytes.toString('utf8');
    const tree = acorn.parse(raw, { ecmaVersion: 'latest' });
    let filenameFunction;
    const chunks = new Set();
    walk(tree, node => {
      if (node.type === 'AssignmentExpression' && node.left.type === 'MemberExpression'
          && node.left.property.name === 'u') filenameFunction = node;
      if (node.type === 'CallExpression' && node.callee.type === 'MemberExpression'
          && node.callee.property.name === 'e' && Number.isInteger(node.arguments[0]?.value)) chunks.add(node.arguments[0].value);
    });
    if (!filenameFunction) throw Error(`Missing current filename function: ${pid}`);
    const maps = [];
    walk(filenameFunction.right, node => {
      if (node.type === 'ObjectExpression') maps.push(Object.fromEntries(node.properties.map(property => [property.key.value,property.value.value])));
    });
    const selected = [...chunks].map(id => {
      const basename = maps.find(map => map[id] && !/^[0-9a-f]{20}$/.test(map[id]))?.[id] ?? id;
      const digest = maps.find(map => /^[0-9a-f]{20}$/.test(map[id]))?.[id];
      if (!digest) throw Error(`Missing declared chunk digest: ${pid}/${id}`);
      return { id, name: `${basename}.${digest}.js` };
    });
    products.push({ pid, source, sha256: hash(bytes),
      u: { start: filenameFunction.start, end: filenameFunction.end, source: raw.slice(filenameFunction.start,filenameFunction.end) }, chunks: selected });
  }
  fs.writeFileSync('docs/re/mapping-submission-current-chunk-declarations.json', JSON.stringify(products,null,2) + '\n');
  console.log(JSON.stringify({ declaration_products: products.length, declared_chunks: products.reduce((n,p) => n + p.chunks.length,0) }));
  process.exit(0);
}
const declarations = JSON.parse(fs.readFileSync('docs/re/mapping-submission-current-chunk-declarations.json', 'utf8'));
for (const declaration of declarations) {
  const pid = declaration.pid;
  const directory = `local-ui-reverse/source/official/apps.razer.com/synapse/products/${pid}`;
  const sources = [declaration.source, ...declaration.chunks.map(chunk => `${directory}/mw/${chunk.name}`),
    ...fs.readdirSync(`${directory}/ui/static/js`).filter(name => /^main\..*\.js$/.test(name)).map(name => `${directory}/ui/static/js/${name}`)];
  const files = [];
  for (const source of sources) {
    const bytes = fs.readFileSync(source), raw = bytes.toString('utf8');
    const tree = acorn.parse(raw, { ecmaVersion: 'latest' });
    const receipts = [], seen = new Set();
    const add = (node, name, moduleId) => {
      if (seen.has(`${node.start}:${node.end}`)) return;
      seen.add(`${node.start}:${node.end}`);
      receipts.push({ name, module_id: moduleId, offset: node.start, end: node.end,
        slice_sha256: hash(raw.slice(node.start, node.end)), source: raw.slice(node.start, node.end) });
    };
    // Keep complete owning modules for precise dependency/export resolution.
    walk(tree, node => {
      if (node.type !== 'Property' || !Number.isInteger(node.key.value)
          || !['FunctionExpression', 'ArrowFunctionExpression'].includes(node.value.type)) return;
      const text = raw.slice(node.start, node.end);
      const relevant = source.includes('/ui/')
        ? text.includes('setMappingList:') || text.includes('postMessage({type:"ON_SET_KEYMAPPING"')
        : /taskMakerSetKeyMapping|generateAppEngineMappings=|getMappingDefault:|getLocalStorageDeviceCache|setMappingEngineLocalStorage|nextVersion=function/.test(text)
          || (pid !== 190 && [11469,93490,3327,51765,56839].includes(node.key.value))
          || (pid === 190 && [80071,97564,97518,71616,20297,54180,9367].includes(node.key.value));
      if (relevant) {
        add(node, `owning-webpack:${node.key.value}`, node.key.value);
        walk(node.value, child => {
          const name = child.type === 'FunctionDeclaration' ? child.id?.name
            : child.type === 'AssignmentExpression' && child.left.type === 'MemberExpression' ? child.left.property.name
            : child.type === 'MethodDefinition' ? child.key.name : null;
          if (!name) return;
          const body = raw.slice(child.start, child.end);
          if (/taskMakerSetKeyMapping|generateAppEngineMappings|ON_SET_KEYMAPPING|setMappingEngineLocalStorage|nextVersion|getMappingDefault/.test(body)) {
            add(child, name, node.key.value);
          }
        });
      }
      return false;
    });
    files.push({ source, bytes: bytes.length, sha256: hash(bytes), receipts });
  }
  fs.writeFileSync(`docs/re/mapping-submission-${pid}-current-source.json`, JSON.stringify({
    product_id: pid, method: 'Current official Acorn static syntax inspection; vendor code never executed',
    offset_units: 'UTF-16 code units, end exclusive', files,
    device_write_completion_is_separate_from_task_maker_completion: true,
    gaps: ['Direct app-engine generation, native memory-storage producer/consumer, every OBM mapping opcode and source service persistence require implementation; this receipt alone is not completion.'],
  }, null, 2) + '\n');
  console.log(JSON.stringify({ pid, files: files.length, receipts: files.reduce((n, file) => n + file.receipts.length, 0) }));
}
const hostFiles = [];
for (const relative of ['electron/modules/mapping_engine/win/index.js', 'electron/modules/memory_storage/index.js']) {
  const source = 'local-ui-reverse/source/installed/app-4.0.827/' + relative;
  const bytes = fs.readFileSync(source), raw = bytes.toString('utf8');
  const tree = acorn.parse(raw, { ecmaVersion: 'latest' });
  const receipts = [];
  walk(tree, node => {
    const key = node.key?.name ?? node.key?.value;
    if (['PropertyDefinition','MethodDefinition'].includes(node.type)
        && ['localStorageSetItem','callFunction'].includes(key)) {
      receipts.push({ name: key, offset: node.start, end: node.end, source: raw.slice(node.start,node.end) });
    }
  });
  hostFiles.push({ source, bytes: bytes.length, sha256: hash(bytes), complete_source: raw, receipts });
}
fs.writeFileSync('docs/re/mapping-submission-host-current-source.json', JSON.stringify({
  host_version: '4.0.827', method: 'Acorn static syntax only, native targets not invoked',
  offset_units: 'UTF-16 code units, end exclusive', files: hostFiles,
}, null, 2) + '\n');
