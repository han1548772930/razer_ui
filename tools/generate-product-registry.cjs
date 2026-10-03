// Static registry generation. Downloaded bundles are parsed as data, never run.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file));
const digest = data => crypto.createHash('sha256').update(data).digest('hex');
const catalogPath = 'docs/re/product-catalog.json';
const catalogBytes = read(catalogPath);
const catalog = JSON.parse(catalogBytes);
const pending = JSON.parse(read('docs/re/unimplemented-products.json'));
const inventory = JSON.parse(read('.ref/discovery/interfaces.json'));
if (digest(read('tools/inventory-razer-interfaces.cjs')) !== inventory.scanner_sha256)
  throw Error('Maintained inventory scanner changed; regenerate its source evidence first');
const check = process.argv.includes('--check');
const inspectIds = process.argv.find(arg => arg.startsWith('--inspect='))?.slice(10).split(',').map(Number);
const sourceFiles = new Map(inventory.products.flatMap(product => product.files.map(file => [file.path, file])));
const compiled = new Map(pending.compiled_products.map(product => [product.product_id, product]));
const forbidden = ['.ref/frontend/', '.ref/synapse-asar/', '.ref/host-4.0.821/', '.work/latest-source-check/host-4.0.821/'];
const assert = (condition, message) => { if (!condition) throw Error(message); };
const key = node => node?.name ?? node?.value;
const prop = (node, name) => node?.properties?.find(p => p.type === 'Property' && key(p.key) === name)?.value;
const expr = (source, offset) => {
  // Read one initializer without consuming the following sibling property or
  // declarator. parseExpressionAt would treat that comma as a JS sequence.
  const parser = new acorn.Parser({ecmaVersion: 'latest'}, source, offset);
  parser.nextToken();
  return parser.parseMaybeAssign();
};
const escape = value => value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
function walk(node, visit) {
  if (!node?.type || visit(node) === false) return;
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) { for (const child of value) if (child?.type) walk(child, visit); }
    else if (value?.type) walk(value, visit);
  }
}
function isJsx(node) {
  if (node?.type !== 'CallExpression') return false;
  let callee = node.callee;
  if (callee.type === 'SequenceExpression') callee = callee.expressions.at(-1);
  return callee.type === 'MemberExpression' && ['jsx', 'jsxs', 'createElement'].includes(key(callee.property));
}
function sourceReader() {
  const cache = new Map();
  return file => {
    assert(!forbidden.some(prefix => file.startsWith(prefix)), `Obsolete source: ${file}`);
    if (!cache.has(file)) {
      const bytes = read(file), sha256 = digest(bytes);
      const metadata = sourceFiles.get(file);
      assert(metadata?.sha256 === sha256, `Inventory source hash mismatch: ${file}`);
      cache.set(file, {source: bytes.toString('utf8'), sha256});
    }
    return cache.get(file);
  };
}

// Find the Redux/HOC alias immediately following this exact class. Restrict the
// search to its following declarations; minified names in other modules differ.
function ownerAliases(source, navigation) {
  let start = navigation.owner_offset;
  let owner = navigation.owner, end;
  if (start == null) {
    const from = Math.max(0, navigation.offset - 40000);
    const prefix = source.slice(from, navigation.offset);
    for (const match of [...prefix.matchAll(/(?:\b(?:const|let|var)\s+|,)([\w$]+)=(?![=>])/g)].reverse()) {
      const initStart = from + match.index + match[0].indexOf(match[1] + '=') + match[1].length + 1;
      try {
        const init = expr(source, initStart);
        if (init.end > navigation.usage_offset) { owner = match[1]; start = initStart; end = init.end; break; }
      } catch {}
    }
  }
  else if (source.startsWith('class ', start)) end = expr(source, start).end;
  else {
    const prefix = source.slice(Math.max(0, start - 250), start);
    const match = [...prefix.matchAll(/([\w$]+)=function\([^)]*\)\{/g)].at(-1);
    if (match) {
      owner = match[1];
      const init = start - prefix.length + match.index + match[1].length + 1;
      end = expr(source, init).end;
      // Babel IIFE invokes the class factory with React.Component.
      if (source[end] === '(') end = expr(source, init).end;
    } else end = start;
  }
  const aliases = new Set([owner]);
  const transitions = [];
  const segment = source.slice(end);
  for (const match of segment.matchAll(/\bclass\s+([\w$]+)\s+extends|(?:\b(?:const|let|var)\s+|,)([\w$]+)=(?![=>])|\bfunction\s+([\w$]+)\(/g)) {
      const name = match[1] || match[2] || match[3];
      const initStart = match[1] || match[3] ? end + match.index : end + match.index + match[0].indexOf(name + '=') + name.length + 1;
      try {
        const nearby = source.slice(initStart, initStart + 12000);
        if (![...aliases].some(alias => nearby.includes(`(${alias},`) || nearby.includes(`(${alias})`) || nearby.includes(`=${alias},`) || nearby.startsWith(`${alias},`) || nearby.startsWith(`${alias};`))) continue;
        let node = expr(source, initStart);
        if (node.type === 'SequenceExpression') node = node.expressions[0];
        const mounts = [];
        const localAliases = new Set(aliases);
        walk(node, child => {
          if (child.type === 'AssignmentExpression' && child.left.type === 'Identifier' && child.right.type === 'Identifier' && localAliases.has(child.right.name)) localAliases.add(child.left.name);
        });
        if (node.type === 'Identifier' && aliases.has(node.name)) mounts.push(node);
        else if (node.type === 'CallExpression' && node.arguments.some(function hasOwner(arg) {
          return (arg.type === 'Identifier' && aliases.has(arg.name)) || (arg.type === 'CallExpression' && arg.arguments.some(hasOwner));
        })) mounts.push(node);
        else walk(node, child => {
          if (isJsx(child) && child.arguments[0]?.type === 'Identifier' && localAliases.has(child.arguments[0].name)) mounts.push(child);
        });
        // A displayMode selector is a boundary, not an interchangeable wrapper:
        // propagating through it would collapse pairing into the default root.
        if (source.slice(node.start, node.end).includes('get("displayMode")')) continue;
        if (mounts.length) {
          aliases.add(name);
          transitions.push({name, offset: initStart, mounts: mounts.map(node => source.slice(node.start, node.end))});
        }
      } catch { /* A nested assignment is not a top-level HOC declarator. */ }
  }
  return {names: [...aliases], transitions};
}

function rootReturns(source) {
  const reads = [...source.matchAll(/\.get\("displayMode"\)/g)];
  const roots = [];
  // Application roots follow feature helpers in these generated entries.
  for (const read of reads.slice(-3)) {
    const start = Math.max(0, read.index - 250);
    const tail = source.slice(start, read.index + 4500);
    for (const match of tail.matchAll(/\breturn\s*/g)) {
      const offset = start + match.index + match[0].length;
      try {
        const node = acorn.parseExpressionAt(source, offset, {ecmaVersion: 'latest'});
        const text = source.slice(node.start, node.end);
        if (!text.includes('.jsx') && !text.includes('createElement')) continue;
        roots.push({offset, node, text});
      } catch { /* A bare return or statement is not a route expression. */ }
    }
  }
  return roots.reverse();
}

function directRoot(source) {
  const matches = [...source.matchAll(/createRoot\)?\(.{1,180}?\)\.render\(/g)];
  const match = matches.at(-1);
  if (!match) return null;
  const offset = match.index + match[0].length;
  const node = expr(source, offset);
  return {offset, node, text: source.slice(node.start, node.end)};
}

function branchMount(node, aliases, source, condition = 'default', conditions = []) {
  if (node.type === 'ConditionalExpression') {
    const test = source.slice(node.test.start, node.test.end);
    // A negative guard (e.g. chromaApp !== mode) still leads to the default
    // workspace. Preserve the full predicate separately; it is not a URL mode.
    const literal = /"([^"]+)"===/.exec(test)?.[1] ?? /==="([^"]+)"/.exec(test)?.[1] ?? condition;
    return [...branchMount(node.consequent, aliases, source, literal, [...conditions, test]), ...branchMount(node.alternate, aliases, source, condition, [...conditions, `!(${test})`])];
  }
  if (node.type === 'SequenceExpression') return branchMount(node.expressions.at(-1), aliases, source, condition, conditions);
  const found = [];
  walk(node, child => {
    if (isJsx(child) && child.arguments[0]?.type === 'Identifier' && aliases.includes(child.arguments[0].name))
      found.push({display_mode: condition, expression: source.slice(child.start, child.end), offset: child.start, conditions});
  });
  return found;
}

const rows = [], unresolved = [];
const allProducts = catalog.products.filter(product => product.ui_entry_found).sort((a, b) => a.product_id - b.product_id);
assert(allProducts.length === 331 && pending.products.length === 321, 'Product inventory scope changed; review counts');
assert(pending.products.every(row => allProducts.some(p => p.product_id === row.product_id)), 'Missing pending product');
const products = inspectIds ? allProducts.filter(product => inspectIds.includes(product.product_id)) : allProducts;
for (const product of products) {
  const getSource = sourceReader();
  const groups = [];
  for (const navigation of product.navigation) {
    const {source, sha256} = getSource(navigation.source);
    assert(sha256 === navigation.sha256, `Catalog source hash mismatch: ${navigation.source}`);
    let array = expr(source, navigation.offset);
    if (array.type === 'SequenceExpression') array = array.expressions[0];
    assert(array.type === 'ArrayExpression', `Expected navigation array: ${product.product_id}/${navigation.offset}`);
    const items = navigation.items.map(item => {
      let object = expr(source, item.offset);
      if (object.type === 'SequenceExpression') object = object.expressions[0];
      assert(object.type === 'ObjectExpression', `Expected page object: ${product.product_id}/${item.offset}`);
      const component = prop(object, item.component_kind);
      const actual = component ? source.slice(component.start, component.end) : null;
      assert(actual === item.component, `Component mismatch: ${product.product_id}/${item.offset}`);
      assert(item.name.value, `Unresolved page label: ${product.product_id}/${item.offset}`);
      return {...item, key: `product-${product.product_id}-${path.basename(navigation.source, '.js')}-${item.offset}`};
    });
    let mode, primary = false, reachability;
    if (navigation.reachable_from) {
      const chain = [];
      for (let branch = navigation.reachable_from; branch; branch = branch.parent) {
        const {source: parent, sha256: parentHash} = getSource(branch.path);
        let branchExpr;
        if (parent[branch.offset] === '(') branchExpr = expr(parent, branch.offset);
        else {
          // Switch children properties record the property start (e.g. default:),
          // while conditional lazy branches record the expression itself.
          const colon = parent.indexOf(':', branch.offset);
          assert(colon - branch.offset < 100, `Unknown lazy branch boundary: ${branch.path}`);
          branchExpr = expr(parent, colon + 1);
        }
        const text = parent.slice(branchExpr.start, branchExpr.end);
        assert(branch.module_ids.every(id => text.includes(String(id))), `Lazy module edge mismatch: ${product.product_id}`);
        chain.push({...branch, parent: undefined, sha256: parentHash, expression: text});
      }
      mode = chain.at(-1).display_mode;
      primary = mode === 'default';
      reachability = {kind: 'lazy_root_chain', chain};
    } else {
      const aliasEvidence = ownerAliases(source, navigation);
      const aliases = aliasEvidence.names;
      let mounts = [];
      for (const candidate of rootReturns(source)) {
        mounts = branchMount(candidate.node, aliases, source);
        if (mounts.length) { reachability = {kind: 'root_display_mode', selector_expression: candidate.text, aliases, alias_transitions: aliasEvidence.transitions, mounts}; break; }
      }
      if (!mounts.length && product.navigation.length === 1) {
        const candidate = directRoot(source);
        if (candidate) {
          mounts = branchMount(candidate.node, aliases, source);
          if (mounts.length) reachability = {kind: 'direct_react_root', root_expression: candidate.text, aliases, alias_transitions: aliasEvidence.transitions, mounts};
        }
      }
      if (mounts.length && new Set(mounts.map(mount => mount.display_mode)).size === 1) { mode = mounts[0].display_mode; primary = mode === 'default'; }
      else {
        mode = 'unresolved';
        reachability = {kind: 'unresolved_root_mount', aliases, mounted_navigation_evidence: navigation.evidence};
        unresolved.push({product_id: product.product_id, owner: navigation.owner, offset: navigation.offset, aliases});
      }
    }
    groups.push({...navigation, key: `product-${product.product_id}-${path.basename(navigation.source, '.js')}-${navigation.offset}`,
      display_mode: mode, primary, root_evidence: reachability, items});
  }
  rows.push({...product, navigation: groups});
  if (rows.length % 50 === 0) console.log(`Verified ${rows.length}/${products.length} product navigation sources`);
}
const audit = {schema_version: 1, audit_date: '2026-10-03', generator_sha256: digest(fs.readFileSync(__filename)),
  source_catalog: {path: catalogPath, sha256: digest(catalogBytes)}, inventory_scanner_sha256: inventory.scanner_sha256,
  scope: 'Source-backed product routes, not completed native product UIs or connected services.',
  summary: {registered_product_ids: rows.length, formerly_unregistered_product_ids: pending.products.length,
    navigation_groups: rows.reduce((sum, p) => sum + p.navigation.length, 0), pages: rows.reduce((sum, p) => sum + p.navigation.reduce((n, g) => n + g.items.length, 0), 0),
    primary_resolved_product_ids: rows.filter(p => p.navigation.some(g => g.primary)).length, unresolved_groups: unresolved.length},
  unresolved, products: rows.map(p => ({product_id: p.product_id, name: p.name, categories: p.categories, edition_ids: p.edition_ids,
    connection_aliases: p.connection_aliases, referenced_by: p.referenced_by, adapter_status: compiled.has(p.product_id) ? 'existing_partial_native_adapter' : 'source_audited_no_native_adapter_claim', navigation: p.navigation}))};

// JSON string escaping and Rust string escaping agree for these source strings,
// except control-character Unicode escapes, which are handled explicitly.
const rust = value => JSON.stringify(value).replace(/\\u([0-9a-f]{4})/gi, '\\u{$1}');
const optional = value => value == null ? 'None' : `Some(${rust(value)})`;
const literal = value => typeof value === 'string' ? rust(value) : String(value);
const output = ['// Generated by tools/generate-product-registry.cjs. Do not hand-edit.', '&['];
for (const p of rows) {
  assert(p.navigation.filter(g => g.primary).length <= 1, `Multiple primary roots: ${p.product_id}`);
  output.push(`RegisteredProduct { id: ${p.product_id}, name: ${rust(p.name)}, categories: &[${p.categories.map(rust).join(',')}], edition_ids: &[${p.edition_ids.join(',')}], navigations: &[`);
  for (const g of p.navigation) {
    output.push(`ProductNavigation { key: ${rust(g.key)}, owner: ${rust(g.owner ?? '')}, display_mode: ${rust(g.display_mode)}, source: ${rust(g.source)}, source_sha256: ${rust(g.sha256)}, offset: ${g.offset}, primary: ${g.primary}, reachability: ${rust(JSON.stringify(g.root_evidence))}, pages: &[`);
    for (const page of g.items) {
      const role = page.name.value === 'HELP' ? 'Help' : (g.display_mode === 'multiDevicePairing' ? 'StandaloneMode' : 'Page');
      const existing = g.primary && compiled.has(p.product_id);
      output.push(`ProductPage { id: ProductPageId { product_id: ${p.product_id}, key: ${rust(page.key)} }, kind: ProductPageKind(${rust(page.name.value)}), role: ProductPageRole::${role}, source_id: ${Number.isInteger(page.id) ? `Some(${page.id})` : 'None'}, component_expression: ${optional(page.component)}, component_kind: ${rust(page.component_kind)}, extra_class: ${optional(page.extra_class)}, offset: ${page.offset}, adapter_status: AdapterStatus::${existing ? 'ExistingAdapter' : 'SourceAudited'} },`);
    }
    output.push('] },');
  }
  output.push('] },');
}
output.push(']');
const files = new Map([
  ['src/product/registry_data.rs', output.join('\n') + '\n'],
  ['docs/re/product-registration-audit.json', JSON.stringify(audit, null, 2) + '\n'],
]);
if (inspectIds) {
  console.log(JSON.stringify(audit.summary));
  console.log(JSON.stringify(rows.map(p => ({product_id:p.product_id,navigation:p.navigation.map(g=>({owner:g.owner,primary:g.primary,mode:g.display_mode,evidence:g.root_evidence}))}))));
  process.exit(0);
}
for (const [file, content] of files) {
  if (check) assert(read(file).toString('utf8') === content, `Generated file stale: ${file}`);
  else fs.writeFileSync(path.join(root, file), content);
}
console.log(JSON.stringify(audit.summary));
if (unresolved.length) console.log('Unresolved roots:', JSON.stringify(unresolved));
