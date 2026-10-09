// Inspect current renderer/host state producers as AST data. Never evaluate reference JS.
const fs = require('fs');
const path = require('path');
const acorn = require('acorn');
const { Source, walk, key, hash } = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const bytes = file => fs.readFileSync(path.join(root, file));
const assert = (ok, message) => { if (!ok) throw Error(message); };
const source = new Source('systray/systrayv2');
const receipts = [];
const inputs = new Map();
function record(file, symbol, node, purpose) {
  inputs.set(file, hash(bytes(file)));
  const result = { path: file, sha256: hash(bytes(file)), symbol,
    start: node.start, end: node.end, source: read(file).slice(node.start, node.end), purpose };
  receipts.push(result);
  return result.source;
}
function binding(module, name, purpose) {
  return record(source.module(module).file, `${module}:${name}`, source.binding(module, name), purpose);
}
function unique(nodes, name) {
  assert(nodes.length === 1, `Expected one ${name}, received ${nodes.length}`);
  return nodes[0];
}
function select(tree, predicate, name) {
  const found = [];
  walk(tree, node => { if (predicate(node)) found.push(node); });
  return unique(found, name);
}
function method(module, name, purpose) {
  const scope = source.module(module);
  const node = select(scope.fn, node => node.type === 'CallExpression'
    && node.arguments[1]?.type === 'Literal' && node.arguments[1].value === name
    && node.arguments.length === 3, `${module} method ${name}`);
  return record(scope.file, `${module}:method ${name}`, node, purpose);
}
function getter(module, name, purpose) {
  const scope = source.module(module);
  const node = select(scope.fn, node => node.type === 'ObjectExpression'
    && node.properties.some(p => key(p.key) === 'key' && p.value.value === name)
    && node.properties.some(p => key(p.key) === 'get'), `${module} getter ${name}`);
  return record(scope.file, `${module}:getter ${name}`, node, purpose);
}
function assignedExport(module, name, purpose) {
  const scope = source.module(module);
  const node = select(scope.fn, node => node.type === 'AssignmentExpression'
    && node.left.type === 'MemberExpression' && key(node.left.property) === name
    && node.left.object.name === scope.fn.params[1].name
    && !['UnaryExpression', 'Identifier'].includes(node.right.type), `${module} export ${name}`);
  return record(scope.file, `${module}:export ${name}`, node, purpose);
}
const launchers = binding(9001, 'v', 'Sr: launcher preference, default sorting/limit and logo projection');
const apps = binding(9001, 'f', 'bN: current installed app catalog and running-window state producer');
assert(launchers.includes('get("launchers")') && launchers.includes('slice(0,5)')
  && launchers.includes('isAppInstalled'), 'Launcher producer changed');
assert(apps.includes('get("installedModules")') === false, 'Installed-module lookup moved into bN');
binding(9001, 'h', 'bN helper: installedModules and isAppInstalled jointly gate catalog membership');
binding(7071, 'm', 'RazerApp.apps: localStorage apps array; invalid/missing value returns []');
getter(3839, 'apps', 'Public apps getter delegates to module 7071');
getter(3839, 'userId', 'currentUserId localStorage accessor');
getter(3839, 'stayLoggedIn', 'Session persistence preference accessor');
getter(3839, 'otherAppsRunning', 'Window-service clients gate implicit session restoration');
getter(3839, 'userInfo', 'Public userInfo getter delegates to getUserInfoItem');
method(3839, 'getUserLoggedIn', 'Restores only when userId and stayLoggedIn/otherAppsRunning permit it');
binding(8180, 'J', 'getUserLoggedIn2: generate JWT from existing authentication then getUserOffline');
binding(8180, 'W', 'getUserOffline: getUserItemOffline followed by onUserGet');
binding(8180, 'X', 'onUserGet: emits userGet only with a currentUserId and real user data');
assignedExport(8180, 'onStorageChange', 'users changes emit userDataChange and decrypted userGet; apps changes emit catalog events');
const offline = binding(5845, 'I', 'getUserItemOffline requires IndexedDB auth; decrypts users data or creates an existing guest identity');
assert(offline.includes('User credentials not found') && offline.includes('decryptData')
  && offline.includes('isGuest:!0'), 'Offline account requirements changed');
binding(5845, 'P', 'getUserDataItem2 selects only the row matching current userId');
binding(5845, 'T', 'getUserInfoItem decrypts the current users row; no device DLL call');
assignedExport(7801, 'decryptData', 'Encrypted renderer user data decode helper');
for (const name of ['C', 'T', 'Qe', 'qe', 'Je']) {
  binding(2554, name,
    ({C:'Initial account item is {}; no fake guest',T:'Initial app items/launchers/installedApps are []',
      Qe:'userGet/userDataChange/logOut listeners and initial getUserLoggedIn request',
      qe:'Initial bN catalog fetch; storage apps/widgets/installedModules and launchers event routes',
      Je:'Left click queries getWindowStatus then toggles source browser visibility'})[name]);
}
for (const [file, property, purpose] of [
  ['.ref/host-4.0.827/electron/preload.js', 'getUserApps', 'Renderer IPC wrapper for getUserApps'],
  ['.ref/host-4.0.827/electron/lib/common.js', 'getUserApps', 'Current host enumerates User Data/Apps directories; this is not account state'],
]) {
  const tree = acorn.parse(read(file), { ecmaVersion: 'latest' });
  const node = select(tree, node => (node.type === 'Property' && key(node.key) === property)
    || (node.type === 'AssignmentExpression' && node.left.type === 'MemberExpression'
      && key(node.left.property) === property), `${file} ${property}`);
  record(file, property, node, purpose);
}
const mainFile = '.ref/host-4.0.827/electron/main.js';
const mainAst = acorn.parse(read(mainFile), { ecmaVersion: 'latest' });
for (const value of ['GET_USER_APPS', 'getMemoryStorageItem']) {
  const node = select(mainAst, node => node.type === 'SwitchCase'
    && (node.test?.value === value || (node.test?.type === 'MemberExpression'
      && key(node.test.property) === value)), `host action ${value}`);
  // Stacked memory-storage labels share the next nonempty consequent.
  record(mainFile, `electronAction:${value}`, node, value === 'GET_USER_APPS'
    ? 'GET_USER_APPS delegates to lib/common getUserApps, not a DLL'
    : 'Renderer-only memory-storage IPC action label; delegate receipt follows');
}
const memorySwitch = select(mainAst, node => node.type === 'SwitchStatement'
  && node.cases.some(c => c.test?.value === 'getMemoryStorageItem'), 'host memory-storage dispatch switch');
const memoryCaseIndex = memorySwitch.cases.findIndex(c => c.test?.value === 'getMemoryStorageItem');
const memoryDelegate = memorySwitch.cases.slice(memoryCaseIndex).find(c => c.consequent.length);
assert(memoryDelegate && read(mainFile).slice(memoryDelegate.start, memoryDelegate.end)
  .includes('memoryStorage?.callFunction'), 'Memory-storage IPC delegate changed');
record(mainFile, 'electronAction:getMemoryStorageItem shared delegate', memoryDelegate,
  'Stacked getMemoryStorageItem label falls through to memoryStorage.callFunction(action,payload)');
const memoryFile = '.ref/host-4.0.827/electron/modules/memory_storage/index.js';
const memoryAst = acorn.parse(read(memoryFile), { ecmaVersion: 'latest' });
record(memoryFile, 'getMemoryStorageItem', select(memoryAst, node => node.type === 'SwitchCase'
  && node.test?.value === 'getMemoryStorageItem', 'memory-storage query'),
  'Reads host process Map and returns JSON [{value,windowName}]; not a device DLL or disk cache');
const simpleFile = '.ref/host-4.0.827/electron/modules/simple_service/win/index.js';
const simpleAst = acorn.parse(read(simpleFile), { ecmaVersion: 'latest' });
for (const predicate of [
  node => node.type === 'Property' && key(node.key) === 'simpleGetUserApps',
  node => node.type === 'PropertyDefinition' && key(node.key) === 'simpleGetUserApps',
]) {
  record(simpleFile, 'simpleGetUserApps', select(simpleAst, predicate, 'simple-service apps ABI/method'),
    'Installed app-list query only: void(string, pointer), callback void(bool,string,string)');
}
const manifestPath = source.directory + '/asset-manifest.json';
inputs.set(manifestPath, hash(bytes(manifestPath)));
const tray = read('src/shell/tray.rs');
assert(tray.includes('session: TraySession::Guest'), 'Tray must mount the source guest presentation branch');
assert(tray.includes('launchers: vec![TrayLauncher'), 'Review local activation-target presentation boundary');
assert(tray.includes('not an observed Razer'), 'Local Guest must not claim an observed account');
const backend = read('src/backend/mod.rs');
assert(backend.includes('BLOCKING_ENGINES: &[&str] = &["SysUtilsNative"]'), 'SysUtils ABI load boundary changed');
const output = {
  schema_version: 1, method: 'Acorn static AST only; no reference JS, app or DLL execution',
  offset_encoding: 'UTF-16, zero-based, end-exclusive',
  generator_sha256: hash(bytes('tools/audit-tray-state-source.cjs')),
  source_inputs: Object.fromEntries(inputs), receipts,
  state_contract: {
    initial_user_item: {}, initial_app_items: [], initial_launchers: [], initial_installed_apps: [],
    launcher: 'bN reads renderer metadata/preferences and observed installed/running apps; Sr uses saved launchers or up to five title-sorted non-website catalog entries.',
    account: 'userGet/userDataChange are renderer identity/storage events backed by existing auth credentials and user data, not device-query results.',
  },
  implementation_boundary: {
    files: ['src/shell/tray.rs', 'src/backend/mod.rs', 'src/backend/runtime.rs'],
    account_publisher: 'Not connected; the local tray mounts only the source Guest presentation branch and does not claim a signed-in identity.',
    launcher_publisher: 'Official catalog/preferences publisher not connected; one Synapse footer row activates this running local process. It is not an observed official apps/installedModules/launchers result.',
    available_next_step: 'A worker simpleGetUserApps query can publish installed-app observations after ABI/PE registration. It cannot replace source apps/installedModules/launchers/user storage state.',
    unavailable_bridge: 'Existing Rust backend has no source-verified Electron renderer localStorage/IndexedDB/memoryStorage or window-service IPC endpoint.',
    forbidden_inference: 'Do not infer Guest/Authenticated from devices, app directories or cache existence; do not guess a disk-cache schema.',
    sysutils: 'SysUtilsNative remains load-blocked pending verified ABI/lifecycle; export presence is insufficient.',
  },
  runtime_validation: 'not_run',
};
const target = 'docs/re/tray-state-current-evidence.json';
const serialized = JSON.stringify(output, null, 2) + '\n';
if (process.argv.includes('--check')) assert(read(target).replace(/\r\n/g, '\n') === serialized,
  'Tray state evidence stale; inspect source before regenerating');
else fs.writeFileSync(path.join(root, target), serialized);
console.log(`Tray state: ${receipts.length} current AST receipts; account and launcher producer boundaries checked.`);
