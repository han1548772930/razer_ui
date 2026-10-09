// Read current host/Dashboard JavaScript only as Acorn AST data. Reference
// files are never required, imported, evaluated, or run. --check never writes.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const acorn = require('acorn');
const {Source, walk, key} = require('./webpack-source.cjs');

const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const requireFact = (condition, message) => { if (!condition) throw Error(message); };
const all = (node, predicate) => {
  const found = [];
  walk(node, child => { if (predicate(child)) found.push(child); });
  return found;
};
const one = (node, predicate, description) => {
  const found = all(node, predicate);
  requireFact(found.length === 1, `Expected one ${description}, found ${found.length}`);
  return found[0];
};
const member = (node, name) => node?.type === 'MemberExpression' && key(node.property) === name;
const call = (node, name) => node.type === 'CallExpression' && member(node.callee, name);

class StaticFile {
  constructor(file) {
    this.file = file;
    this.bytes = fs.readFileSync(path.join(root, file));
    this.source = this.bytes.toString('utf8');
    this.ast = acorn.parse(this.source, {ecmaVersion: 'latest'});
    this.bindings = new Map();
    for (const statement of this.ast.body) {
      if (statement.type === 'VariableDeclaration') {
        for (const declaration of statement.declarations) {
          if (declaration.id.type === 'Identifier') this.bindings.set(declaration.id.name, declaration.init);
        }
      } else if (statement.type === 'FunctionDeclaration') {
        this.bindings.set(statement.id.name, statement);
      }
    }
  }
  text(node) { return this.source.slice(node.start, node.end); }
  receipt(node) {
    return {path: this.file, sha256: hash(this.bytes), offset: node.start, end: node.end, source: this.text(node)};
  }
  imports(moduleName) {
    const declaration = one(this.ast, node => node.type === 'VariableDeclarator'
      && node.id.type === 'ObjectPattern' && node.init?.type === 'CallExpression'
      && node.init.callee.name === 'require' && node.init.arguments[0]?.value === moduleName,
    `destructured import ${moduleName} in ${this.file}`);
    return Object.fromEntries(declaration.id.properties.map(property => [key(property.key), property.value.name]));
  }
  exported(name) {
    const assignment = one(this.ast, node => node.type === 'AssignmentExpression'
      && member(node.left, 'exports') && node.left.object.name === 'module', `module.exports in ${this.file}`);
    const property = assignment.right.properties.find(property => key(property.key) === name);
    requireFact(property, `Missing export ${name} in ${this.file}`);
    const value = property.value.type === 'Identifier' ? this.bindings.get(property.value.name) : property.value;
    requireFact(value, `Unresolved export ${name} in ${this.file}`);
    return value;
  }
}

const base = '.ref/host-4.0.827/electron/';
const packagePath = '.ref/host-4.0.827/package.json';
const packageBytes = fs.readFileSync(path.join(root, packagePath));
const hostPackage = JSON.parse(packageBytes.toString('utf8'));
requireFact(hostPackage.name === 'razerappengine' && hostPackage.version === '4.0.827', 'Current host package identity changed');
const tab = new StaticFile(`${base}components/Tab/Tab.js`);
const common = new StaticFile(`${base}components/Tab/common.js`);
const ui = new StaticFile(`${base}components/Tab/TabUI.js`);
const manager = new StaticFile(`${base}components/Tab/TabManager.js`);
const preload = new StaticFile(`${base}preload.js`);
const tabImports = tab.imports('./common');
const tabElectron = tab.imports('electron');
const createTab = one(tab.ast, node => node.type === 'PropertyDefinition' && key(node.key) === 'createTab', 'Tab.createTab');
const windowHandler = one(createTab, node => call(node, 'setWindowOpenHandler'), 'Tab.createTab window-open handler');
const handler = windowHandler.arguments[0]?.body?.callee;
requireFact(handler?.type === 'ArrowFunctionExpression' && handler.body.type === 'BlockStatement', 'Tab opener handler shape changed');
const statements = handler.body.body;
const newWindowBranch = one(handler.body, node => node.type === 'IfStatement'
  && all(node.test, child => member(child, 'policy')).length === 4, '5/6/7/8 branch');
const policies = all(newWindowBranch.test, node => node.type === 'Literal' && typeof node.value === 'number').map(node => node.value);
requireFact(JSON.stringify(policies) === '[5,6,7,8]', 'Tab new-window policy set changed');
requireFact(all(newWindowBranch.consequent, node => node.type === 'CallExpression'
  && node.callee.name === tabImports.createWindow).length === 1, 'New-window branch must call imported createWindow');
const sameWindowBranch = statements.find(node => node.type === 'IfStatement'
  && all(node, child => child.type === 'CallExpression' && child.callee.name === tabImports.sendCreateNewTabAction).length === 2);
requireFact(sameWindowBranch, 'Missing existing-host createNewTab branch');
const sendTabCalls = all(sameWindowBranch, node => node.type === 'CallExpression' && node.callee.name === tabImports.sendCreateNewTabAction);
requireFact(tab.text(sameWindowBranch).includes('attachToWindow') && tab.text(sameWindowBranch).includes('3===+'), 'Missing policy 3 attachToWindow handling');
const reuseTabBranch = statements.find(node => node.type === 'IfStatement' && call(node.test, 'findTab'));
const reuseWindowBranch = statements.find(node => node.type === 'IfStatement'
  && all(node.consequent, child => child.type === 'Literal' && child.value === 'appFocus').length === 1);
requireFact(reuseTabBranch && reuseWindowBranch && reuseTabBranch.start < newWindowBranch.start
  && reuseWindowBranch.start < newWindowBranch.start, 'Name reuse must precede the policy creation branch');
requireFact(all(reuseTabBranch, node => node.type === 'CallExpression' && node.callee.name === tabImports.sendChangeActiveTab).length === 1,
  'Existing tab must request activation');
const viewCreation = one(createTab, node => node.type === 'AssignmentExpression' && member(node.left, 'tab')
  && node.left.object.type === 'ThisExpression' && node.right.type === 'ConditionalExpression', 'Tab view creation');
requireFact(viewCreation.right.consequent.type === 'NewExpression'
  && viewCreation.right.consequent.callee.name === tabElectron.WebContentsView
  && viewCreation.right.alternate.type === 'NewExpression'
  && viewCreation.right.alternate.callee.name === tabElectron.BrowserView, 'Tab must create embedded views, not BrowserWindow');
const attachView = one(tab.ast, node => node.type === 'PropertyDefinition' && key(node.key) === 'setTabAsActiveView', 'attach active view');
requireFact(all(attachView, node => call(node, 'addChildView')).length === 1
  && all(attachView, node => call(node, 'setBrowserView')).length === 1, 'Tab view attachment changed');

const sendNewTab = common.exported('sendCreateNewTabAction');
const sendMessage = common.exported('sendTabMessage');
const createWindow = common.exported('createWindow');
requireFact(all(sendNewTab, node => node.type === 'CallExpression' && common.bindings.get(node.callee.name) === sendMessage).length === 1,
  'sendCreateNewTabAction must call the exported sendTabMessage binding');
const commonElectron = common.imports('electron');
const browserWindow = one(createWindow, node => node.type === 'NewExpression' && node.callee.name === commonElectron.BrowserWindow, 'BrowserWindow constructor');
const newTabMessage = one(sendNewTab, node => node.type === 'ObjectExpression'
  && node.properties.some(property => key(property.key) === 'action' && property.value.value === 'createNewTab'), 'createNewTab message');
const newTabData = newTabMessage.properties.find(property => key(property.key) === 'data').value;
const windowId = newTabData.properties.find(property => key(property.key) === 'windowId').value;
requireFact(member(windowId, 'id') && windowId.object.name === sendNewTab.params[0].name, 'createNewTab must retain the selected host window id');
requireFact(all(sendMessage, node => call(node, 'send') && node.arguments[0]?.value === 'tab-message').length === 1, 'Host tab-message channel changed');
const uiReceive = one(ui.ast, node => node.type === 'SwitchCase' && node.test?.value === 'createNewTab', 'TabUI createNewTab receive');
requireFact(all(uiReceive, node => call(node, 'createNewTab')).length === 1, 'TabUI receive must call createNewTab');
const uiCreate = one(ui.ast, node => node.type === 'PropertyDefinition' && key(node.key) === 'createNewTab', 'TabUI.createNewTab');
const uiSend = one(ui.ast, node => node.type === 'MethodDefinition' && key(node.key) === 'sendEvent', 'TabUI.sendEvent');
requireFact(all(uiCreate, node => node.type === 'Property' && key(node.key) === 'action' && node.value.value === 'tab-create').length === 1,
  'TabUI must dispatch tab-create');
requireFact(all(uiSend, node => call(node, 'doTabAction')).length === 1, 'TabUI must call preload doTabAction');
const ipcBridge = one(preload.ast, node => node.type === 'Property' && key(node.key) === 'doTabAction', 'preload doTabAction');
requireFact(all(ipcBridge, node => call(node, 'invoke') && node.arguments[0]?.value === 'tabEvent').length === 1, 'doTabAction IPC channel changed');
const ipcHandler = one(manager.ast, node => call(node, 'handle') && node.arguments[0]?.value === 'tabEvent', 'TabManager tabEvent handler');
const tabCreateCase = one(ipcHandler, node => node.type === 'SwitchCase' && node.test?.value === 'tab-create', 'TabManager tab-create case');
const tabImport = one(manager.ast, node => node.type === 'VariableDeclarator' && node.init?.type === 'CallExpression'
  && node.init.callee.name === 'require' && node.init.arguments[0]?.value === './Tab', 'TabManager Tab import');
const tabConstructor = one(tabCreateCase, node => node.type === 'NewExpression' && node.callee.name === tabImport.id.name, 'new Tab in tab-create');
requireFact(tabConstructor.arguments.length === 3 && member(tabConstructor.arguments[2], 'windowId'), 'Tab construction must receive the existing windowId');

const dashboard = new Source('synapse/dashboard');
const flagsNode = dashboard.binding(84058, dashboard.exported(84058, 'ZP').name);
const flags = dashboard.literal(84058, flagsNode);
requireFact(flags.sameWindow === 'policy=3' && flags.diffWindow === 'policy=5', 'Dashboard window flags changed');
const openFunction = dashboard.binding(84058, dashboard.exported(84058, '_P').name);
requireFact(all(openFunction, node => call(node, 'open') && node.callee.object.name === 'window').length === 1,
  'Dashboard helper must reach window.open for a new name');
const queryName = dashboard.binding(43112, dashboard.exported(43112, 'id').name);
const focusName = dashboard.binding(43112, dashboard.exported(43112, 'yH').name);
requireFact(dashboard.snippet(43112, queryName).includes('getWindowServiceClients')
  && dashboard.snippet(43112, queryName).includes('a.windowName===e'), 'Dashboard named target query changed');
requireFact(dashboard.snippet(43112, focusName).includes('activateWindowServiceClient'), 'Dashboard named target activation changed');
const registry = dashboard.module(54420);
const registryRows = all(registry.fn, node => node.type === 'ObjectExpression'
  && node.properties.some(property => key(property.key) === 'windowName')
  && node.properties.some(property => key(property.key) === 'openParam')).map(node => {
  const props = Object.fromEntries(node.properties.map(property => [key(property.key), property.value]));
  return {window_name: dashboard.literal(54420, props.windowName), url_expression: dashboard.snippet(54420, props.url),
    open_param: dashboard.literal(54420, props.openParam), receipt: dashboard.receipt(54420, node)};
});
for (const name of ['macro', 'armory', 'profiles', 'alexa', 'feedback-synapse']) {
  requireFact(registryRows.some(row => row.window_name === name && row.open_param.startsWith('policy=3,')), `Missing policy 3 Dashboard module ${name}`);
}
const pairingModule = dashboard.module(22534);
const pairingImport = dashboard.binding(22534, 'D');
requireFact(pairingImport.type === 'CallExpression' && pairingImport.arguments[0]?.value === 84058, 'Pairing helper import changed');
const pairingCallback = one(pairingModule.fn, node => node.type === 'Property' && key(node.key) === 'overrideAction'
  && all(node, child => call(child, 'set') && child.arguments[0]?.value === 'displayMode'
    && child.arguments[1]?.value === 'multiDevicePairing').length === 1, 'pairing URL construction callback');
const pairingOpen = one(pairingCallback, node => node.type === 'CallExpression'
  && node.callee.type === 'SequenceExpression'
  && node.callee.expressions.some(value => member(value, '_P') && value.object.name === 'D')
  && node.arguments.some(value => member(value, 'sameWindow')), 'Dashboard pairing sameWindow open call');
requireFact(dashboard.snippet(22534, pairingCallback).includes('"multiDevicePairing"')
  && dashboard.snippet(22534, pairingCallback).includes('"containerId"'), 'Pairing route contract changed');

const shell = read('crates/razer-shell/src/shell.rs');
const tabs = read('crates/razer-shell/src/shell/host_tabs.rs');
const windowLayer = read('crates/razer-shell/src/shell/display_window.rs');
const pairingStart = shell.indexOf('fn open_product_pairing_tab(');
const chromaStart = shell.indexOf('fn open_chroma_window(', pairingStart);
const moduleStart = shell.indexOf('fn open_module_tab(');
requireFact(pairingStart >= 0 && chromaStart > pairingStart && moduleStart > chromaStart, 'Local routing methods missing');
const pairingLocal = shell.slice(pairingStart, chromaStart);
requireFact(pairingLocal.includes('multi_device_pairing_name(&identity)')
  && pairingLocal.includes('self.navigate(Location::Pairing(name.to_string())')
  && !pairingLocal.includes('WindowPolicy::Different')
  && !pairingLocal.includes('display_window::open_or_focus'),
  'Local pairing opener must navigate through a named host tab');
requireFact(windowLayer.includes('policy != WindowPolicy::Same') && windowLayer.includes('policy=3 must be opened through the host tab registry'),
  'OS-window layer must reject policy 3');
const moduleLocal = shell.slice(moduleStart, shell.indexOf('\n    fn ', moduleStart + 5));
requireFact(moduleLocal.includes('self.navigate(next, window, cx)'), 'Module opener must navigate through host tabs');
for (const name of ['macro', 'armory', 'profiles', 'alexa', 'feedback-synapse']) {
  requireFact(tabs.includes(`=> "${name}".into()`), `Native host tab lacks source name ${name}`);
}

const report = {
  schema_version: 1,
  source_version: '4.0.827',
  source_package: {path: packagePath, sha256: hash(packageBytes), name: hostPackage.name, version: hostPackage.version},
  source_version_audit: 'docs/re/current-host-version-audit.md',
  scope: 'Application-internal window.open requests originating in an existing Dashboard Tab. Razer ID, external URLs, absent host windows and other BrowserWindow-origin opener paths are separate branches.',
  policy: {same_window: 3, new_window_from_tab: policies, name_reuse_precedes_creation: true,
    attach_to_window: 'policy 3 may select the host owning attachToWindow; otherwise the opener Tab host is used.',
    source_names_are_not_os_window_types: true},
  host: {
    imports: {tab_common: tabImports, tab_electron: tabElectron},
    open_handler: tab.receipt(windowHandler), existing_tab: tab.receipt(reuseTabBranch), existing_named_window: tab.receipt(reuseWindowBranch),
    new_window_branch: tab.receipt(newWindowBranch), same_host_branch: tab.receipt(sameWindowBranch),
    target_calls: sendTabCalls.map(node => tab.receipt(node)),
    send_create_new_tab: common.receipt(sendNewTab), send_tab_message: common.receipt(sendMessage),
    browser_window_constructor: common.receipt(browserWindow),
    ui_receive: ui.receipt(uiReceive), ui_create_tab: ui.receipt(uiCreate), ui_send_event: ui.receipt(uiSend),
    preload_bridge: preload.receipt(ipcBridge), manager_ipc_channel: manager.receipt(ipcHandler.arguments[0]), manager_tab_constructor: manager.receipt(tabCreateCase),
    embedded_view_constructor: tab.receipt(viewCreation), attach_active_view: tab.receipt(attachView),
  },
  dashboard: {flags: {values: flags, receipt: dashboard.receipt(84058, flagsNode)},
    open_helper: dashboard.receipt(84058, openFunction), query_named_target: dashboard.receipt(43112, queryName), activate_named_target: dashboard.receipt(43112, focusName),
    registry_module: 54420, registry_rows: registryRows,
    pairing: {module: 22534, source_policy: 3, helper_import: dashboard.receipt(22534, pairingImport), callback: dashboard.receipt(22534, pairingCallback), open_call: dashboard.receipt(22534, pairingOpen)}},
  local: {checked_files: ['crates/razer-shell/src/shell.rs', 'crates/razer-shell/src/shell/host_tabs.rs', 'crates/razer-shell/src/shell/display_window.rs'],
    module_opening: 'Named host tabs through open_module_tab -> navigate; source names are preserved.',
    os_window_guard: 'display_window::open_or_focus rejects WindowPolicy::Same.',
    pairing_exception: 'Product pairing follows source sameWindow/policy=3 by navigating to a named host tab. The tab name preserves the source container/product/serial identity and is reused on repeat opens.'},
  verification: 'Static Acorn AST/literal resolution plus local Rust source checks. No reference JavaScript, installers, DLLs, application, build or tests executed. --check only compares generated evidence/document bytes and never writes.',
};
const markdown = [
  '# 当前宿主窗口策略审计', '',
  '2026-10-04。当前正式宿主 4.0.827 的 `Tab.js` 证明：从已有 Dashboard 页签发出的应用内请求，',
  '`policy=3` 添加或聚焦宿主中的具名页签；`policy=5/6/7/8` 才进入创建原生窗口的分支。',
  '`windowName`、`frameName` 和 `sameWindow` 的名字本身不能证明“独立操作系统窗口”。', '',
  '版本与提取来源见 [current-host-version-audit.md](current-host-version-audit.md)。本次只读取',
  '`.ref/host-4.0.827/` 和当前 `.ref/applications/synapse/dashboard/`；没有引用被删除的旧宿主。', '',
  '## 当前源码调用链', '',
  '1. `Tab.js:createTab` 注册 `webContents.setWindowOpenHandler`。已有同名 Tab 时先',
  '   `sendChangeActiveTab(frameName)` 并拒绝重复打开；命中 `winMap` 时发出 `appFocus` 并返回。',
  '   这两项发生在按 policy 创建的分支之前。Razer ID 与外部 URL 有单独处理。',
  '2. 对新名字，`policy=5/6/7/8` 调用导入的 `createWindow`；其当前实现包含 `new BrowserWindow(...)`。',
  '   其余请求在能找到当前宿主时调用 `sendCreateNewTabAction(currentHost, frameName, url, features)`。',
  '   `policy=3` 若带有可解析的 `attachToWindow`，则改用该名字所属的宿主窗口；这仍然是添加页签。',
  '3. `common.js:sendCreateNewTabAction` 发出 `createNewTab`，载荷中的 `windowId` 来自选定的宿主。',
  '   `sendTabMessage` 通过 `tab-message` 通道发送给该窗口及其页签。',
  '4. `TabUI.js` 收到 `createNewTab` 后调用 `createNewTab`，创建页签 DOM，保持 `windowId`，',
  '   再通过 `sendEvent` 发出 `tab-create`。`preload.js:doTabAction` 使用 `invoke("tabEvent", ...)`。',
  '5. `TabManager.js` 的 `tabEvent` 处理器在 `tab-create` 分支调用',
  '   `new Tab(url, features, windowId).init()`。`Tab.js` 建立 `WebContentsView`（旧 Electron 分支为',
  '   `BrowserView`），通过 `contentView.addChildView` 或 `setBrowserView` 挂到既有宿主。', '',
  '这里的结论限定于已有 `Tab` 发起的路径。`common.js` 中其他顶层 `BrowserWindow` 的打开处理器、',
  '外部链接、登录窗口和宿主缺失情况不能简单套用这张表。', '',
  '## Dashboard 入口证据', '',
  '模块 84058 的枚举把 `sameWindow` 定义为 `policy=3`，`diffWindow` 定义为 `policy=5`。',
  '其 `_P` 打开函数先查询并聚焦同名目标，目标不存在时才调用 `window.open(url, name, flags)`。',
  '模块 54420 的当前表中，下列入口均登记 `policy=3`：', '',
  '| 具名目标 | 当前源码路径表达式 |',
  '| --- | --- |',
  ...registryRows.filter((row, index, rows) => ['macro', 'armory', 'profiles', 'alexa', 'feedback-synapse'].includes(row.window_name)
    && rows.findIndex(candidate => candidate.window_name === row.window_name) === index)
    .map(row => `| \`${row.window_name}\` | \`${row.url_expression}\` |`), '',
  '配对入口同样如此：模块 22534 的设备盒构建产品 URL，写入 `containerId`、',
  '`displayMode=multiDevicePairing` 及非空时的 `allMasters`，然后调用模块 84058 的 `_P`，',
  '传入 `sameWindow` 与 `autoFocus`。它没有传入 `diffWindow`。完整 AST 范围、文件 SHA-256',
  '及跨模块收据见 [host-window-policy-current-evidence.json](host-window-policy-current-evidence.json)。', '',
  '## 本地实现与配对例外', '',
  '模块入口使用 `open_module_tab -> navigate` 进入宿主页签，保留 `macro`、`armory`、',
  '`profiles`、`alexa` 和 `feedback-synapse` 等源码身份。同名再次打开由宿主页签注册表复用。',
  '`display_window::open_or_focus` 拒绝 `WindowPolicy::Same`，防止将 policy 3 误送到原生窗口创建路径。', '',
  '**多设备配对使用具名宿主 Tab。** 本地 `open_product_pairing_tab` 保留来源的',
  '`sameWindow/policy=3` 语义，按容器、产品与序列号生成 Tab 名称并在重复打开时复用。', '',
  '## 静态复核', '',
  '生成：`node tools/audit-host-window-policy.cjs`。只读校验：',
  '`node tools/audit-host-window-policy.cjs --check`。校验模式只解析并比较证据与文档，不写文件。',
  '该工具以 Acorn 解析引用文件，静态解析 CommonJS 导入/导出与 webpack 模块，完全不执行下载的',
  'JavaScript。此次没有运行应用、构建、测试、安装器或 DLL；静态调用链不替代运行时交互校验。', '',
].join('\n');
const outputs = [
  ['docs/re/host-window-policy-current-evidence.json', JSON.stringify(report, null, 2) + '\n'],
  ['docs/re/host-window-policy-current-audit.md', markdown],
];
if (process.argv.includes('--check')) {
  for (const [file, expected] of outputs) requireFact(read(file) === expected, `Stale generated file: ${file}`);
  console.log('host window policy audit: verified current AST chain, native routes and pairing tabs; no files written');
} else {
  for (const [file, text] of outputs) fs.writeFileSync(path.join(root, file), text);
  console.log('host window policy audit: generated current evidence and audit');
}
