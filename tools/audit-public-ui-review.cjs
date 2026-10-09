// Maintained current-source review receipts. Parse vendor code as AST data only.
const fs = require('node:fs'), path = require('node:path'), acorn = require('acorn');
const {Source, walk, hash, key} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const compact = ({path, sha256, offset, end}) => ({path, sha256, offset, end});
const settings = new Source('synapse/settings');
const manifest = JSON.parse(read(`${settings.directory}/asset-manifest.json`));
settings.files = [...new Set(Object.values(manifest.files))]
  .filter(file => /^\/synapse\/settings\/static\/js\/[^/]+\.js$/.test(file))
  .map(file => `.ref/applications${file}`);
const settingsChunk = settings.files.find(file => /\/720\.[^/]+\.js$/.test(file));
if (!settingsChunk) throw Error('Current Settings 720 chunk is not manifest declared');
const settingsText = settings.text(settingsChunk), settingsBindings = {};
walk(acorn.parse(settingsText, {ecmaVersion:'latest'}), node => {
  if (node.type === 'VariableDeclarator' && ['Ks','ho','uo'].includes(node.id.name)) {
    if (settingsBindings[node.id.name]) throw Error('Ambiguous current Settings declaration');
    settingsBindings[node.id.name] = {path:settingsChunk, sha256:hash(settingsText),
      offset:node.init.start, end:node.init.end, source:settingsText.slice(node.init.start,node.init.end)};
  }
});
for (const name of ['Ks','ho','uo']) if (!settingsBindings[name]) throw Error(`Missing Settings ${name}`);
for (const text of ['getAppAutoStart(c)','getMinimizedOnStartUp(c)','active:r,disabled:!n'])
  if (!settingsBindings.Ks.source.includes(text)) throw Error(`Changed startup source: ${text}`);

const profiles = new Source('synapse/profiles');
const profileBindings = Object.fromEntries(['Ba','oe','Ga','$n','Ua'].map(name =>
  [name, profiles.receipt(43, profiles.binding(43,name))]));
for (const name of ['ia','xa']) profileBindings[name] = profiles.receipt(43,profiles.binding(43,name));
for (const name of ['AM','d9']) {
  const exported = profiles.exported(1867,name);
  if (exported.type !== 'Identifier') throw Error(`Changed Profiles naming export ${name}`);
  profileBindings[`1867/${name}`] = profiles.receipt(1867,profiles.binding(1867,exported.name));
}
for (const text of ['[P.sortType,P.filterType,c]','case Ta:','t.devices.find(e=>e.isVisible)',
  'activeProfileIndex:e','[null===A||void 0===A?void 0:A.profiles]'])
  if (!profileBindings.Ua.source.includes(text)) throw Error(`Changed linked-game source: ${text}`);
for (const text of ['type:"ON_ADD_PROFILE"','type:"ON_DUPLICATE_PROFILE"','type:"ON_DELETE_PROFILE"','100)'])
  if (!profileBindings.Ua.source.includes(text)) throw Error(`Changed profile collection source: ${text}`);
for (const text of ['confirmDel(!1)','confirmDel(!0)','window.addEventListener("click",this.mouseClick)'])
  if (!profileBindings.ia.source.includes(text)) throw Error(`Changed profile deletion source: ${text}`);

const dashboard = new Source('synapse/dashboard');
const dashboardRoot = dashboard.receipt(22534,dashboard.binding(22534,'Pi'));
const chroma = new Source('chroma-app/dashboard');
chroma.files = [...new Set(Object.values(JSON.parse(read(`${chroma.directory}/asset-manifest.json`)).files))]
  .filter(file => /^\/chroma-app\/dashboard\/static\/js\/[^/]+\.js$/.test(file))
  .map(file => `.ref/applications${file}`);
const chromaToolbar = Object.fromEntries(['Ks','Bs','Fs'].map(name =>
  [name,chroma.receipt(23322,chroma.binding(23322,name))]));
if (!chromaToolbar.Fs.source.includes('e={name:"settings-chroma",url:"/chroma-app/settings/"}'))
  throw Error('Chroma settings target changed');
const {source: studio} = require('./audit-chroma-studio.cjs');
const studioRoots = {ambient:1958,static:3181,spectrum:8552,breathing:2777,fire:5591,
  reactive:7518,ripple:5305,starlight:6548,wave:1591,wheel:8379,tidal:8154,audio:9120,'chroma-generate':2474};
const studioReceipts = Object.fromEntries(Object.entries(studioRoots).map(([name,module]) =>
  [name,{module,...compact(studio.receipt(module,studio.module(module).fn))}]));
const feedback = new Source('feedback');
const feedbackRoot = feedback.binding(4496,'ps');
const feedbackNames = ['render','getCategoryById','categoryOnClick','inputOnChange','emailOnBlur',
  'enableSubmit','renderEmail','renderForm','renderPrivacyEnquiry','renderSubmittingForm',
  'handleBeforeSubmit','isCheckLogNeeded','onSubmit'];
const feedbackMethods = {};
for (const method of feedbackRoot.body.body) {
  if (method.type === 'MethodDefinition' && feedbackNames.includes(key(method.key)))
    feedbackMethods[key(method.key)] = feedback.receipt(4496,method);
}
walk(feedbackRoot,node => {
  if (node.type === 'AssignmentExpression' && node.left.type === 'MemberExpression'
      && node.left.object.type === 'ThisExpression' && feedbackNames.includes(key(node.left.property)))
    feedbackMethods[key(node.left.property)] = feedback.receipt(4496,node.right);
});
for (const name of feedbackNames) if (!feedbackMethods[name]) throw Error(`Missing Feedback method ${name}`);
const catalog = JSON.parse(read('docs/re/application-catalog.json'));
if (catalog.applications.length !== 24) throw Error('Application inventory changed');
const endpoints = catalog.applications.map(app => {
  const directory = `.ref/applications${app.route}`;
  const files = ['index.html','manifest.json','asset-manifest.json'];
  const current = Object.fromEntries(files.map(name => {
    const file = directory + name;
    return [name,fs.existsSync(path.join(root,file)) ? {path:file,sha256:hash(read(file))} : null];
  }));
  return {route:app.route, recorded_html_status:app.endpoints['index.html']?.http_status,
    current_files:current, review_status:'Per-page status is documented in application-review-current.md; manifest presence is not UI completion.'};
});
const nativePaths = ['crates/razer-state/src/lib.rs','crates/razer-settings/src/settings_page.rs','crates/razer-settings/src/settings_window.rs',
  'crates/razer-settings/src/settings_systray_action.rs','crates/razer-app-pages/src/profiles_page.rs','crates/razer-app-pages/src/profiles_page/devices.rs',
  'crates/razer-pages/src/features/profile_collection.rs','crates/razer-pages/src/features/product_workspace.rs','crates/razer-pages/src/features/workspace.rs',
  'crates/razer-pages/src/features/source_workspace.rs','crates/razer-pages/src/features/control_pod_audio.rs',
  'crates/razer-app-pages/src/chroma_page.rs','crates/razer-pages/src/features/chroma_studio.rs','crates/razer-pages/src/features/chroma_studio_properties.rs',
  'crates/razer-app-pages/src/feedback_page.rs','crates/razer-shell/src/shell/chroma_window.rs'];
const report = {
  date:'2026-10-07',
  method:'Manifest-scoped Acorn parsing and exact current source hashes. Native render/lifecycle review is documented separately; source parsing alone is not UI completion.',
  settings:settingsBindings, profiles:profileBindings, dashboard:compact(dashboardRoot), studio:studioReceipts,
  chroma_toolbar:chromaToolbar,
  feedback:{module:4496,methods:feedbackMethods,categories:feedback.literal(4496,feedback.binding(4496,'ds'))},
  endpoints,
  native:Object.fromEntries(nativePaths.map(file=>[file,hash(read(file))])),
  reviewed_fixes:[
    'Settings startup controls keep a separate optional local draft; source default values and dependent disabled state are preserved without a host/OS query or write claim.',
    'Open DeviceGames follows all supplied workspaces, retains previously observed local games and incorporates new known associations.',
    'Linked Games filters by an actual association on any supplied device; deleted profile IDs are not valid assignment targets.',
    'Profile Add/Duplicate/Delete edit the local collection, preserving the independent assignment target and opaque copied settings; Delete requires delayed source-style confirmation.',
    'The live profile dropdown refreshes IDs and labels while retaining a valid assignment target; removed targets cancel pending rename/delete.',
    'Active-profile deletion refuses uncommitted mouse mapping or 1382 audio editor changes before a local restore can discard them.',
    'Chroma local preference save errors are displayed with retry while the current session edits remain available; writes are not claimed atomic.',
  ],
  pending:'See application-review-current.md. A validated source receipt, existing route or shared editor does not complete a page or device review.',
};
const out = path.join(root,'docs/re/public-ui-review-current-evidence.json');
const bytes = JSON.stringify(report,null,2)+'\n';
if (process.argv.includes('--check')) {
  if (fs.readFileSync(out,'utf8')!==bytes) throw Error('Public UI review receipt drifted');
} else fs.writeFileSync(out,bytes);
console.log('Public UI review: 3 Settings scopes, 9 Profiles scopes, Dashboard root, 13 Studio roots, 14 Feedback methods and 24 endpoint inventories; no vendor/runtime execution.');
