// Static inventory only: downloaded host code is read and parsed, never loaded.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const base = '.ref/host-4.0.827/';
const output = 'docs/re/host-architecture-current-evidence.json';
const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const walk = (node, fn) => {
  if (!node || typeof node !== 'object') return;
  if (typeof node.type === 'string') fn(node);
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) value.forEach(child => walk(child, fn));
    else if (value && typeof value === 'object') walk(value, fn);
  }
};
const list = directory => fs.readdirSync(path.join(root, directory), {withFileTypes: true})
  .sort((a, b) => a.name.localeCompare(b.name, 'en'))
  .flatMap(entry => entry.isDirectory() ? list(`${directory}/${entry.name}`) : [`${directory}/${entry.name}`]);
const allJsFiles = list(`${base}electron`).filter(file => /\.(?:c?js)$/.test(file));
const files = allJsFiles.filter(file => !file.includes('/assets/js/') && !file.includes('/utils/jsonEditor/'));
const excludedFiles = allJsFiles.filter(file => !files.includes(file)).map(file => {
  const bytes = fs.readFileSync(path.join(root, file));
  return {path: file, sha256: sha256(bytes), bytes: bytes.length, reason: 'Bundled third-party browser library/editor; not first-party host chain'};
});
const packageBytes = fs.readFileSync(path.join(root, base, 'package.json'));
const packageData = JSON.parse(packageBytes.toString('utf8'));
if (packageData.version !== '4.0.827' || packageData.main !== 'electron/main.js') throw Error('Current host identity changed');
const inventories = files.map(file => {
  const bytes = fs.readFileSync(path.join(root, file));
  const source = bytes.toString('utf8');
  let ast, sourceType = 'script';
  try { ast = acorn.parse(source, {ecmaVersion: 'latest', sourceType}); }
  catch { sourceType = 'module'; ast = acorn.parse(source, {ecmaVersion: 'latest', sourceType}); }
  const text = node => node ? source.slice(node.start, node.end) : null;
  const receipt = node => ({start: node.start, end: node.end, source: text(node)});
  const imports = [], dynamicImports = [], functions = [], exports = [], listeners = [], ipcHandlers = [], bridges = [];
  let allFunctionNodes = 0;
  walk(ast, node => {
    if (['ArrowFunctionExpression', 'FunctionExpression', 'FunctionDeclaration'].includes(node.type)) allFunctionNodes++;
    if (node.type === 'CallExpression' && node.callee.name === 'require') {
      if (typeof node.arguments[0]?.value === 'string') {
        const moduleName = node.arguments[0].value;
        const item = {module: moduleName, ...receipt(node)};
        if (moduleName.startsWith('.')) {
          const target = path.posix.normalize(path.posix.join(path.posix.dirname(file), moduleName));
          const candidates = [target, `${target}.js`, `${target}.cjs`, `${target}/index.js`];
          item.resolved = candidates.find(candidate => fs.existsSync(path.join(root, candidate)) && fs.statSync(path.join(root, candidate)).isFile()) ?? null;
          item.boundary = item.resolved ? 'current-host-file' : 'relative-target-not-extracted';
        } else item.boundary = 'node-builtin-or-package-implementation';
        imports.push(item);
      } else dynamicImports.push({kind: 'nonliteral-require', ...receipt(node)});
    }
    if (node.type === 'ImportExpression') dynamicImports.push({kind: 'import-expression', ...receipt(node)});
    if (node.type === 'FunctionDeclaration') functions.push({name: node.id?.name ?? '<anonymous>', kind: node.type, start: node.start, end: node.end});
    if (['PropertyDefinition', 'MethodDefinition'].includes(node.type) && /Function/.test(node.value?.type ?? '')) functions.push({name: text(node.key), kind: node.type, start: node.start, end: node.end});
    if (node.type === 'VariableDeclarator' && /Function/.test(node.init?.type ?? '')) functions.push({name: text(node.id), kind: node.init.type, start: node.start, end: node.end});
    if (node.type === 'AssignmentExpression' && /Function/.test(node.right?.type ?? '')) functions.push({name: text(node.left), kind: node.right.type, start: node.right.start, end: node.right.end});
    if (node.type === 'Property' && /Function/.test(node.value?.type ?? '')) functions.push({name: text(node.key), kind: node.value.type, start: node.value.start, end: node.value.end});
    if (node.type === 'AssignmentExpression' && /(?:^module\.exports|^exports\.)/.test(text(node.left))) exports.push({name: text(node.left), ...receipt(node)});
    if (node.type !== 'CallExpression' || node.callee.type !== 'MemberExpression') return;
    const method = node.callee.property.name ?? node.callee.property.value;
    const channel = node.arguments[0];
    if (['on', 'once', 'handle', 'exposeInMainWorld'].includes(method) && typeof channel?.value === 'string') {
      const item = {receiver: text(node.callee.object), method, channel: channel.value, start: node.start, end: node.end};
      listeners.push(item);
      if (method === 'handle') {
        const switches = [];
        walk(node.arguments[1], child => {
          if (child.type !== 'SwitchStatement') return;
          let pending = [];
          for (const branch of child.cases) {
            pending.push(branch.test ? text(branch.test) : '<default>');
            if (!branch.consequent.length) continue;
            switches.push({discriminant: text(child.discriminant), cases: pending, ...receipt(branch)});
            pending = [];
          }
          if (pending.length) switches.push({discriminant: text(child.discriminant), cases: pending, source: '', start: child.end, end: child.end});
        });
        ipcHandlers.push({...item, source: text(node), actionGroups: switches});
      }
      if (method === 'exposeInMainWorld') bridges.push({...item, source: text(node)});
    }
  });
  return {path: file, sha256: sha256(bytes), bytes: bytes.length, sourceType, allFunctionNodes, imports, dynamicImports, functions, exports, listeners, ipcHandlers, bridges};
});
const sourceFile = name => inventories.find(file => file.path === `${base}electron/${name}`);
const anchorSpecs = [
  ['startup-ready', 'main.js', 'commonDLLDir:'],
  ['host-quit-app', 'main.js', 'quitApp()'],
  ['host-quit-final', 'main.js', 'quitAppFinal()'],
  ['host-tray-menu', 'main.js', 'createSystrayMenuRight()'],
  ['host-launch-apps', 'main.js', 'launchRazerApps(appsArray:'],
  ['host-login-command', 'main.js', 'logIn'],
  ['native-service-status', 'serviceFunction.js', 'getServiceStatus'],
  ['native-version-upgrade', 'mainSubFunction.js', 'upgradeAppEngineVersion'],
];
const anchors = anchorSpecs.map(([id, file, needle]) => {
  const inventory = sourceFile(file), source = fs.readFileSync(path.join(root, inventory.path), 'utf8');
  const ast = acorn.parse(source, {ecmaVersion: 'latest'}), candidates = [];
  walk(ast, node => {
    if (['FunctionDeclaration', 'ArrowFunctionExpression', 'FunctionExpression'].includes(node.type)
      && source.slice(node.start, node.end).includes(needle)) candidates.push(node);
  });
  const selected = candidates.sort((a,b) => (a.end-a.start)-(b.end-b.start))[0];
  if (!selected) throw Error(`Missing source anchor: ${id}`);
  return {id, path: inventory.path, sha256: inventory.sha256, start: selected.start, end: selected.end, source: source.slice(selected.start, selected.end)};
});
const evidence = {
  schema: 1,
  scope: 'Current host first-party JavaScript architecture inventory; static syntax and source chains only',
  source: {path: `${base}package.json`, sha256: sha256(packageBytes), version: packageData.version, entry: packageData.main, dependencies: packageData.dependencies},
  exclusions: ['electron/assets/js third-party browser libraries', 'DebugWindow/utils/jsonEditor third-party editor', 'binary internals, node_modules implementations, runtime verification'],
  excludedFiles,
  counts: {allJsFiles: allJsFiles.length, excludedThirdParty: excludedFiles.length, files: inventories.length, bytes: inventories.reduce((sum, file) => sum+file.bytes, 0), allFunctionNodes: inventories.reduce((sum,file) => sum+file.allFunctionNodes,0), ipcHandlers: inventories.reduce((sum,file) => sum+file.ipcHandlers.length,0), imports: inventories.reduce((sum,file) => sum+file.imports.length,0)},
  anchors,
  files: inventories,
};
const rendered = `${JSON.stringify(evidence, null, 2)}\n`;
if (process.argv.includes('--check')) {
  if (fs.readFileSync(path.join(root, output), 'utf8').replace(/\r\n/g, '\n') !== rendered) throw Error('Host architecture evidence is stale');
  console.log(`Current host architecture evidence verified: ${inventories.length} files, ${evidence.counts.ipcHandlers} IPC handlers`);
} else {
  fs.writeFileSync(path.join(root, output), rendered);
  console.log(`Wrote ${output}: ${inventories.length} files, ${evidence.counts.ipcHandlers} IPC handlers`);
}
