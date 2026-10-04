// Current mounted navigation and scoped local contracts. No reference execution.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const jsFile = '.ref/devices/182/static/js/main.db20a7c4.js';
const cssFile = '.ref/devices/182/static/css/main.48c20423.css';
const js = read(jsFile), cssText = read(cssFile);
const prop = (node, name) => node?.properties?.find(p => p.type === 'Property' && key(p.key) === name)?.value;
const args = node => node?.type === 'CallExpression' ? node.arguments[1] : null;
const candidates = [];
walk(acorn.parse(js, {ecmaVersion: 'latest'}), node => {
  if (node.type !== 'ObjectExpression') return;
  const children = prop(node, 'children');
  if (children?.type !== 'ArrayExpression' || children.elements.length !== 3) return;
  const names = children.elements.map(child => prop(args(child), 'className')?.value);
  if (names.join(',') === 'profile-wrapper,navs-wrapper,right') candidates.push(node);
});
if (candidates.length !== 1) throw Error(`Ambiguous mounted header: ${candidates.length}`);
const header = candidates[0], children = prop(header, 'children').elements;
const receipt = node => ({path: jsFile, sha256: hash(js), start: node.start, end: node.end, text: js.slice(node.start, node.end)});
const center = args(children[1]), right = args(children[2]);
const batteryNodes = [];
walk(right, node => {if (node.type === 'ObjectExpression' && prop(node, 'id')?.value === 'battery-level-tips') batteryNodes.push(node);});
if (batteryNodes.length !== 1) throw Error('Battery mount is not unique');
if (/nav back|nav forward/.test(js.slice(center.start, center.end))) throw Error('Unexpected mounted arrows');
// Preserve every rule; later exact selectors extend earlier declarations.
// These explicitly scoped rules do not claim to implement a general CSS engine.
const css = [];
for (const m of cssText.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
  if (/nav-tabs|\.nav\.|\.profile-act|\.hover-border|\.tooltip-razer|\.right \.battery|\.battery \.low-batt/.test(m[1]))
    css.push({path: cssFile, sha256: hash(cssText), offset: m.index, selector: m[1], declarations: m[2]});
}
const rule = selector => Object.assign({}, ...css.filter(r => r.selector.split(',').includes(selector)).map(r =>
  Object.fromEntries(r.declarations.split(';').filter(Boolean).map(pair => {const i=pair.indexOf(':');return [pair.slice(0,i), pair.slice(i+1)];}))));
const rules = Object.fromEntries(['.nav-tabs .nav', '.nav-tabs .nav:hover', '.nav-tabs .nav.active:hover',
  '.nav-tabs .profile-wrapper', '.nav-tabs .navs-wrapper', '.nav-tabs .right', '.nav-tabs .batt',
  '.nav-tabs .help', '.right .battery', '.tooltip-razer>.main', '.tooltip-razer>.main>.wrapper'].map(selector => [selector, rule(selector)]));
if (rules['.nav-tabs .right'].flex !== '1 1 25%' || rules['.nav-tabs .right']['justify-content'] !== 'flex-end') throw Error('Right column cascade changed');
if (rules['.nav-tabs .navs-wrapper'].flex !== '1 0 max-content') throw Error('Center column cascade changed');
if (rules['.nav-tabs .batt'].margin !== '0 5px') throw Error('Battery cascade changed');
// Lexically delimit specific Rust functions; Rustc checks their types.
function body(file, marker) {
  const text=read(file), start=text.indexOf(marker);
  if(start<0)throw Error(`Missing ${file}:${marker}`);
  const mask=text.replace(/"(?:\\.|[^"\\])*"|\/\/[^\n]*|\/\*[\s\S]*?\*\//g,m=>' '.repeat(m.length));
  const open=mask.indexOf('{',start);let depth=0;
  for(let i=open;i<mask.length;i++){if(mask[i]==='{')depth++;if(mask[i]==='}'&&!--depth)return text.slice(start,i+1);}
  throw Error(`Unclosed Rust scope ${file}:${marker}`);
}
const problems=[];
const expect=(ok, message)=>{if(!ok)problems.push(message);};
const consumers = [
  ['src/features/workspace.rs','fn toolbar(', 'device-navigation-right'],
  ['src/features/source_workspace.rs','impl Render for SourceProductWorkspace', 'source-navigation-right'],
];
for(const [file, marker, id] of consumers){
  const scoped=body(file,marker);
  expect(/surface::nav_right\(\)[\s\S]*?\.children\(crate::ui::battery::element\(&self.device, cx\)\)[\s\S]*?surface::asset_button/.test(scoped), `${file}: battery/help must share nav_right`);
  expect(scoped.includes(`"${id}"`),`${file}: missing right container identity`);
  expect(scoped.includes('surface::split_navs'),`${file}: missing measured overflow`);
  expect(!/device-tab-back|device-tab-forward|nav_arrow_button/.test(scoped),`${file}: obsolete arrows remain`);
}
const surface='src/ui/surface.rs';
expect(body(surface,'pub(crate) fn navigation_button(').includes('-> gpui_kit::base::Button'), 'Navigation hover must use Base Button');
expect(body(surface,'pub(crate) fn nav_overflow(').includes('gpui_kit::base::Button::new'), 'Overflow hover must use Base Button');
expect(body('src/features/profile.rs','fn profile_dialog_button(').includes('-> BaseButton'), 'Dialog hover must use Base Button');
expect(body(surface,'pub(crate) fn nav_left(').includes('relative(0.25)'), 'Missing left basis');
expect(body(surface,'pub(crate) fn nav_right(').includes('relative(0.25)'), 'Missing right basis');
expect(body(surface,'fn label_width(').replace(/\s+/g,'').includes('16./f32::from(window.rem_size())'), 'Mixed physical/CSS pixels');
expect(body('src/shell.rs','fn move_history(').includes('device.step_page_history'), 'Shell must dispatch product history');
expect(body('src/shell.rs','fn toolbar(').includes('self.history_target(false, cx).is_some()') &&
  body('src/shell.rs','fn move_history(').includes('self.history_target(forward, cx)'), 'History presentation and activation must share target resolution');
expect(body('src/shell.rs','fn history_target(').includes('index.map(HistoryTarget::Shell)'), 'History boundary must retain shell navigation');
expect(body('src/shell.rs','fn history_target(').includes('page.read(cx).history_blocked()'), 'Modal lock must not fall through to shell navigation');
expect(body(surface,'pub(crate) fn history_button(').includes('gpui_kit::base::Button::new'), 'History must use direct Base activation');
expect(body('src/shell.rs','fn main_page(').includes('surface::nav_overflow'), 'Main navigation must handle overflow');
const battery=body('src/ui/battery.rs','pub(crate) fn element(');
expect(battery.includes('!device.has_battery'), 'Missing hasBattery guard');
expect(battery.includes('SourceTooltipKind::Battery'), 'Wrong battery tooltip implementation');
expect(!battery.includes('352.'), 'Unmatched attribute tooltip width must not be used');
// Actual Dashboard navigation → module-local exports → all application locales.
const dashboard=new Source('synapse/dashboard'), main=[];
const toolbar = dashboard.binding(96776, 'We');
const historyKeys = ['IUp', 'OXp'].map(name => ({
  export: name, key: dashboard.literal(54693, dashboard.exported(54693, name)),
  receipt: dashboard.receipt(54693, dashboard.exported(54693, name)),
}));
expect(historyKeys.map(item => item.key).join(',') === 'BACK,FORWARD', 'Toolbar translation keys changed');
const localHistory = body(surface, 'pub(crate) fn history_button(');
for (const item of historyKeys) expect(localHistory.includes(`"${item.key}"`), `Wrong toolbar key: ${item.key}`);
for (const kind of ['Device', 'Profiles', 'Alexa', 'Macro', 'Armory']) {
  expect(body('src/shell.rs', 'fn history_target(').includes(`HistoryTarget::${kind}`), `Missing ${kind} history target`);
  expect(body('src/shell.rs', 'fn move_history(').includes(`HistoryTarget::${kind}`), `Missing ${kind} history activation`);
}
walk(dashboard.binding(35378,'x'), node=>{
  if(node.type==='ArrayExpression' && node.elements.length===4 && node.elements.every(e=>prop(e,'name') && prop(e,'id')))main.push(node);
});
if(main.length!==1)throw Error(`Dashboard navigation ambiguous: ${main.length}`);
const mainKeys=main[0].elements.map(e=>dashboard.literal(35378,prop(e,'name')));
for(const k of mainKeys)expect(read('src/nav.rs').includes(`"${k}"`),`Missing mounted Dashboard key ${k}`);
const locales=[];
for(const file of dashboard.files.filter(f=>/\/trans-[^.]+\.[a-f0-9]+\.chunk\.js$/.test(f))){
  dashboard.parse(file);const modules=[...dashboard.modules.values()].filter(m=>m.file===file&&m.exports.has(mainKeys[0]));
  if(modules.length!==1)throw Error(`Ambiguous Dashboard locale ${file}`);
  const locale=path.basename(file).match(/^trans-([^.]+)\./)[1], local=JSON.parse(read(`locales/${locale}.json`));
  for(const k of mainKeys)expect(local[k]===dashboard.literal(modules[0].id,dashboard.exported(modules[0].id,k)),`Wrong ${locale}:${k}`);
  locales.push({locale,path:file,sha256:hash(dashboard.text(file))});
}
const report={schema_version:2,method:'Acorn mounted children and module-local exports; scoped lexical Rust contracts',
  toolbar_history:{source:dashboard.receipt(96776,toolbar),keys:historyKeys,
    local_fallback:'Internal page history first, then distinct retained shell entries; local unified-shell behavior.'},
  mounted:{header:receipt(header),center:receipt(center),right:receipt(right),battery:receipt(batteryNodes[0])},
  main_navigation:{keys:mainKeys,receipt:dashboard.receipt(35378,main[0]),locales},css,rules,
  local_consumers:consumers.map(([file,marker])=>({file,marker,sha256:hash(read(file))})),
  limitations:['Static contracts and cargo check do not prove rendered pixel equivalence.',
    'Header width uses measured text plus audited control dimensions, not live DOM child clientWidth.',
    'Earbud, hideBattValue, externalPowerConnected and extended header branches remain to be integrated.'],problems};
const target='docs/re/device-tabs-audit.json', rendered=JSON.stringify(report,null,2)+'\n';
if(process.argv.includes('--check'))expect(read(target)===rendered,'Stored header audit is stale');
else fs.writeFileSync(path.join(root,target),rendered);
if(problems.length){console.error(problems.join('\n'));process.exitCode=1;}
else console.log(`Mounted header: 3 groups, no extra arrows; ${mainKeys.length} Dashboard tabs / ${locales.length} locales; scoped contracts passed.`);
