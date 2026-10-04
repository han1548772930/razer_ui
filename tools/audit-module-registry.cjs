//! 模块注册表（module → 窗口/URL/打开参数）依据审计。
//!
//! 源码：`.ref/applications/rz-app-menu/static/js/main.83ced465.js`
//! - `n = {sameWindow:"policy=3", diffWindow:"policy=5", …}` 打开参数枚举
//! - `D = [{moduleName, moduleNameTranslation, windowName, url, openParam}, …]` 模块表
//! - `C = "policy=5,tab_visible=0,app_name=synapse,width=1280,…"` 新建窗口策略串
//!
//! 断言本地模块目录的每一行都有源码依据：`linkedGames→profiles`、`macro`、`alexa`、
//! `feedback→feedback-synapse`、`syn3-profile-migration`、`armory` 的窗口名与 URL，
//! 以及本地把已实现的模块改为直接打开。
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const root = path.resolve(__dirname, '..');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const sha256 = b => crypto.createHash('sha256').update(b).digest('hex');

const SRC = '.ref/applications/rz-app-menu/static/js/main.83ced465.js';
const js = read(SRC);
const shell = read('src/shell.rs');
const service = read('src/shell/service_pages.rs');
const pickerHost = read('src/shell/app_picker_host.rs');
const hostTabs = read('src/shell/host_tabs.rs');
const problems = [];

// 1) 打开参数枚举。
const enumMatch = /\{sameWindow:"([^"]+)",diffWindow:"([^"]+)",diffWindowSingleProcess:"([^"]+)",tabVisible:"([^"]+)",tabInvisible:"([^"]+)",windowVisible:"([^"]+)",windowInvisible:"([^"]+)",autoFocus:"([^"]+)"/.exec(js);
if (!enumMatch) problems.push('打开参数枚举未找到');
const openParams = enumMatch
  ? {
      sameWindow: enumMatch[1],
      diffWindow: enumMatch[2],
      diffWindowSingleProcess: enumMatch[3],
      tabVisible: enumMatch[4],
      tabInvisible: enumMatch[5],
      windowVisible: enumMatch[6],
      windowInvisible: enumMatch[7],
      autoFocus: enumMatch[8],
    }
  : null;

// 2) 窗口策略串。
const policy = /C="(policy=5[^"]+)"/.exec(js);
if (!policy) problems.push('Synapse 新建窗口策略串未找到');

// 3) 模块表。
const start = js.indexOf('[{moduleName:"linkedGames"');
let end = -1;
if (start >= 0) {
  let depth = 0;
  for (let i = start; i < js.length; i += 1) {
    if (js[i] === '[') depth += 1;
    else if (js[i] === ']') {
      depth -= 1;
      if (depth === 0) {
        end = i;
        break;
      }
    }
  }
}
if (start < 0 || end < 0) problems.push('模块表未找到');
const modules = [];
if (start >= 0 && end > start) {
  const arrayText = js.slice(start, end + 1);
  for (const m of arrayText.matchAll(/\{moduleName:"([^"]+)",moduleNameTranslation:[\w.$]+,windowName:(?:"([^"]+)"|null),url:(?:"([^"]*)"|`([^`]*)`|null)(?:,openParam:\[([^\]]*)\])?\}/g)) {
    const url = m[3] !== undefined ? m[3] : m[4] !== undefined ? m[4].replace('${encodeURIComponent("/synapse/dashboard")}', '%2Fsynapse%2Fdashboard') : null;
    modules.push({
      module: m[1],
      window: m[2] || null,
      url,
      open_param: m[5] ? m[5].split(',').map(v => v.replace(/^T\.ZP\./, '').trim()) : [],
    });
  }
}
if (modules.length !== 7) problems.push(`模块表条目应为 7，实际 ${modules.length}`);

// 4) 关键条目核对。
const byModule = new Map(modules.map(m => [m.module, m]));
const expect = [
  ['linkedGames', 'profiles', '/synapse/profiles/'],
  ['macro', 'macro', '/synapse/macro/'],
  ['alexa', 'alexa', '/synapse/alexa/'],
  ['armory', 'armory', '/synapse/armory/'],
  ['syn3-profile-migration', 'syn3-profile-migration', '/profile-migration/'],
  ['feedback', 'feedback-synapse', '/feedback/?app=synapse&path=%2Fsynapse%2Fdashboard'],
];
for (const [name, window, url] of expect) {
  const entry = byModule.get(name);
  if (!entry) problems.push(`模块表缺少 ${name}`);
  else {
    if (entry.window !== window) problems.push(`${name} 窗口名应为 ${window}，实际 ${entry.window}`);
    if (entry.url !== url) problems.push(`${name} URL 应为 ${url}，实际 ${entry.url}`);
  }
}
if (byModule.get('add-wifi-device')?.url !== null) problems.push('add-wifi-device 的 url 应为 null');

// 5) 本地模块目录：已实现的模块必须直接打开（不再显示下载/安装门控）。
const rows = {};
for (const m of service.matchAll(/id: "([\w-]+)",[\s\S]{0,1200}?native_page: (Some\(ModulePage::(\w+)[^)]*\)|None)/g)) {
  rows[m[1]] = m[3] || 'None';
}
const direct = { alexa: 'Alexa', macro: 'Macro', armory: 'Armory', feedback: 'Feedback', tour: 'IntroductionTour' };
for (const [id, kind] of Object.entries(direct)) {
  if (!rows[id] || rows[id] === 'None') problems.push(`模块目录 ${id} 应直接打开，实际 ${rows[id]}`);
  else if (!rows[id].startsWith(kind)) problems.push(`模块目录 ${id} 应指向 ${kind}，实际 ${rows[id]}`);
}
if (rows['profile-migration'] === 'None') problems.push('模块目录 profile-migration 应直接打开');
// linked-games 打开 profiles 窗口；feedback 当前应用源码已取得并接上独立根。
if (!shell.includes('Location::Profiles')) problems.push('本地缺少 Location::Profiles（linkedGames 的窗口）');
if (!service.includes('ModulePage::Profiles')) problems.push('linked-games 行未指向 Profiles 页');

// The host App Picker is another module entry point. Once a page is compiled
// locally, it must be visible as bundled/launchable and route to the same named
// window instead of falling through to the unavailable-service message.
const localPickerModules = ['Alexa', 'Macro', 'LinkedGames', 'Armory'];
for (const method of ['bundled_modules', 'launchable_modules']) {
  const list = new RegExp(`\\.${method}\\(\\[([\\s\\S]*?)\\]\\)`).exec(pickerHost)?.[1] || '';
  for (const key of localPickerModules) {
    if (!list.includes(`PickerModule::${key}`)) problems.push(`App Picker ${method} 缺少 ${key}`);
  }
}
for (const route of [
  'open_module_tab(service_pages::ModulePage::Alexa',
  'open_module_tab(service_pages::ModulePage::Profiles',
  'open_module_tab(service_pages::ModulePage::Macro',
  'open_module_tab(service_pages::ModulePage::Armory',
]) {
  if (!pickerHost.includes(route)) problems.push(`App Picker 缺少直接打开路由 ${route}`);
}

// policy=3 modules attach to the current host; their names are tab identities.
for (const [kind, name] of [['Alexa','alexa'], ['Macro','macro'], ['Armory','armory'], ['Profiles','profiles'], ['Feedback','feedback-synapse']]) {
  if (!shell.includes(`service_pages::ModulePage::${kind} => Location::${kind}`)) problems.push(`Missing host-tab module route: ${kind}`);
  if (!hostTabs.includes(`Self::${kind} => "${name}".into()`)) problems.push(`Missing host-tab identity: ${name}`);
}

const report = {
  schema_version: 1,
  scanner_sha256: sha256(fs.readFileSync(__filename)),
  source: { path: SRC, sha256: sha256(fs.readFileSync(path.join(root, SRC))) },
  open_params: openParams,
  synapse_window_policy: policy ? policy[1] : null,
  modules,
  local_module_rows: rows,
  local_picker_modules: localPickerModules,
  problems,
};
fs.writeFileSync(path.join(root, 'docs/re/module-registry-audit.json'), JSON.stringify(report, null, 2) + '\n');
console.log(
  `module registry: modules=${modules.length} rows=${Object.keys(rows).length} problems=${problems.length}`,
);
if (process.argv.includes('--check') && problems.length) {
  for (const p of problems) console.error('  ' + p);
  process.exit(1);
}
