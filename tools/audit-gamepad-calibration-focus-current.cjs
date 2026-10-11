// Static AST receipts. Never import or execute the vendor bundles.
const fs = require('fs');
const acorn = require('acorn');
const crypto = require('crypto');
function walk(node, visit) {
  if (!node?.type) return;
  visit(node);
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) value.forEach(child => walk(child, visit));
    else if (value?.type) walk(value, visit);
  }
}
const files = [];
for (const [source, moduleId] of [
  ['local-ui-reverse/source/official/apps.razer.com/synapse/products/2676/ui/static/js/main.5ee115d4.js', 21107],
  ['local-ui-reverse/source/official/apps.razer.com/synapse/products/2684/ui/static/js/main.fbe5395b.js', 1107],
  ['local-ui-reverse/source/installed/app-4.0.827/electron/main.js', null],
]) {
  const bytes = fs.readFileSync(source);
  const raw = bytes.toString('utf8');
  const receipts = [];
  walk(acorn.parse(raw, { ecmaVersion: 'latest' }), node => {
    const helper = moduleId !== null && node.type === 'Property' && node.key.value === moduleId;
    const host = moduleId === null && node.type === 'SwitchCase'
      && ['getWindowStatus', 'getFocusTab'].includes(node.test?.value);
    if (helper || host) receipts.push({
      name: helper ? `webpack:${moduleId}:A` : `case:${node.test.value}`,
      offset: node.start, end: node.end, source: raw.slice(node.start, node.end),
    });
  });
  if (receipts.length !== (moduleId === null ? 2 : 1)) throw Error(`Missing focus receipts: ${source}`);
  files.push({ source, sha256: crypto.createHash('sha256').update(bytes).digest('hex'), bytes: bytes.length, receipts });
}
fs.writeFileSync('docs/re/gamepad-calibration-focus-current-source.json', JSON.stringify({
  parser: 'Acorn static syntax only', offset_units: 'UTF-16 code units', files,
  semantics: {
    initial: 'Missing/empty tab name => true; otherwise require visible, not minimized and getFocusTab matching the requested tab (empty selected tab matches the synapse window itself).',
    focus_true: 'Re-query selected tab; no OS active-state comparison.',
    focus_false: 'Re-query visibility/minimization and selected tab; losing OS focus alone does not imply false.',
    changeActiveTab: 'Compare data.name with the requested tab.',
    activeTabChanged: 'Compare data.tabName with the requested tab.',
    popup: 'The mounted popup closes only after this helper has been true while open and later becomes false.',
  },
}, null, 2) + '\n');
console.log(JSON.stringify({ files: files.map(({ source, receipts }) => ({ source, receipts: receipts.map(({ name, offset, end }) => ({ name, offset, end })) })) }));

// Host producers must also be proven; the UI consumer alone does not establish
// whether a BrowserWindow blur means a different selected frame.
const producers = [];
for (const filename of ['common.js', 'TabManager.js']) {
  const source = `local-ui-reverse/source/installed/app-4.0.827/electron/components/Tab/${filename}`;
  const bytes = fs.readFileSync(source);
  const raw = bytes.toString('utf8');
  const receipts = [];
  walk(acorn.parse(raw, { ecmaVersion: 'latest' }), node => {
    const windowEvent = filename === 'common.js' && node.type === 'CallExpression'
      && node.callee?.type === 'MemberExpression' && node.callee.property?.name === 'on'
      && ['focus', 'blur'].includes(node.arguments[0]?.value);
    const broadcast = filename === 'common.js' && node.type === 'VariableDeclarator'
      && ['be', 'me'].includes(node.id?.name);
    const selected = filename === 'TabManager.js' && node.type === 'MethodDefinition'
      && node.key?.name === 'handleChangeActiveTab';
    if (windowEvent || broadcast || selected) receipts.push({
      name: windowEvent ? `window:${node.arguments[0].value}` : selected ? 'handleChangeActiveTab' : `broadcast:${node.id.name}`,
      offset: node.start, end: node.end, source: raw.slice(node.start, node.end),
    });
  });
  if (receipts.length !== (filename === 'common.js' ? 4 : 1)) throw Error(`Missing host focus producer: ${source}`);
  producers.push({ source, sha256: crypto.createHash('sha256').update(bytes).digest('hex'), bytes: bytes.length, receipts });
}
fs.writeFileSync('docs/re/gamepad-calibration-focus-host-producers-current.json', JSON.stringify({
  parser: 'Acorn static syntax only', offset_units: 'UTF-16 code units', files: producers,
  implementation: {
    initial: 'Once per mounted trigger popup; retries do not reinitialize the helper.',
    focus: 'GPUI activation event corresponds to common.js BrowserWindow focus/blur; true only queries selected host tab, false queries selected tab and borrowed native HWND visibility/minimization.',
    visibility: 'IsWindowVisible/IsIconic on the owning Windows window, never GPUI presentation visibility or GetForegroundWindow. No additional hide/resize/poll event is fabricated.',
    frame_identity: 'Retained local HostTab IDs serve as internal frame identity; this adapter does not claim an Electron BrowserView or its native frameName was queried.',
    missing: 'Other platform BrowserWindow status adapters, thumbstick focus connection and application runtime acceptance remain missing.',
  },
}, null, 2) + '\n');
