// Reconstruct the mounted Profiles application, not shared URL helpers.
const fs = require('fs'), path = require('path');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const source = new Source('synapse/profiles');
const candidates = [];
walk(source.binding(43, 'Ba'), node => {
  if (node.type === 'ArrayExpression' && node.elements.length === 2
      && node.elements.every(e => e?.type === 'ObjectExpression'
        && e.properties.some(p => key(p.key) === 'component')
        && e.properties.some(p => key(p.key) === 'name'))) candidates.push(node);
});
if (candidates.length !== 1) throw Error('Ambiguous mounted navigation');
const navigation = candidates[0].elements.map(item => {
  const prop = name => item.properties.find(p => key(p.key) === name).value;
  return {id: source.literal(43, prop('id')), key: source.literal(43, prop('name')),
    component: source.snippet(43, prop('component')), receipt: source.receipt(43, item)};
});
if (navigation.map(n => n.key).join(',') !== 'GAMES_HEADER,DEVICE') throw Error('Navigation changed');
const keys = ['GAMES_HEADER', 'DEVICE', 'LINKED_GAMES', 'VIEWS', 'ORDER', 'ALL_GAMES',
  'REMOVED_GAMES', 'NAME_A_TO_Z', 'NAME_Z_TO_A', 'LAST_PLAYED', 'MOST_PLAYED',
  'CLICK_TO_ADD', 'GAME_PROGRAM', 'DRAG_AND_DROP_HERE', 'ADD_GAME_TITLE',
  'STILL_DONT_SEE_YOUR_GAME', 'BROWSE', 'SCAN_FOR_GAMES', 'SEARCH', 'REFRESH', 'ADD'];
const locales = [];
for (const file of source.files.filter(f => /\/trans-[^.]+\.[a-f0-9]+\.chunk\.js$/.test(f))) {
  source.parse(file);
  const modules = [...source.modules.values()].filter(m => m.file === file && m.exports.has('GAMES_HEADER'));
  if (modules.length !== 1) throw Error(`Ambiguous locale module ${file}`);
  const module = modules[0], locale = path.basename(file).match(/^trans-([^.]+)\./)[1];
  const values = Object.fromEntries(keys.map(k => [k, source.literal(module.id, source.exported(module.id, k))]));
  const local = JSON.parse(read(`locales/${locale}.json`));
  for (const k of keys) if (local[k] !== values[k]) throw Error(`Application locale mismatch ${locale}:${k}`);
  locales.push({locale, path: file, sha256: hash(source.text(file)), values});
}
const cssPaths = ['main.ee3cb5b6.css', '9449.b8e7f39a.chunk.css']
  .map(f => `${source.directory}/static/css/${f}`);
const css = [];
for (const file of cssPaths) {
  const text = read(file);
  for (const rule of parseCSS(text)) {
    if (/game-tile|profiles-link-games|listDevices_wrapper|device_to_linked_game|choose-a-mat|^\.main-nav|^\.nav-tabs|^\.razer-profiles|^\.content-wrapper|^\.backdrop|^\.plus-icon|^\.app_row|^\.popup-widget/.test(rule.selector)) {
      css.push({path: file, sha256: hash(text), ...rule});
    }
  }
}
const tile = css.find(rule => rule.selector === '.game-tile');
if (!tile?.declarations.includes('width:290px') || !tile.declarations.includes('height:220px')) throw Error('Game tile geometry changed');
const shell = read('crates/razer-shell/src/shell.rs'), local = read('crates/razer-app-pages/src/profiles_page.rs');
const problems = [];
for (const text of ['const ALL: [Self; 2]', 'Self::Games => "GAMES_HEADER"', 'Self::Devices => "DEVICE"',
  'view: ProfilesView::Games', 'history: vec![ProfilesView::Games]', '290.', '220.', '150.',
  'CLICK_TO_ADD', 'GAME_PROGRAM', 'DRAG_AND_DROP_HERE', 'REMOVED_GAMES']) {
  if (!local.includes(text)) problems.push(`Missing local contract: ${text}`);
}
for (const forbidden of ['fn route(', 'fn description(', 'GlobalShortcuts,', 'ChromaStudio,', 'nav_arrow_button', 'game_tile::add_new_tile']) {
  if (local.includes(forbidden)) problems.push(`Obsolete inferred page remains: ${forbidden}`);
}
if (!shell.includes('HistoryTarget::Profiles') || !shell.includes('page.step_history(forward, window, cx)')) {
  problems.push('Profiles history must belong to the existing shell toolbar');
}
const report = {
  schema_version: 2,
  method: 'Acorn module-local bindings and explicit webpack export getters; no reference code execution',
  window: {name: 'profiles', url: '/synapse/profiles/', open_param: 'policy=3,shouldFocus=1,tab_visible=1'},
  navigation, default_view: 'GAMES_HEADER',
  title: source.literal(4693, source.exported(4693, 'L$3')),
  components: [
    ...['Ba', 'oe', 'Ga', 'Kn', 'Zt', '$n'].map(name => ({name, ...source.receipt(43, source.binding(43, name))})),
    {name: 'filtering_toolbar', ...source.receipt(8844, source.binding(8844, 'm'))},
    {name: 'add_game_dialog', ...source.receipt(3137, source.binding(3137, 'f'))},
    {name: 'popup_widget', ...source.receipt(5529, source.binding(5529, 'r'))},
  ],
  locales, css,
  corrections: [
    'The five shared route regexes do not define mounted navigation: Ba mounts Games and Devices only.',
    'The initial view is nav[0], Games, unless session history selects another tab.',
    'Xt is the application toolbar, not a second pair of arrows in the page navigation.',
    'The main grid uses .game-tile (290x220, body 150); .linked-game-tile (240x190) belongs to a separate embedded surface.',
    'All ten application locales exist; short minified symbols resolve through module 4693 export getters.',
  ],
  limitations: ['Static contracts do not establish rendered pixel equivalence or hardware/service behavior.'],
  problems,
};
const target = 'docs/re/profiles-app-audit.json';
const rendered = JSON.stringify(report, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (read(target) !== rendered) problems.push('Stored Profiles audit is stale');
} else fs.writeFileSync(path.join(root, target), rendered);
if (problems.length) { console.error(problems.join('\n')); process.exitCode = 1; }
else console.log(`Profiles: ${navigation.length} mounted tabs, ${locales.length} scoped locales, ${css.length} CSS receipts; source/local contract verified.`);
