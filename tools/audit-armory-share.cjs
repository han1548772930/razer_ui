// Current Armory share/detail controls, parsed as AST and CSS data only.
// No reference module is loaded or evaluated. --check performs no writes.
const fs = require('fs');
const path = require('path');
const acorn = require('acorn');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const source = new Source('synapse/armory');
const requireFact = (condition, message) => { if (!condition) throw Error(message); };
const one = (root, predicate, description) => {
  const found = [];
  walk(root, node => { if (predicate(node)) found.push(node); });
  requireFact(found.length === 1, `Expected one ${description}; found ${found.length}`);
  return found[0];
};
function deviceMenu(pid, className, labels, categories) {
  const directory = `.ref/devices/${pid}`;
  const manifestPath = `${directory}/asset-manifest.json`;
  const manifest = JSON.parse(read(manifestPath));
  const main = Object.values(manifest.files).find(file => /(?:^|\/)main\.[a-f0-9]+\.js$/.test(file));
  requireFact(main, `Missing current PID ${pid} main bundle`);
  const file = `${directory}/${main.replace(/^\.\//, '').replace(/^\//, '')}`;
  const text = read(file);
  const ast = acorn.parse(text, {ecmaVersion: 'latest'});
  const cls = one(ast, node => node.type === 'ClassDeclaration' && node.id.name === className, `PID ${pid} profile class`);
  const scope = one(ast, node => node.type === 'BlockStatement' && node.body.includes(cls), `PID ${pid} profile lexical scope`);
  const bindings = new Map(scope.body.filter(node => node.type === 'VariableDeclaration')
    .flatMap(node => node.declarations).filter(node => node.id.type === 'Identifier').map(node => [node.id.name, node.init]));
  const receipt = node => ({path: file, sha256: hash(text), offset: node.start, end: node.end, source: text.slice(node.start, node.end)});
  const methods = Object.fromEntries(['getProfileMenu', 'updateProfileSet', 'openArmoryEditView', 'getDisabledItems'].map(name => {
    const method = one(cls, node => node.type === 'AssignmentExpression' && node.left.type === 'MemberExpression'
      && node.left.object.type === 'ThisExpression' && key(node.left.property) === name, `PID ${pid} ${name}`);
    return [name, receipt(method)];
  }));
  const imported = Object.assign(Object.create(Source.prototype), {directory, files: [file], modules: new Map(), texts: new Map(), parsed: new Set()});
  imported.parse(file);
  const resolve = (namespace, name) => {
    const binding = bindings.get(namespace);
    requireFact(binding?.type === 'CallExpression' && Number.isInteger(binding.arguments[0]?.value), `PID ${pid} namespace ${namespace}`);
    const module = binding.arguments[0].value;
    const exported = imported.exported(module, name);
    return {module, namespace: receipt(binding), value: imported.literal(module, exported),
      literal: imported.receipt(module, exported.type === 'Identifier' ? imported.binding(module, exported.name) : exported)};
  };
  const share = resolve(labels, 's1I'), edit = resolve(labels, 's_O'), supported = resolve(categories, '$u');
  requireFact(share.value === 'SHARE_TO_WORKSHOP' && edit.value === 'EDIT_DETAILS_IN_WORKSHOP', `PID ${pid} share labels changed`);
  requireFact(JSON.stringify(supported.value) === JSON.stringify(['KEYBOARD', 'MOUSE', 'AUDIO', 'SYSTEM', 'BROADCASTER', 'MOUSEPLUSMAT']), `PID ${pid} sharing categories changed`);
  requireFact(methods.updateProfileSet.source.includes(`case ${labels}.s1I:case ${labels}.s_O:this.openArmoryEditView()`), `PID ${pid} menu handler changed`);
  for (const fragment of ['selectedProfileGuid', 'productId', '/synapse/armory/?view=my-upload&sharePopup=true&pid=', 'type:"PROFILE"', 'policy=3,tab_visible=1']) {
    requireFact(methods.openArmoryEditView.source.includes(fragment), `PID ${pid} opener changed: ${fragment}`);
  }
  for (const fragment of ['installedVersion', 'isProfileSharingEnabled', 'CAN_RESHARE', 'armoryMaintenanceModeEnabled']) {
    requireFact(methods.getProfileMenu.source.includes(fragment), `PID ${pid} menu gate changed: ${fragment}`);
  }
  requireFact(methods.getDisabledItems.source.includes('.SHARED') && methods.getDisabledItems.source.includes('armoryMaintenanceModeEnabled'), `PID ${pid} disabled states changed`);
  return {pid, manifest: {path: manifestPath, sha256: hash(read(manifestPath))}, class_name: className, share, edit, supported, methods};
}
const deviceMenus = [deviceMenu(182, 'lD', 'aE', 'EE'), deviceMenu(653, 'UD', 'nt', 'H')];
const form = source.binding(13784, 'Q');
const modal = source.binding(24988, 'A');
const validator = one(modal, node => node.type === 'VariableDeclarator' && node.id.name === 'M', 'modal submit predicate').init;
requireFact(source.snippet(24988, validator).includes('t===c.VZ.PROFILE?-1!==i&&-1!==r&&!s&&d'), 'PROFILE submit predicate changed');
const inputs = Object.fromEntries(['title', 'description'].map(name => {
  const node = one(form, node => node.type === 'ObjectExpression'
    && node.properties.some(property => key(property.key) === 'name' && property.value?.value === name)
    && node.properties.some(property => key(property.key) === 'maxLength'), `${name} input`);
  return [name, {max_length: node.properties.find(property => key(property.key) === 'maxLength').value.value, receipt: source.receipt(13784, node)}];
}));
requireFact(inputs.title.max_length === 32 && inputs.description.max_length === 500, 'Share input length boundaries changed');
const formText = source.snippet(13784, form);
for (const fragment of ['ae.gamesSupported.length<10', 'hideProfileInfo:!0', 'isSingleProfile', 'profile:-1', 'maxLength:32', 'maxLength:500']) {
  requireFact(formText.includes(fragment), `Share form contract changed: ${fragment}`);
}
const gamePicker = source.binding(16230, 'g');
requireFact(source.snippet(16230, gamePicker).includes('showFileOpenDialog("Open",!0,["exe","url"])'), 'Source game browse types changed');
const service = Object.fromEntries(['nX', 'tl'].map(name => {
  const node = source.binding(9728, source.exported(9728, name).name);
  return [name === 'nX' ? 'submit' : 'moderation', source.receipt(9728, node)];
}));
requireFact(service.submit.source.includes('"/contributions"') && service.moderation.source.includes('"/toxicity/validate"'), 'Share service endpoints changed');
const labelKeys = ['SHARE_NEW', 'FALLBACK_DEVICE_PROMPT', 'DEVICE_PLACEHOLDER', 'SELECT_PROFILE', 'PROFILE_PLACEHOLDER',
  'TITLE', 'HELPER_TEXT', 'AWESOME_CONTENT_TITLE', 'DESCRIPTION_OPTIONAL', 'DESCRIPTION_INPUT_PLACEHOLDER', 'ADD_DETAILS',
  'GAMES_SUPPORTED_OPTIONAL', 'ADD_GAME', 'FILE_SIZE', 'SUBMIT', 'PROFILE_DETAILS', 'SENSITIVITY', 'CANCEL', 'CLOSE', 'REMOVE'];
const locales = [];
for (const file of source.files.filter(file => /\/trans-[^.]+\.[a-f0-9]+\.chunk\.js$/.test(file))) {
  source.parse(file);
  const module = [...source.modules.values()].find(module => module.file === file && module.exports.has('SHARE_NEW'));
  requireFact(module, `Missing Armory share locale ${file}`);
  const locale = path.basename(file).match(/^trans-([^.]+)\./)[1];
  const local = JSON.parse(read(`locales/${locale}.json`));
  const values = Object.fromEntries(labelKeys.map(name => [name, source.literal(module.id, source.exported(module.id, name))]));
  for (const [name, value] of Object.entries(values)) requireFact(local[name] === value, `Current locale mismatch: ${locale}/${name}`);
  locales.push({locale, module: module.id, path: file, sha256: hash(source.text(file)), values});
}
requireFact(locales.length === 10, `Expected ten current Armory locales; found ${locales.length}`);
const css = [];
for (const name of ['main.c0e644c4.css', '458.d86e844b.chunk.css']) {
  const file = `${source.directory}/static/css/${name}`, text = read(file);
  for (const rule of parseCSS(text).filter(rule => /share-new-profile|detail-footer/.test(rule.selector))) {
    css.push({path: file, sha256: hash(text), ...rule});
  }
}
const page = read('src/shell/armory_page.rs');
const share = read('src/shell/armory_page/share_profile.rs');
const shareCompact = share.replace(/\s+/g, '');
const shell = read('src/shell.rs');
const profileMenu = read('src/features/profile.rs');
for (const fragment of ['pub(super) fn open_share_profile(', 'ShareProfileDialog::new(device, window, cx)', '.children(self.share_profile.clone())', 'phase1: false', 'guest: true']) {
  requireFact(page.includes(fragment), `Missing local share entry/default guard: ${fragment}`);
}
for (const fragment of ['TITLE_LIMIT: usize = 32', 'DESCRIPTION_LIMIT: usize = 500', 'GAME_LIMIT: usize = 10',
  'fn has_title', 'has_title(&self.title.read(cx).value())', 'self.device.profiles.iter()', 'self.unavailable = true',
  'DialogPopup::new()', 'close_on_backdrop_press(false)', 'ShareClosed', 'armory-local-profile-preview',
  'extension.eq_ignore_ascii_case("exe")', 'extension.eq_ignore_ascii_case("url")', 'this.games.retain',
  'Local draft', 'has not been submitted']) {
  requireFact(shareCompact.includes(fragment.replace(/\s+/g, '')), `Missing native share contract: ${fragment}`);
}
requireFact(shell.includes('open_share_profile('), 'Share dialog is not reachable through AppShell');
requireFact(profileMenu.includes('matches!(pid, 182 | 653)') && profileMenu.includes('linked.push(Share)')
  && profileMenu.includes('ProfileCommand::Share => cx.emit(WorkspaceEvent::ShareProfile)'), 'Current supported profile menu entry is missing');
requireFact(shell.includes('WorkspaceEvent::ShareProfile =>') && shell.includes('let device = entity.read(cx).snapshot(cx);'), 'Share event does not capture the actual local device');
requireFact(!share.includes('a5e4d21d-d5db-41b0-9a85-7926acee9a04') && !share.includes('"3KB"'), 'Native preview copied source placeholder contribution data');
const report = {
  schema_version: 1,
  method: 'Current application-manifest modules resolved with Acorn; locale literals and CSS parsed without executing reference JavaScript.',
  source_modules: [66517, 24988, 13784, 45190, 13476, 90516, 57230, 16230, 27588].map(module => ({module, ...source.receipt(module, source.module(module).fn)})),
  form: {inputs, submit_predicate: source.receipt(24988, validator), profile_defaults: source.receipt(45190, source.binding(45190, 'd')), game_limit: 10, game_file_types: ['exe', 'url']},
  service, locales, css, device_menus: deviceMenus,
  native: {page: 'src/shell/armory_page.rs', dialog: 'src/shell/armory_page/share_profile.rs',
    entry: 'Explicit Share command on an existing local device/profile passes its snapshot to AppShell and ArmoryPage::open_share_profile.',
    source_gates: 'Current PID 182 and 653 require installed Armory, non-guest account, profile-sharing feature and supported category; shared and maintenance states disable Share. The explicit local draft entry does not claim these remote gates passed. Direct access to implemented local modules follows the user requirement.',
    retained_data: ['actual device', 'actual profile selection', 'title', 'description', 'up to ten user-selected game paths'],
    snapshot_semantics: 'The modal owns the captured local snapshot. It never edits the originating device or adds a remote contribution card.',
    preview: '90516 hideProfileInfo branch is represented by a local profile/device summary and available mouse DPI values. Native device artwork, key bindings and other source detail sections are still incomplete.',
    file_size: 'Actual UTF-8 serialized local Profile snapshot bytes, explicitly labelled Local draft; not a fabricated remote contribution size.',
    submission: 'Valid local fields enable a command that reports unavailable and retains the draft. No account, moderation result, network request, upload progress or success is synthesized.',
    games: 'Direct native browsing follows the source exe/url selection boundary; installed-game scanning and the source intermediate chooser are not implemented.',
    focus: 'GPUI Base Dialog/DialogPopup own Escape and focus isolation; closing returns to the Armory page.',
    guest_root: 'Browse/My Downloads remain the settled guest navigation. No My Uploads feature/account flag is fabricated.'},
  verification: 'Static source checks and formatting only. Runtime pixels, IME/paste boundaries, native file dialogs and focus restoration are unverified because application/build/test execution is prohibited.',
};
const markdown = [
  '# Armory 本地配置分享与详情预览', '',
  '2026-10-04。产品配置文件菜单中的“分享”现在可以把已有设备与当前配置的快照交给 Armory，',
  '打开实际可编辑的本地分享草稿。空的 Browse/My Downloads 不添加示例贡献；guest 的 My Uploads',
  '仍保持隐藏。入口是明确的本地草稿能力，不意味着账户、远端分享或内容服务已经接通。', '',
  '## 当前源码', '',
  '- 66517 挂载分享页面；24988 定义 modal 和提交条件；13784 定义完整表单。',
  '- PROFILE 提交条件要求设备和配置索引均非 -1、标题 trim 后非空且 hasToxicContent 为假。',
  '  标题最大 32、描述最大 500 个 JavaScript UTF-16 单位，描述可选。',
  '- 13784 选中设备后挂载 90516，使用 `hideProfileInfo=true`；该预览使用所选配置内容，',
  '  不需要伪造服务器贡献作者、UUID、点赞或下载统计。57230 是 DPI 详情模块。',
  '- 支持游戏最多 10 项；16230 的 Browse 使用 `.exe` / `.url` 多文件选择。',
  '- 9728 的真实服务为 POST `/contributions` 和 POST `/toxicity/validate`。本地实现不调用它们。', '',
  '源码表单使用 800px 内容、最大 1020px modal、36px 标题、27px 提交按钮、23px 标题输入和',
  '91px 描述框。当前 10 种语言的表单文案已逐项与现有 locale 比较，没有改用自行翻译的替代内容。', '',
  'PID 182/653 的当前 profile 类分别为 lD/UD，菜单 `s1I` 均解析为 `SHARE_TO_WORKSHOP`。',
  '两者 `openArmoryEditView` 都传递 selectedProfileGuid/productId，以 `policy=3,tab_visible=1` 打开',
  '`/synapse/armory/?view=my-upload&sharePopup=true`，已有目标则广播并激活。支持类别均为',
  'KEYBOARD、MOUSE、AUDIO、SYSTEM、BROADCASTER、MOUSEPLUSMAT；常量、方法和当前 manifest 均有静态收据。', '',
  '官方菜单还要求已安装 Armory、非 guest、开启 profile-sharing feature；已分享和维护状态会禁用分享。',
  '原生入口遵照用户直接访问已实现模块的要求，开放明确的本地草稿，并不表示远端账户或特性门槛已满足。',
  '入口只添加到已经完整实现 profile more-menu 的 PID 182/653，其余设备未拼接局部替代菜单。', '',
  '## 已实现分支', '',
  '分享草稿保留传入快照，预选当前配置，标题从空值开始。用户可选择该设备已有配置、输入标题和',
  '描述、查看 UTF-16 计数、添加或移除游戏文件。单配置设备不进入可分享选择；没有真实配置时',
  '提交保持禁用。游戏文件只保留路径与名称，不执行、不上传文件。', '',
  '本地字段满足条件后提交会显示“分享服务未连接”，保留输入；不会把未执行的在线内容检测写成',
  '已通过，也不会制造加载进度或成功状态。文件选择等待来自真实的系统选择器。', '',
  '下方详情预览显示真实设备、配置名及已有鼠标 DPI 参数。文件大小明确标为“本地草稿”，计算的是',
  '当前本地 Profile 快照的 UTF-8 JSON 字节，不使用源示例 contribution 或硬编码的 3KB。',
  'Base Dialog/DialogPopup 承担模态、Escape、焦点隔离和面板点击边界；关闭回到 Armory。', '',
  '## 尚未完成', '',
  '完整远端卡片、远端详情、发布后的作者/点赞/下载、账户与维护状态、在线内容检测及上传仍未实现。',
  '本地详情目前是可达的配置摘要/DPI 预览；原生产品图、键位映射、音频等完整 90516 子区尚未补齐。',
  '游戏添加直接走源 Browse 对应的系统文件选择；安装游戏扫描、源中间选择页和拖动排序尚未实现。',
  '本地快照格式不等同于官方 contribution 上传包；草稿不写回设备或发布到服务器。', '',
  '## 校验', '',
  '`node tools/audit-armory-share.cjs --check` 只解析与比较证据，不写文件。',
  '生成证据使用 `node tools/audit-armory-share.cjs`；默认根审计由 `audit-armory-default.cjs` 保持。',
  '静态 AST、CSS、现有 locale 和原生接线收据见 [armory-share-current-evidence.json](armory-share-current-evidence.json)。',
  '未运行应用、构建、测试、下载的 JavaScript 或 DLL。像素、IME/粘贴边界、系统文件选择器和',
  '焦点恢复仍需后续获准运行验证；本审计不把编译或片段检查当作运行等价证明。', '',
].join('\n');
const outputs = [['docs/re/armory-share-current-evidence.json', JSON.stringify(report, null, 2) + '\n'], ['docs/re/armory-share-current-audit.md', markdown]];
if (process.argv.includes('--check')) {
  for (const [file, expected] of outputs) requireFact(read(file) === expected, `Stale Armory share evidence: ${file}`);
  console.log('Armory share: current form/detail/service evidence and reachable local draft verified; no files written.');
} else {
  for (const [file, value] of outputs) fs.writeFileSync(path.join(root, file), value);
  console.log('Armory share: generated current evidence and audit.');
}
