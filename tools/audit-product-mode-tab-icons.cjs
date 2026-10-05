// Current service-product favicon selectors and the mounted 164 pairing root.
// Acorn parses bytes as data; no downloaded JavaScript is imported or run.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {walk, key, hash} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const productIds = [164, 179, 241, 653, 740, 746, 769, 777, 778, 784, 3871, 3884, 3886, 3946];
const products = [];
let mountedPairing;
function receipt(file, text, node) {
  return {path: file, sha256: hash(text), offset: node.start, end: node.end,
    source: text.slice(node.start, node.end)};
}
for (const productId of productIds) {
  const directory = `.ref/devices/${productId}`, manifestFile = `${directory}/asset-manifest.json`;
  const manifestText = read(manifestFile), manifest = JSON.parse(manifestText);
  const files = [...new Set(Object.values(manifest.files))]
    .filter(file => /static\/js\/[^/]+\.js$/.test(file))
    .map(file => `${directory}/${file.slice(file.indexOf('static/js/'))}`).sort();
  const selectors = [], fingerprints = [];
  for (const file of files) {
    const text = read(file);
    fingerprints.push([file, hash(text)]);
    // This prefilter only limits the literal-selector audit. Computed selectors
    // and alternate roots without favicon DOM access are not inferred from it.
    if (!/querySelector(?:All)?\([^)]{0,120}icon/.test(text)) continue;
    const ast = acorn.parse(text, {ecmaVersion: 'latest'}), parents = new Map();
    walk(ast, node => {
      for (const value of Object.values(node)) {
        for (const child of Array.isArray(value) ? value : [value]) {
          if (child?.type) parents.set(child, node);
        }
      }
    });
    const matching = (scope, predicate) => {
      const found = []; walk(scope, node => { if (predicate(node)) found.push(node); });
      return found;
    };
    const one = (scope, predicate) => {
      const found = matching(scope, predicate);
      if (found.length !== 1) throw Error(`Ambiguous product ${productId}: ${found.length}`);
      return found[0];
    };
    walk(ast, node => {
      if (node.type !== 'CallExpression' || !['querySelector', 'querySelectorAll'].includes(key(node.callee.property))) return;
      const selector = node.arguments[0]?.value;
      if (typeof selector !== 'string' || !selector.includes('link') || !selector.includes('icon')) return;
      let owner = parents.get(node);
      while (owner && !/Function/.test(owner.type)) owner = parents.get(owner);
      if (!owner) throw Error('Unscoped favicon selector');
      const entry = {selector, query_offset: node.start, ...receipt(file, text, owner)};
      entry.behavior = entry.source.includes('readAsDataURL') ? 'preserve-image-bytes-as-data-url' : 'runtime-icon-selection';
      selectors.push(entry);
      if (entry.behavior !== 'runtime-icon-selection') return;
      if (productId !== 164) throw Error(`New service-product override requires mount audit: ${productId}`);
      let component = owner;
      while (component && component.type !== 'ClassDeclaration') component = parents.get(component);
      if (!component) throw Error('164 override is no longer class-owned');
      let scope = parents.get(component);
      while (scope && !/Function/.test(scope.type)) scope = parents.get(scope);
      const refs = matching(scope, n => n.type === 'Identifier' && n.name === component.id.name && n !== component.id);
      if (refs.length !== 1) throw Error('164 pairing root reference count changed');
      const connect = parents.get(refs[0]), wrapper = parents.get(connect);
      if (connect.type !== 'CallExpression' || wrapper.type !== 'VariableDeclarator') throw Error('164 connect is no longer retained');
      const mount = one(scope, n => n.type === 'CallExpression' && n.arguments[0]?.name === wrapper.id.name);
      const branch = parents.get(mount);
      if (branch.type !== 'ConditionalExpression' || !text.slice(branch.test.start, branch.test.end).includes('"multiDevicePairing"')) throw Error('164 pairing mount gate changed');
      let render = branch;
      while (render && !/Function/.test(render.type)) render = parents.get(render);
      if (!text.slice(render.start, render.end).includes('searchParams.get("displayMode")')) throw Error('164 mode parameter changed');
      const iconCall = parents.get(owner), assetName = iconCall.arguments[0]?.name;
      const asset = one(scope, n => n.type === 'VariableDeclarator' && n.id.name === assetName);
      if (!text.slice(asset.start, asset.end).includes('static/media/hyperpolling_icon.b7c3d035.svg')) throw Error('164 pairing icon changed');
      mountedPairing = {
        product_id: productId,
        component: receipt(file, text, component),
        connected_wrapper: receipt(file, text, wrapper),
        mount: receipt(file, text, mount),
        mode_selection: receipt(file, text, render),
        asset: receipt(file, text, asset),
        conclusion: 'Only displayMode=multiDevicePairing mounts the HyperPolling favicon override. The normal product root retains its HTML ACCESSORY icon.',
      };
    });
  }
  products.push({product_id: productId, manifest: manifestFile, manifest_sha256: hash(manifestText),
    script_count: files.length, scripts_sha256: hash(JSON.stringify(fingerprints)), selectors});
}
if (!mountedPairing) throw Error('164 pairing override was not found');
const evidence = {
  generator_sha256: hash(fs.readFileSync(__filename)),
  method: 'Current manifest-declared JS hashes, Acorn literal DOM-selector ownership and explicit 164 displayMode mount; no reference execution',
  products, mounted_pairing: mountedPairing,
  native_boundary: 'The local normal device tabs use HTML favicons. The explicitly requested independent pairing window has no native TabUI favicon slot; its OS-window policy remains an existing documented exception.',
  limitations: ['This receipt covers the 14 listed service products, not all 331 product mappings.',
    'Computed DOM selectors, browser favicon event order and hardware-service-driven modes are not runtime-verified.'],
};
const target = path.join(root, 'docs/re/product-mode-tab-icons-current-evidence.json');
const output = JSON.stringify(evidence, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (fs.readFileSync(target, 'utf8') !== output) throw Error('Stale product-mode favicon receipt');
} else fs.writeFileSync(target, output);
console.log(`Verified ${products.length} current service-product scopes and the mounted 164 pairing favicon override.`);
