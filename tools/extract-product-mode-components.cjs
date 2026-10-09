// Static root-to-page audit. Acorn parses downloaded bundles as data only.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const sha = text => crypto.createHash('sha256').update(text).digest('hex');
const audit = JSON.parse(read('docs/re/display-mode-audit.json'));
const pages = JSON.parse(read('docs/re/audio-product-pages.json')).products;
const args = process.argv.slice(2).filter(arg => arg !== '--check').map(Number);
const walk = (node, fn) => {
  if (!node?.type || fn(node) === false) return;
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) { for (const child of value) if (child?.type) walk(child, fn); }
    else if (value?.type) walk(value, fn);
  }
};
const key = node => node?.name ?? node?.value;
const results = [];
for (const product of audit.products) {
  const lighting = pages.find(page => page.product_id === product.product_id)?.pages
    .find(page => ['TAB_LIGHTING', 'LIGHTING'].includes(page.key));
  if (!lighting || !product.modes.some(mode => mode.mode === 'chromaApp' && mode.kind === 'root')) continue;
  if (args.length && !args.includes(product.product_id)) continue;
  const source = read(product.source);
  if (sha(source) !== product.sha256 || lighting.path !== product.source || lighting.sha256 !== product.sha256)
    throw Error(`Source receipt mismatch: ${product.product_id}`);
  const ast = acorn.parse(source, { ecmaVersion: 'latest' });
  const scopes = new WeakMap(), conditionals = [];
  function visit(node, scope) {
    if (!node?.type) return;
    if (['ClassDeclaration', 'FunctionDeclaration'].includes(node.type)) scope?.defs.set(node.id.name, node);
    if (['Program', 'FunctionDeclaration', 'FunctionExpression', 'ArrowFunctionExpression'].includes(node.type)) {
      scope = { parent: scope, defs: new Map() };
      for (const parameter of node.params ?? []) if (parameter.type === 'Identifier') scope.defs.set(parameter.name, null);
    }
    scopes.set(node, scope);
    if (node.type === 'VariableDeclarator' && node.id.type === 'Identifier') scope.defs.set(node.id.name, node.init);
    if (node.type === 'ConditionalExpression') conditionals.push(node);
    for (const value of Object.values(node)) {
      if (Array.isArray(value)) { for (const child of value) if (child?.type) visit(child, scope); }
      else if (value?.type) visit(value, scope);
    }
  }
  visit(ast, null);
  function resolve(node, seen = new Set()) {
    if (!node || seen.has(node)) return node;
    seen.add(node);
    if (node.type === 'Identifier') {
      let scope = scopes.get(node);
      while (scope) {
        if (scope.defs.has(node.name)) return resolve(scope.defs.get(node.name), seen) ?? node;
        scope = scope.parent;
      }
    }
    return node;
  }
  const lightingSymbol = /\.jsx\)\(([^,]+),/.exec(lighting.component)?.[1];
  const branch = product.modes.find(mode => mode.mode === 'chromaApp' && mode.kind === 'root');
  const conditional = conditionals.find(node => node.test.start <= branch.offset && node.test.end > branch.offset);
  if (!conditional) throw Error(`Missing current root ${product.product_id}`);
  const seen = new Set(), components = [], matched = [];
  function trace(node, depth = 0) {
    if (!node || depth > 6) return;
    if (node.type === 'Identifier' && node.name === lightingSymbol) {
      matched.push({ symbol: node.name, offset: node.start });
      return;
    }
    const symbol = node.type === 'Identifier' ? node.name : null;
    node = resolve(node);
    if (!node || seen.has(node)) return;
    seen.add(node);
    if (node.type === 'CallExpression') {
      for (const arg of node.arguments) if (['Identifier', 'ClassExpression', 'FunctionExpression'].includes(arg.type)) trace(arg, depth + 1);
      return;
    }
    let render = node;
    if (['ClassDeclaration', 'ClassExpression'].includes(node.type))
      render = node.body.body.find(member => key(member.key) === 'render')?.value;
    if (!render || !/Function/.test(render.type)) return;
    components.push({ symbol, offset: node.start, end: node.end,
      render_offset: render.start, render_end: render.end, render_source: source.slice(render.start, render.end) });
    walk(render, child => {
      if (child.type === 'CallExpression' && child.callee.type === 'SequenceExpression'
        && ['jsx', 'jsxs'].includes(key(child.callee.expressions.at(-1)?.property))) {
        const target = child.arguments[0];
        if (target.type !== 'Literal') trace(target, depth + 1);
      }
    });
  }
  const rootCall = conditional.consequent;
  trace(rootCall.arguments?.[0]);
  const cssDirectory = path.posix.dirname(product.source).replace(/\/js$/, '/css');
  const cssFile = fs.readdirSync(path.join(root, cssDirectory)).find(file => /^main\..*\.css$/.test(file));
  if (!cssFile) throw Error(`Missing current main CSS ${product.product_id}`);
  const cssPath = `${cssDirectory}/${cssFile}`, cssSource = read(cssPath);
  const wrapperRule = /(?:^|\})\.body-wrapper\{([^}]+)\}/.exec(cssSource)?.[1];
  if (!wrapperRule?.includes('min-width:600px') || !wrapperRule.includes('padding:10px 20px 20px'))
    throw Error(`Unresolved body-wrapper geometry ${product.product_id}`);
  const bodyMinWidth = components.some(component =>
    /\.body-wrapper,\s*\.main-container\{\s*min-width:unset/.test(component.render_source)) ? 0 : 600;
  results.push({ product_id: product.product_id, name: product.name, source: product.source, sha256: product.sha256,
    root_condition: source.slice(conditional.test.start, conditional.test.end), root_offset: conditional.start,
    root_component: source.slice(rootCall.arguments[0].start, rootCall.arguments[0].end),
    normal_lighting: { key: lighting.key, symbol: lightingSymbol, nav_offset: lighting.nav_offset },
    body_min_width: bodyMinWidth,
    css: { path: cssPath, sha256: sha(cssSource), body_wrapper: wrapperRule },
    mounts_normal_lighting: matched.length > 0, matched, components });
  console.log(`${product.product_id}: ${matched.length ? 'same lighting component' : 'REQUIRES SEPARATE AUDIT'}`);
}
const output = 'docs/re/product-mode-current-components.json';
const text = JSON.stringify({ schema_version: 1, scanner_sha256: sha(read('tools/extract-product-mode-components.cjs')),
  method: 'Lexically resolved Acorn AST. Compare each current chromaApp renderer with its own registered Lighting component; downloaded JavaScript is never evaluated.',
  products: results }, null, 2) + '\n';
const routesOutput = 'crates/razer-pages/src/features/audio_chroma_modes.json';
const routesText = JSON.stringify(results.filter(product => product.mounts_normal_lighting)
  .map(product => ({ product_id: product.product_id, page: product.normal_lighting.key,
    body_min_width: product.body_min_width })), null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (read(output) !== text) throw Error(`${output} is stale`);
  if (read(routesOutput) !== routesText) throw Error(`${routesOutput} is stale`);
} else {
  fs.writeFileSync(path.join(root, output), text);
  fs.writeFileSync(path.join(root, routesOutput), routesText);
}
