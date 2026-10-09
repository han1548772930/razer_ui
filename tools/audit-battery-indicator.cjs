function tipKeys(state) {
  const cases = state.cases;
  // 每条分支的第二/第三段是 `ra.oJ` 这类「导出表.键」引用；先取键，再解析常量字符串。
  const byKey = (sym, key) => {
    const m = new RegExp(`${key}:\\(\\)=>([\\w$]+)`).exec(state.text);
    if (!m) throw new Error(`export map entry ${key} not found`);
    const str = new RegExp(`[,;{]\\s*${m[1]}\\s*=\\s*"([A-Z0-9_]+)"`).exec(state.text);
    if (!str) throw new Error(`tip constant for ${key} not found`);
    return str[1];
  };
  return {
    off: byKey(cases.off[0], cases.off[1]),
    chargedFull: byKey(cases.chargedFull[0], cases.chargedFull[1]),
    charging: byKey(cases.charging[0], cases.charging[1]),
    percentNoCharge: byKey(cases.percentNoCharge[1], cases.percentNoCharge[2]),
    percentPaused: byKey(cases.percentPaused[0], cases.percentPaused[1]),
    percentDefault: byKey(cases.percentDefault[0], cases.percentDefault[1]),
  };
}

//! 设备页电量图标的依据审计。
//!
//! 从当前源码里**重新抽取**状态机、CSS、文案 key 与图标文件名，然后断言：
//! 1. 每个图标都能在 `assets/synapse/` 找到对应的打包文件；
//! 2. `crates/razer-widgets/src/battery.rs` 里的类名/图标表与抽取结果一致；
//! 3. 文案 key 在语言包 `locales/zh-CN.json` 里有译文。
//!
//! 只做静态解析，不执行任何下载的 JavaScript。
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const root = path.resolve(__dirname, '..');
const sha256 = b => crypto.createHash('sha256').update(b).digest('hex');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');

// 状态机只在少数包里有；面板与设备页用的是同一份共享实现。
const STATE_BUNDLES = [
  '.ref/devices/104/static/js/main.9b94769e.js',
  '.ref/devices/105/static/js/main.ef834389.js',
];
const CSS_BUNDLE = '.ref/devices/100/static/css/main.dd229426.css';
const ZH_CHUNK = '.ref/applications/synapse/dashboard/static/js/trans-zh-CN.9295e59f.chunk.js';
const RUST = 'crates/razer-widgets/src/battery.rs';

function findStateBundle() {
  for (const rel of STATE_BUNDLES) {
    const abs = path.join(root, rel);
    if (!fs.existsSync(abs)) continue;
    const text = fs.readFileSync(abs, 'utf8');
    const start = text.indexOf('switch(e.chargingStatus){');
    if (start < 0) continue;
    const end = text.indexOf('}return{battName:', start);
    if (end < 0) continue;
    const body = text.slice(start, end);
    const patterns = {
      off: /case"off":\w+="batt batt-off",\w+=([\w$]+)\.(\w+);/,
      chargedFull: /a="batt charging100",\w+=([\w$]+)\.(\w+)/,
      charging: /a="batt charging",\w+=([\w$]+)\.(\w+)/,
      percentNoCharge: /case"NoCharge_BatteryFull":\w+=([\w$]+)\((?:[\w$]+)\.level\),\w+=([\w$]+)\.(\w+)/,
      warning: /case"batt-warning":\w+="batt batt-warning",[^;]*?([\w$]+)\.(\w+);/,
      percentPaused: /PAUSED_CHARGING:\w+=[\w$]+\((?:[\w$]+)\.level,!0\),\w+=([\w$]+)\.(\w+)/,
      percentDefault: /default:\w+=[\w$]+\(100\),\w+=([\w$]+)\.(\w+)/,
    };
    const found = {};
    for (const [name, re] of Object.entries(patterns)) {
      const m = re.exec(body);
      if (!m) { found.missing = name; break; }
      found[name] = m.slice(1);
    }
    if (found.missing) throw new Error(`state machine branch missing in ${rel}: ${found.missing}`);
    const bucket = /const \w+=\(e=>\{let a=0;return a=10===e\?10:e>10&&e<20\?20:10\*Math\.floor\(e\/10\),a\}\)/.exec(text);
    if (!bucket) throw new Error(`bucket formula not found in ${rel}`);
    const disconnected = /if\(!e\|\|e<=0\)return"batt batt-disconnected"/.exec(text);
    if (!disconnected) throw new Error(`disconnected branch not found in ${rel}`);
    return { rel, text, offset: start, cases: found, bucketAt: bucket.index };
  }
  throw new Error('battery state machine not found');
}

function cssRules() {
  const text = read(CSS_BUNDLE);


  const grab = re => {
    const m = re.exec(text);
    if (!m) throw new Error(`css rule missing: ${re}`);
    return m[0];
  };
  const icons = {};
  for (const m of text.matchAll(/\.batt\.batt-(\d+)(?:\.paused)?\{background-image:url\(([^)]+)\)\}/g)) {
    (icons[m[1]] ||= []).push(m[2].replace('../../static/media/', ''));
  }
  for (const m of text.matchAll(/\.batt\.(charging100|charging|batt-disconnected|batt-off|batt-warning)\{background-image:url\(([^)]+)\)\}/g)) {
    icons[m[1]] = m[2].replace('../../static/media/', '').replace(/#\d+$/, '');
  }
  return {
    container: grab(/\.right \.battery\{[^}]*\}/),
    lowBatt: grab(/\.battery \.low-batt\{[^}]*\}/),
    iconBox: grab(/\.nav-tabs \.batt,[^{]*\{background-position:50%;background-repeat:no-repeat;background-size:20px;height:26px;margin:0 10px;width:26px\}/),
    iconDefault: grab(/\.nav-tabs \.batt\{background-image:url\([^)]*\);margin:0 5px\}/),
    // Historical attribute rule is not mounted by the current battery node.
    unusedAttributeTooltip: grab(/\.nav-tabs \.batt\.batt-warning\[tooltip\]:before\{[^}]*\}/),
    icons,
  };
}

function packagedIcons() {
  const rust = read(RUST);
  const used = [...rust.matchAll(/"(synapse\/battery[^"]+\.svg)"/g)].map(m => m[1]);
  const missing = used.filter(rel => !fs.existsSync(path.join(root, 'assets', rel.replace(/^synapse\//, 'synapse/'))));
  return { used: [...new Set(used)].sort(), missing };
}

// `hideBattValue` 是产品工作区里写死的 prop（例如 1330 的
// `.ref/devices/1330/static/js/main.f0797abf.js`：`hideBattValue:!0`），不是设备数据。
// 这里按当前包重算 `!0` 的产品集合，并与 `crates/razer-widgets/src/battery.rs` 的 `HIDE_BATTERY_VALUE` 表
// 逐项比对；同时核对隐藏分支的本地标记。
function hideBatteryValue() {
  const devices = path.join(root, '.ref/devices');
  const products = [];
  for (const name of fs.readdirSync(devices).sort((a, b) => Number(a) - Number(b))) {
    const jsDir = path.join(devices, name, 'static', 'js');
    if (!fs.existsSync(jsDir)) continue;
    let hides = false;
    for (const file of fs.readdirSync(jsDir)) {
      if (!file.endsWith('.js')) continue;
      const text = fs.readFileSync(path.join(jsDir, file), 'utf8');
      if (/hideBattValue:!0/.test(text)) {
        hides = true;
        break;
      }
    }
    if (hides) products.push(Number(name));
  }
  const rust = read(RUST);
  const table = /const HIDE_BATTERY_VALUE: &\[u32\] = &\[([^\]]*)\]/.exec(rust.replace(/\r\n/g, '\n'));
  if (!table) throw new Error('HIDE_BATTERY_VALUE table not found in battery.rs');
  const declared = table[1]
    .split(',')
    .map(part => part.trim())
    .filter(Boolean)
    .map(Number);
  return { products, declared, rust_markers: [
    '.mr(surface::css(17.))',
    'hideBattValue',
  ].filter(marker => rust.includes(marker)) };
}

function translations(keys) {
  const zh = JSON.parse(read('locales/zh-CN.json'));
  const en = JSON.parse(read('locales/en.json'));
  return Object.fromEntries(
    Object.entries(keys).map(([name, key]) => [name, { key, zh: zh[key] ?? null, en: en[key] ?? null }]),
  );
}

const state = findStateBundle();
const keys = tipKeys(state);
const css = cssRules();
const packaged = packagedIcons();
const strings = translations(keys);
const rust = read(RUST);

const problems = [];
// 顶栏右侧那一组：`.nav-tabs .right{flex:1 1 25%}`、
// `.nav-tabs .help{height:24px;width:24px;border-radius:5px;margin-right:10px}`、
// `.right .battery{height:46px;color:#ccc;font-size:14px}`。
{
  const cssText = read(CSS_BUNDLE);
  const rules = new Map();
  for (const m of cssText.matchAll(/([^{}]+)\{([^}]*)\}/g)) {
    const sel = m[1].replace(/\s+/g, ' ').trim();
    const decl = {};
    for (const part of m[2].split(';')) {
      const k = part.indexOf(':');
      if (k > 0) decl[part.slice(0, k).trim()] = part.slice(k + 1).trim();
    }
    rules.set(sel, {...rules.get(sel), ...decl});
  }
  const right = rules.get('.nav-tabs .right');
  const help = rules.get('.nav-tabs .help');
  const batteryRow = rules.get('.right .battery');
  if (!right) problems.push('CSS 缺少 .nav-tabs .right');
  if (!help) problems.push('CSS 缺少 .nav-tabs .help');
  if (!batteryRow) problems.push('CSS 缺少 .right .battery');
  if (help) {
    if (Number(/(\d+)px/.exec(help.height || '')?.[1]) !== 24) problems.push('.help 高度应为 24px');
    if (Number(/(\d+)px/.exec(help.width || '')?.[1]) !== 24) problems.push('.help 宽度应为 24px');
    if (Number(/(\d+)px/.exec(help['margin-right'] || '')?.[1]) !== 10) problems.push('.help 右边距应为 10px');
  }
  if (batteryRow) {
    if (Number(/(\d+)px/.exec(batteryRow.height || '')?.[1]) !== 46) problems.push('.right .battery 高度应为 46px');
    if (batteryRow.color !== '#ccc') problems.push('.right .battery 文字色应为 #ccc');
    if (batteryRow['font-size'] !== '14px') problems.push('.right .battery 字号应为 14px');
  }
  // 本地帮助按钮在设备页工具栏里：`asset_button("device-help", …help…).size(css(24.)).mr(css(10.))`
  const workspaceSrc = read('crates/razer-pages/src/features/workspace.rs');
  if (!/asset_button\(\s*"device-help"/.test(workspaceSrc) || !workspaceSrc.includes('help-active.svg'))
    problems.push('本地设备页应有帮助按钮（help-default/help-active）');
  if (!/\.size\(surface::css\(24\.\)\)\s*\n?\s*\.mr\(surface::css\(10\.\)\)/.test(workspaceSrc))
    problems.push('本地帮助按钮应为 24px 且右边距 10px');
  if (!rust.includes('46.')) problems.push('本地电量行应为 46px');
  }
for (const [name, value] of Object.entries(strings)) {
  if (value.zh === null) problems.push(`语言包缺少 ${name} (${value.key})`);
}
if (packaged.missing.length) problems.push(`打包缺失: ${packaged.missing.join(', ')}`);
// `hideBattValue:!0` 的产品表必须与当前包一致，隐藏分支必须按 `.hideBattValue{margin-right:17px}`
// 与「不渲染百分比、不挂载提示」实现。
const hidden = hideBatteryValue();
if (hidden.products.join(',') !== hidden.declared.join(','))
  problems.push(`HIDE_BATTERY_VALUE 表与当前源不一致：源 ${hidden.products.join(',')} / 本地 ${hidden.declared.join(',')}`);
for (const marker of ['.mr(surface::css(17.))', 'hideBattValue'])
  if (!hidden.rust_markers.includes(marker)) problems.push(`battery.rs 缺少隐藏分支标记 ${marker}`);
if (!/\.hideBattValue\{margin-right:17px\}/.test(read(CSS_BUNDLE)))
  problems.push('.hideBattValue{margin-right:17px} 不在当前 CSS 里');
if (hidden.declared.some((id, ix) => hidden.products[ix] !== id))
  problems.push('HIDE_BATTERY_VALUE 必须按升序列出当前源里的产品');
if (/source_tooltip\.rs/.test('')) problems.push('unreachable');
// Rust 表里必须逐条出现原版类名。
for (const cls of ['batt batt-off', 'batt charging', 'batt charging100', 'batt batt-warning', 'batt batt-disconnected']) {
  if (!rust.includes(`"${cls}"`)) problems.push(`battery.rs 缺少类名 ${cls}`);
}
for (const level of [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 100]) {
  if (!rust.includes(`batt batt-${level}`)) problems.push(`battery.rs 缺少 batt-${level}`);
  if (!rust.includes(`synapse/battery-paused-${level}.svg`)) problems.push(`battery.rs 缺少暂停档 ${level}`);
}
// CSS 声明的暂停片段必须都已裁成独立文件。
const pausedSheets = Object.values(css.icons).flat().filter(v => v.startsWith('icon_battery_paused'));
if (!pausedSheets.length) problems.push('CSS 未声明暂停档图标');


const report = {
  schema_version: 2,
  scanner_sha256: sha256(fs.readFileSync(__filename)),
  state_machine: {
    source: state.rel,
    offset: state.offset,
    sha256: sha256(fs.readFileSync(path.join(root, state.rel))),
    cases: {
      off: 'batt batt-off',
      charging: 'batt charging',
      charging_full: 'batt charging100 (level >= 99)',
      no_charge_battery_full: 'bucket(level)',
      warning: 'batt batt-warning',
      paused: 'bucket(level, paused)  // ReachChargingLimit',
      default: 'bucket(100); BATTERY_PERCENT interpolates actual level',
    },
    bucket: '10 === level ? 10 : (level > 10 && level < 20) ? 20 : 10 * floor(level / 10)',
    disconnected: 'level <= 0 → batt batt-disconnected',
  },
  css: { source: CSS_BUNDLE, ...css },
  strings,
  icons: {
    referenced_by_css: css.icons,
    packaged_by_rust: packaged.used,
    missing: packaged.missing,
    paused_crops: Object.keys(css.icons)
      .filter(k => /^\d+$/.test(k))
      .map(k => `assets/synapse/battery-paused-${k}.svg`)
      .filter(rel => fs.existsSync(path.join(root, rel))),
  },
  rust_table: { source: RUST, sha256: sha256(fs.readFileSync(path.join(root, RUST))) },
  hide_battery_value: {
    rule: '产品工作区传 hideBattValue:!0 → 不渲染百分比 span、不挂载 tooltip，并加 .hideBattValue{margin-right:17px}',
    products_in_source: hidden.products,
    rust_table: hidden.declared,
    rust_markers: hidden.rust_markers,
  },
  mounted_header_audit: 'docs/re/device-tabs-audit.json',
  limitations: ['Icon/string table verification only; current parent layout and mounted tooltip are checked by audit-device-tabs.cjs.',
    'No rendered pixel or hardware behavior validation.'],
  problems,
};

const jsonPath = path.join(root, 'docs/re/battery-indicator-audit.json');
const rendered = JSON.stringify(report, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (fs.readFileSync(jsonPath, 'utf8') !== rendered) problems.push('Stored battery audit is stale');
} else fs.writeFileSync(jsonPath, rendered);
console.log(`battery indicator: ${packaged.used.length} icons in Rust table, ${problems.length} problems`);
if (process.argv.includes('--check') && problems.length) {
  for (const p of problems) console.error('  ' + p);
  process.exit(1);
}
