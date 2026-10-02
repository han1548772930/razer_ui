// Parse downloaded source as data. Never import, eval or execute a Razer bundle.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const acorn = require('acorn');
const {spawnSync} = require('child_process');
const root = path.resolve(__dirname, '..');
const catalog = JSON.parse(fs.readFileSync(path.join(root, '.ref/discovery/products.json'), 'utf8'));
const arguments = process.argv.slice(2);
const merge = arguments.includes('--merge');
const refresh = arguments.includes('--refresh');
const worker = arguments.includes('--worker');
const selected = arguments.filter(value => !['--merge', '--refresh', '--worker'].includes(value)).map(Number);
if (selected.some(value => !Number.isSafeInteger(value) || value < 1)) throw Error('Expected product IDs, optionally --merge');
if (worker && selected.length !== 1) throw Error('A worker parses exactly one product');
const cachePath = path.join(root, '.ref/discovery/interfaces.json');
const checkpointDir = path.join(root, '.ref/discovery/interface-checkpoints');
const sha256 = value => crypto.createHash('sha256').update(value).digest('hex');
const scannerHash = sha256(fs.readFileSync(__filename));
const cached = new Map();
const stringMaps = new Map();
const productsWithNavigation = new Set();
if (fs.existsSync(cachePath)) {
  const previous = JSON.parse(fs.readFileSync(cachePath, 'utf8'));
  if (previous.scanner_sha256 === scannerHash) {
    for (const product of previous.products) {
      if (product.files.some(file => file.mounted_navigation?.length)) productsWithNavigation.add(product.product_id);
    }
    for (const product of previous.products) for (const file of product.files) {
      if (!file.parse_error && file.sha256) cached.set(file.path, file);
    }
  }
}
const key = n => n?.name ?? n?.value;
const prop = (n, name) => n?.properties?.find(p => p.type === 'Property' && key(p.key) === name)?.value;

function walk(node, visit) {
  if (!node?.type || visit(node) === false) return;
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) for (const child of value) { if (child?.type) walk(child, visit); }
    else if (value?.type) walk(value, visit);
  }
}

function ownWalk(node, visit) {
  // Constructor-local bindings only; arrow handlers have independent scopes.
  walk(node, child => {
    if (child !== node && /Function|Class/.test(child.type)) return false;
    return visit(child);
  });
}

function exportedStrings(ast) {
  const values = new Map();
  walk(ast, node => {
    if (node.type !== 'Property' || typeof key(node.key) !== 'number' ||
        !['ArrowFunctionExpression', 'FunctionExpression'].includes(node.value.type)) return;
    const locals = new Map();
    ownWalk(node.value.body, n => {
      if (n.type === 'VariableDeclarator' && n.id.type === 'Identifier' &&
          n.init?.type === 'Literal') locals.set(n.id.name, n.init.value);
    });
    ownWalk(node.value.body, n => {
      if (n.type !== 'CallExpression' || n.callee.type !== 'MemberExpression' ||
          key(n.callee.property) !== 'd' || n.arguments[1]?.type !== 'ObjectExpression') return;
      for (const p of n.arguments[1].properties) {
        let body = p.value?.body;
        if (body?.type === 'BlockStatement' && body.body.length === 1 && body.body[0].type === 'ReturnStatement') body = body.body[0].argument;
        const value = body?.type === 'Identifier' ? locals.get(body.name) : body?.value;
        if (typeof value !== 'string') continue;
        const name = key(p.key);
        if (!values.has(name)) values.set(name, new Set());
        values.get(name).add(value);
      }
    });
    return false;
  });
  return values;
}

function inspect(source, file, inheritedStrings = new Map()) {
  const ast = acorn.parse(source, {ecmaVersion: 'latest', sourceType: 'script'});
  const strings = exportedStrings(ast);
  for (const [name, values] of inheritedStrings) {
    if (!strings.has(name)) strings.set(name, new Set());
    for (const value of values) strings.get(name).add(value);
  }
  stringMaps.set(file, strings);
  const routes = [], navigationCandidates = [], overlayCandidates = [], lazyRoots = [];
  const snippet = n => n ? source.slice(n.start, n.end) : null;
  // Lexical lookup avoids mixing identically minified names from separate
  // Webpack modules (e.g. n_ can be HOME, a translation, or an action type).
  const parents = new WeakMap(), scopes = new WeakMap();
  function index(node, scope, parent) {
    if (!node?.type) return;
    parents.set(node, parent);
    if (node.type === 'FunctionDeclaration' && node.id) scope?.bindings.set(node.id.name, node);
    if (node.type === 'Program' || node.type === 'BlockStatement' || /Function/.test(node.type)) {
      scope = {parent: scope, bindings: new Map()};
      for (const parameter of node.params ?? []) if (parameter.type === 'Identifier') scope.bindings.set(parameter.name, null);
    }
    scopes.set(node, scope);
    if (node.type === 'VariableDeclarator' && node.id.type === 'Identifier') scope.bindings.set(node.id.name, node.init);
    for (const value of Object.values(node)) {
      if (Array.isArray(value)) for (const child of value) { if (child?.type) index(child, scope, node); }
      else if (value?.type) index(value, scope, node);
    }
  }
  index(ast, null, null);
  const binding = node => {
    for (let scope = scopes.get(node); scope; scope = scope.parent) {
      if (scope.bindings.has(node.name)) return scope.bindings.get(node.name);
    }
    return null;
  };
  const exportedArrays = new Map();
  const moduleExports = new Map();
  walk(ast, node => {
    if (node.type !== 'CallExpression' || node.callee.type !== 'MemberExpression' ||
        key(node.callee.property) !== 'd' || node.arguments[1]?.type !== 'ObjectExpression') return;
    for (const exported of node.arguments[1].properties) {
      let value = exported.value?.body;
      if (value?.type === 'BlockStatement' && value.body.length === 1) value = value.body[0].argument;
      if (value?.type === 'Identifier') value = binding(value);
      let owner = node;
      while (owner && !(owner.type === 'Property' && typeof key(owner.key) === 'number' && /Function/.test(owner.value?.type ?? ''))) owner = parents.get(owner);
      if (owner) {
        const id = key(owner.key);
        if (!moduleExports.has(id)) moduleExports.set(id, new Map());
        moduleExports.get(id).set(key(exported.key), value);
      }
      if (value?.type !== 'ArrayExpression') continue;
      const name = key(exported.key);
      if (!exportedArrays.has(name)) exportedArrays.set(name, new Set());
      exportedArrays.get(name).add(value);
    }
  });
  const exactExport = value => {
    if (value?.type !== 'MemberExpression' || value.object.type !== 'Identifier') return null;
    const imported = binding(value.object);
    const id = imported?.type === 'CallExpression' && imported.arguments[0]?.value;
    return typeof id === 'number' ? moduleExports.get(id)?.get(key(value.property)) : null;
  };
  const exportedArray = value => {
    const exact = exactExport(value);
    if (exact?.type === 'ArrayExpression') return exact;
    const candidates = value?.type === 'MemberExpression' && exportedArrays.get(key(value.property));
    return candidates?.size === 1 ? [...candidates][0] : null;
  };
  const nameOf = node => {
    if (node?.type === 'Literal') return {value: node.value, resolution: 'literal'};
    const exact = exactExport(node);
    if (exact?.type === 'Literal') return {value: exact.value, resolution: 'bound-webpack-export'};
    if (node?.type === 'Identifier' && binding(node)?.type === 'Literal') {
      return {value: binding(node).value, resolution: 'lexical-literal'};
    }
    if (node?.type === 'CallExpression' && node.arguments.length === 0 &&
        node.callee.type === 'MemberExpression' && key(node.callee.property) === 'toUpperCase') {
      const base = nameOf(node.callee.object);
      if (typeof base.value === 'string') return {value: base.value.toUpperCase(), resolution: base.resolution + '-uppercase'};
    }
    const values = node?.type === 'MemberExpression' && strings.get(key(node.property));
    if (values?.size === 1) return {value: [...values][0], resolution: 'unique-webpack-export'};
    return {value: null, resolution: 'unresolved', expression: snippet(node)};
  };
  const items = (array, resolveValue = () => null, seen = new Set()) => {
    if (seen.has(array)) return [];
    seen = new Set([...seen, array]);
    return array.elements.filter(Boolean).flatMap(item => {
      if (item.type === 'SpreadElement') {
        const expanded = resolveValue(item.argument) ?? exportedArray(item.argument);
        if (expanded) return items(expanded, resolveValue, seen).map(row => ({...row,
          expanded_from: {expression: snippet(item.argument), offset: item.start}}));
        return [{id: null, name: {value: null, resolution: 'unresolved-spread', expression: snippet(item.argument)},
          component: null, offset: item.start}];
      }
      return [{id: prop(item, 'id')?.value ?? null, name: nameOf(prop(item, 'name')),
        component: snippet(prop(item, 'component') ?? prop(item, 'renderComponent')),
        component_kind: prop(item, 'renderComponent') ? 'renderComponent' : 'component',
        extra_class: snippet(prop(item, 'extraClass')), offset: item.start}];
    });
  };
  const addLazy = (lazy, selector, branch, offset) => {
    const chunks = [], modules = [];
    walk(lazy, n => {
      if (n.type !== 'CallExpression' || n.callee.type !== 'MemberExpression') return;
      if (key(n.callee.property) === 'e' && typeof n.arguments[0]?.value === 'number') chunks.push(n.arguments[0].value);
      if (key(n.callee.property) === 'bind' && typeof n.arguments[1]?.value === 'number') modules.push(n.arguments[1].value);
    });
    lazyRoots.push({display_mode: branch, selector, offset,
      chunk_ids: [...new Set(chunks)], module_ids: [...new Set(modules)]});
  };
  walk(ast, node => {
    // Newer entrypoints mount displayMode branches through LazyRoot. Follow
    // only these explicit JSX children, not every lazy control in the bundle.
    if (node.type === 'CallExpression' && node.arguments[1]?.type === 'ObjectExpression') {
      const properties = node.arguments[1];
      const children = prop(properties, 'children');
      if (prop(properties, '$name') && children?.type === 'ObjectExpression') {
        for (const branch of children.properties) {
          const lazy = prop(branch.value?.arguments?.[1], 'lazy');
          if (!lazy) continue;
          addLazy(lazy, snippet(prop(properties, '$name')), key(branch.key) ?? snippet(branch.key), branch.start);
        }
      }
    }
    // Setup wrappers precede the actual product root. Some use a ternary
    // instead of the $name switch; only follow its explicit setupStatus gate.
    if (node.type === 'ReturnStatement' && node.argument?.type === 'ConditionalExpression' &&
        snippet(node.argument.test).includes('this.props.setupStatus')) {
      for (const branch of ['consequent', 'alternate']) {
        const expression = node.argument[branch];
        const lazy = prop(expression?.arguments?.[1], 'lazy');
        if (lazy) addLazy(lazy, snippet(node.argument.test), branch, expression.start);
      }
    }
    if (node.type === 'ArrayExpression' && node.elements.length && node.elements.every(
      item => item?.type === 'ObjectExpression' && prop(item, 'name') && prop(item, 'component'))) {
      navigationCandidates.push({offset: node.start, items: items(node)});
    }
    if (node.type === 'Property' && key(node.key) === 'className' &&
        node.value.type === 'Literal' && typeof node.value.value === 'string' &&
        /popup|modal|alert|dialog|tooltip/.test(node.value.value)) {
      overlayCandidates.push({class_name: node.value.value, offset: node.start});
    }
    if (!['ClassDeclaration', 'ClassExpression'].includes(node.type)) return;
    const constructor = node.body.body.find(method => method.kind === 'constructor');
    if (!constructor) return;
    const locals = new Map(), fields = new Map(), navs = [];
    ownWalk(constructor.value.body, n => {
      if (n.type === 'VariableDeclarator' && n.id.type === 'Identifier') locals.set(n.id.name, n.init);
      if (n.type === 'AssignmentExpression' && n.left.type === 'MemberExpression' && n.left.object.type === 'ThisExpression') fields.set(key(n.left.property), n.right);
      if (n.type === 'AssignmentExpression' && snippet(n.left) === 'this.state') {
        const value = prop(n.right, 'navs');
        if (value) navs.push(value);
      }
    });
    const resolveArray = (value, seen = new Set()) => {
      if (!value || seen.has(value)) return null;
      seen.add(value);
      if (value.type === 'ArrayExpression') return value;
      if (value.type === 'Identifier') return resolveArray(locals.get(value.name), seen);
      if (value.type === 'MemberExpression' && value.object.type === 'ThisExpression') return resolveArray(fields.get(key(value.property)), seen);
      if (value.type === 'CallExpression' && snippet(value.callee) === 'Array.from') return resolveArray(value.arguments[0], seen);
      if (value.type === 'CallExpression' && value.callee.type === 'MemberExpression' && value.callee.object.type === 'ThisExpression') {
        return resolveArray(fields.get(key(value.callee.property)), seen);
      }
      if (['ArrowFunctionExpression', 'FunctionExpression'].includes(value.type)) {
        if (value.body.type !== 'BlockStatement') return resolveArray(value.body, seen);
        const returns = value.body.body.filter(n => n.type === 'ReturnStatement');
        if (returns.length === 1) return resolveArray(returns[0].argument, seen);
      }
      return null;
    };
    for (const value of navs) {
      const array = resolveArray(value);
      if (array?.type !== 'ArrayExpression') continue;
      const methods = node.body.body.filter(method => method.type === 'MethodDefinition').map(m => key(m.key));
      routes.push({owner: node.id?.name ?? null, class_offset: node.start,
        navs_offset: array.start, state_offset: value.start, items: items(array, resolveArray), methods,
        evidence: 'Constructor assigns this.state.navs from this array, possibly via a local/this field or Array.from copy; component expressions preserved'});
    }
    // Other products keep navigation in renderNav instead of state.navs.
    // Require the same class's rendered JSX props to actually call that method.
    for (const method of node.body.body.filter(m => m.kind !== 'constructor')) {
      walk(method.value?.body, n => {
        if (n.type !== 'CallExpression' || n.arguments[1]?.type !== 'ObjectExpression') return;
        const value = prop(n.arguments[1], 'navs');
        if (value?.type !== 'CallExpression' || value.callee.object?.type !== 'ThisExpression') return;
        const array = resolveArray(value);
        if (!array || routes.some(row => row.navs_offset === array.start)) return;
        routes.push({owner: node.id?.name ?? null, class_offset: node.start, navs_offset: array.start,
          state_offset: value.start, items: items(array), methods: [key(method.key)],
          evidence: 'Rendered JSX navs property calls the same class constructor-bound method returning this array'});
      });
    }
  });
  const resolve = (value, fields = new Map(), seen = new Set()) => {
    if (!value || seen.has(value)) return null;
    seen.add(value);
    if (value.type === 'ArrayExpression') return value;
    if (value.type === 'Identifier') return resolve(binding(value), fields, seen);
    if (['ArrowFunctionExpression', 'FunctionExpression'].includes(value.type)) {
      if (value.body.type !== 'BlockStatement') return resolve(value.body, fields, seen);
      const returns = value.body.body.filter(n => n.type === 'ReturnStatement');
      if (returns.length === 1) return resolve(returns[0].argument, fields, seen);
    }
    if (value.type === 'CallExpression') {
      const callee = value.callee.type === 'SequenceExpression' ? value.callee.expressions.at(-1) : value.callee;
      if (callee.type === 'MemberExpression' && key(callee.property) === 'useMemo') return resolve(value.arguments[0], fields, seen);
      if (callee.type === 'MemberExpression' && callee.object.type === 'ThisExpression') return resolve(fields.get(key(callee.property)), fields, seen);
    }
    return null;
  };
  const addRoute = (array, value, owner, evidence, methods = []) => {
    if (!array || !array.elements.length || !array.elements.every(item =>
      item?.type === 'ObjectExpression' && prop(item, 'name') && prop(item, 'component')) ||
      routes.some(row => row.navs_offset === array.start)) return;
    routes.push({owner: owner?.id?.name ?? null, class_offset: owner?.start ?? null,
      navs_offset: array.start, state_offset: value.start, items: items(array), methods, evidence});
  };
  walk(ast, node => {
    // Functional roots pass a lexically bound useMemo array to navigation JSX.
    if (node.type === 'CallExpression' && node.arguments[1]?.type === 'ObjectExpression') {
      const value = prop(node.arguments[1], 'navs');
      if (value?.type === 'Identifier') addRoute(resolve(value), value, null,
        'Rendered JSX navs property resolves through lexical bindings/useMemo to this array');
    }
    if (node.type !== 'FunctionDeclaration' || !node.id) return;
    const parent = parents.get(node);
    if (parent?.type !== 'BlockStatement') return;
    const prototypes = [];
    // Babel _createClass(Constructor, [{key:render,value:function...}]).
    // Requiring this call avoids treating arbitrary object state as a class.
    ownWalk(parent, n => {
      if (n.type === 'CallExpression' && n.arguments[0]?.type === 'Identifier' &&
          n.arguments[0].name === node.id.name && n.arguments[1]?.type === 'ArrayExpression' &&
          n.arguments[1].elements.some(row => prop(row, 'key')?.value === 'render')) prototypes.push(...n.arguments[1].elements);
    });
    if (!prototypes.length) return;
    const fields = new Map(), states = [];
    ownWalk(node.body, n => {
      if (n.type !== 'AssignmentExpression' || n.left.type !== 'MemberExpression' || n.left.object.type !== 'Identifier') return;
      const alias = n.left.object.name;
      if (!fields.has(alias)) fields.set(alias, new Map());
      fields.get(alias).set(key(n.left.property), n.right);
      if (key(n.left.property) === 'state') states.push({alias, value: prop(n.right, 'navs')});
    });
    for (const {alias, value} of states) if (value) addRoute(resolve(value, fields.get(alias)), value, node,
      'Babel constructor writes instance.state.navs; paired _createClass contains render');
    for (const [alias, assigned] of fields) {
      // The object returned by the constructor is the one `this` refers to in
      // its prototype methods. Do not associate unrelated local objects.
      let returnsAlias = false;
      ownWalk(node.body, n => {
        if (n.type !== 'ReturnStatement') return;
        const last = n.argument?.type === 'SequenceExpression' ? n.argument.expressions.at(-1) : n.argument;
        if (last?.type === 'Identifier' && last.name === alias) returnsAlias = true;
      });
      if (!returnsAlias) continue;
      for (const method of prototypes.filter(row => prop(row, 'key')?.value === 'render')) {
        walk(prop(method, 'value')?.body, n => {
          if (n.type !== 'CallExpression' || n.arguments[1]?.type !== 'ObjectExpression') return;
          const value = prop(n.arguments[1], 'navs');
          if (value?.type !== 'CallExpression' || value.callee.object?.type !== 'ThisExpression') return;
          addRoute(resolve(value, assigned), value, node,
            'Babel prototype render passes this method result to JSX navs; method belongs to returned constructor instance', ['render']);
        });
      }
    }
  });
  const mounted = new Set(routes.map(row => row.navs_offset));
  return {path: file, sha256: crypto.createHash('sha256').update(source).digest('hex'),
    mounted_navigation: routes, unmounted_navigation_candidates: navigationCandidates.filter(row => !mounted.has(row.offset)),
    overlay_class_candidates: overlayCandidates, lazy_roots: lazyRoots};
}

function scanProduct(product) {
  // Export string slices can retain a bundle's complete source buffer. They
  // are useful only within this product's lazy-root graph, never across PIDs.
  stringMaps.clear();
  const pid = product.product_id;
  const files = [], missing = [];
  const references = product.asset_entrypoints.filter(ref => ref.endsWith('.js'));
  for (const reference of references) {
    const url = new URL(reference, product.endpoints['ui/index.html'].final_url);
    const prefix = `/synapse/products/${pid}/ui/`;
    if (!url.pathname.startsWith(prefix)) throw Error(`Unexpected entrypoint ${url}`);
    const relative = `.ref/devices/${pid}/${url.pathname.slice(prefix.length)}`;
    const filename = path.join(root, relative);
    if (!fs.existsSync(filename)) { missing.push(relative); continue; }
    const source = fs.readFileSync(filename, 'utf8');
    try {
      const prior = cached.get(relative);
      const hash = crypto.createHash('sha256').update(source).digest('hex');
      files.push(!refresh && !selected.length && productsWithNavigation.has(pid) && prior?.sha256 === hash ? prior : inspect(source, relative));
    }
    catch (error) { files.push({path: relative, parse_error: error.message}); }
  }
  // Iterate the growing queue: displayMode -> setupStatus -> actual root may
  // span several chunks. Path deduplication makes cyclic references finite.
  for (const entry of files) {
    for (const branch of entry.lazy_roots ?? []) {
      for (const resource of product.resource_entries) {
        const url = new URL(resource.url);
        const chunkId = Number(path.basename(url.pathname).split('.')[0]);
        if (!url.pathname.endsWith('.js') || !branch.chunk_ids.includes(chunkId)) continue;
        const prefix = `/synapse/products/${pid}/ui/`;
        if (!url.pathname.startsWith(prefix)) throw Error(`Unexpected chunk ${url}`);
        const relative = `.ref/devices/${pid}/${url.pathname.slice(prefix.length)}`;
        if (files.some(file => file.path === relative)) continue;
        const filename = path.join(root, relative);
        if (!fs.existsSync(filename)) { missing.push(relative); continue; }
        const source = fs.readFileSync(filename, 'utf8');
        if (!branch.module_ids.some(id => new RegExp(`(?:^|[,{])${id}:`).test(source))) continue;
        try {
          const parentStrings = stringMaps.get(entry.path) ?? exportedStrings(acorn.parse(fs.readFileSync(path.join(root, entry.path), 'utf8'), {ecmaVersion: 'latest'}));
          const file = inspect(source, relative, parentStrings);
          file.reachable_from = {path: entry.path, ...branch};
          files.push(file);
        } catch (error) { files.push({path: relative, parse_error: error.message}); }
      }
    }
  }
  stringMaps.clear();
  return {product_id: pid, files, missing_entrypoints: missing};
}

function writeJsonAtomic(filename, value) {
  fs.writeFileSync(filename + '.tmp', JSON.stringify(value, null, 2) + '\n');
  fs.renameSync(filename + '.tmp', filename);
}

function checkpointPath(pid) {
  return path.join(checkpointDir, `${pid}.json`);
}

function readCheckpoint(product) {
  const filename = checkpointPath(product.product_id);
  if (!fs.existsSync(filename)) return null;
  const checkpoint = JSON.parse(fs.readFileSync(filename, 'utf8'));
  if (checkpoint.scanner_sha256 !== scannerHash || checkpoint.acorn_version !== acorn.version ||
      checkpoint.catalog_product_sha256 !== sha256(JSON.stringify(product))) return null;
  const record = checkpoint.product;
  if (record.product_id !== product.product_id || record.missing_entrypoints.length ||
      !record.files.length || record.files.some(file => file.parse_error || !file.sha256)) return null;
  for (const file of record.files) {
    const filename = path.join(root, file.path);
    if (!fs.existsSync(filename) || sha256(fs.readFileSync(filename)) !== file.sha256) return null;
  }
  return record;
}

const products = catalog.products.filter(product => product.ui_exists &&
  (!selected.length || selected.includes(product.product_id)));
fs.mkdirSync(checkpointDir, {recursive: true});
if (worker) {
  if (products.length !== 1) throw Error(`No UI entrypoint for product ${selected[0]}`);
  const product = products[0];
  writeJsonAtomic(checkpointPath(product.product_id), {
    scanner_sha256: scannerHash, acorn_version: acorn.version,
    catalog_product_sha256: sha256(JSON.stringify(product)),
    generated_at_utc: new Date().toISOString(), product: scanProduct(product),
  });
  process.exit(0);
}

const results = [];
let resumed = 0;
for (const product of products) {
  // --refresh bypasses old aggregate results, but a checkpoint produced by
  // this exact scanner/parser and unchanged inputs can resume an interrupted
  // refresh. Each AST process exits before the next product starts.
  let record = readCheckpoint(product);
  if (record) {
    resumed += 1;
  } else {
    console.log(`Parsing product ${product.product_id} (${results.length + 1}/${products.length})`);
    const child = spawnSync(process.execPath,
      ['--max-old-space-size=4096', __filename, '--worker', ...(refresh ? ['--refresh'] : []), String(product.product_id)],
      {cwd: root, encoding: 'utf8', windowsHide: true, maxBuffer: 1024 * 1024});
    if (child.error || child.status !== 0) {
      throw Error(`Product ${product.product_id} parser failed; completed checkpoints are preserved.\n${child.error ?? child.stderr}`);
    }
    record = JSON.parse(fs.readFileSync(checkpointPath(product.product_id), 'utf8')).product;
  }
  results.push(record);
  if (results.length % 10 === 0) console.log(`Completed entrypoints: ${results.length} (${resumed} resumed)`);
}
const result = {schema_version: 1, scanner_sha256: scannerHash, acorn_version: acorn.version,
  generated_at_utc: new Date().toISOString(),
  scope: 'Static product entrypoint and explicit displayMode/setupStatus lazy-root AST. Navigation assignments and JSX bindings are evidence; overlay class names and other arrays are candidates, not reachability proof.',
  products: results};
if (merge && selected.length) {
  const previous = JSON.parse(fs.readFileSync(cachePath, 'utf8'));
  const updated = new Map(results.map(product => [product.product_id, product]));
  result.products = previous.products.map(product => updated.get(product.product_id) ?? product);
}
const target = path.join(root, '.ref/discovery', selected.length && !merge ? 'interface-sample.json' : 'interfaces.json');
writeJsonAtomic(target, result);
console.log(JSON.stringify({products: results.length, resumed, missing: results.filter(row => row.missing_entrypoints.length).length,
  with_navigation: results.filter(row => row.files.some(file => file.mounted_navigation?.length)).length,
  parse_errors: results.flatMap(row => row.files).filter(file => file.parse_error).length}));
