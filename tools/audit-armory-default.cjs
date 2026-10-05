// Current Armory root, navigation reducer, feature fallback and introduction.
// Reference JavaScript is parsed by Acorn only; never evaluated.
const fs = require('fs'), path = require('path');
const {Source, hash} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const read = name => fs.readFileSync(path.join(root, name), 'utf8');
const source = new Source('synapse/armory');
if (process.argv.includes('--check') && process.argv.includes('--write-locales')) {
  throw Error('--check is read-only and cannot be combined with --write-locales');
}
const keys = ['SPOTLIGHT_HEADER', 'BROWSE_HEADER', 'MY_DOWNLOADS_HEADER', 'MY_UPLOADS_HEADER',
  'DASHBOARD_WORKSHOP', 'DASHBOARD_EXCHANGE', 'WORKSHOP_GET_STARTED', 'EXCHANGE_GET_STARTED',
  'AI_MACRO_DESC', 'MACRO_USAGE_DESC', 'ASSIGN_MACRO_DESC', 'CLOSE'];
const controlKeys = ['ALL_CONNECTED_DEVICES_TEXT', 'LAPTOPS', 'MICE', 'KEYBOARDS', 'HEADSETS',
  'SPEAKERS', 'NEVER_DOWNLOADED', 'ALREADY_DOWNLOADED', 'VIEW_ALL_ITEMS', 'MOST_LIKES',
  'MOST_DOWNLOADS', 'LATEST', 'TITLE', 'CREATOR_NAME', 'TEXT_TRENDING', 'FILTER', 'SORT'];
const locales = [];
for (const file of source.files.filter(f => /\/trans-[^.]+\.[a-f0-9]+\.chunk\.js$/.test(f))) {
  source.parse(file);
  const modules = [...source.modules.values()].filter(m => m.file === file && m.exports.has(keys[0]));
  if (modules.length !== 1) throw Error(`Ambiguous Armory locale ${file}`);
  const locale = path.basename(file).match(/^trans-([^.]+)\./)[1], module = modules[0];
  const values = Object.fromEntries(keys.map(k => [k, source.literal(module.id, source.exported(module.id, k))]));
  const filename = `locales/${locale}.json`, local = JSON.parse(read(filename));
  if (process.argv.includes('--write-locales')) {
    local.ARMORY_SOURCE = {...local.ARMORY_SOURCE, ...values};
    fs.writeFileSync(path.join(root, filename), JSON.stringify(local, null, 2) + '\n');
  }
  for (const key of keys) if (local.ARMORY_SOURCE?.[key] !== values[key]) throw Error(`Locale mismatch ${locale}:${key}`);
  for (const key of controlKeys) {
    const value = source.literal(module.id, source.exported(module.id, key));
    if (local[key] !== value) throw Error(`Control locale mismatch ${locale}:${key}`);
  }
  locales.push({locale, module: module.id, path: file, sha256: hash(source.text(file)), values});
}
// Current lazy card/detail/share modules remain in the root receipt. Local
// sharing now has a separate audit; it does not fabricate service contributions.
const components = [29770, 20540, 77989, 60094, 95889, 3026, 86024, 68142,
  27588, 13476, 66517, 54408, 49496, 38198, 52259, 70017, 8679].map(id => ({
  module: id, ...source.receipt(id, source.module(id).fn),
}));
const nav = source.binding(20540, 'c');
const navSource = source.snippet(20540, nav);
if (!navSource.includes('navs:[{id:i.ftd') || !navSource.includes('global.armory')) throw Error('Navigation changed');
const css = [];
for (const name of ['main.c0e644c4.css', '458.d86e844b.chunk.css']) {
  const file = `${source.directory}/static/css/${name}`, text = read(file);
  for (const match of text.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (/armory-introduction-banner|^\.nav-tabs|^div.nav-tabs|^\.main-nav|filteringBar|profile-card|detail-footer|share-new-profile-modal|share-new-profile-form|profile-act|\.reshare/.test(match[1])) {
      css.push({path: file, sha256: hash(text), offset: match.index, selector: match[1], declarations: match[2]});
    }
  }
}
const local = read('src/shell/armory_page.rs'), shell = read('src/shell.rs');
const problems = [];
const banner = source.snippet(3026, source.binding(3026, 'a'));
const bannerKeys = Object.fromEntries(['$JL', 'HBy'].map(name => [name,
  source.literal(54693, source.exported(54693, name))]));
if (!banner.includes('isExchangeEnabled') || !banner.includes('text:a?s.$JL:s.HBy')
  || bannerKeys.$JL !== 'EXCHANGE_GET_STARTED' || bannerKeys.HBy !== 'WORKSHOP_GET_STARTED') {
  problems.push('Armory mounted banner feature/locale branch changed');
}
if (!local.includes('tr("WORKSHOP_GET_STARTED").to_uppercase()')
  || local.includes('tr("EXCHANGE_GET_STARTED").to_uppercase()')) {
  problems.push('Native no-response banner must use the initial Workshop branch');
}
const inlineIcons = [
  {module: 70017, output: 'assets/synapse/armory-filter.svg'},
  {module: 8679, output: 'assets/synapse/armory-sort.svg'},
].map(item => {
  const literal = read(item.output), currentModule = source.snippet(item.module, source.module(item.module).fn);
  for (const match of literal.matchAll(/\b(?:d|viewBox|transform|width|height)="([^"]+)"/g)) {
    if (!currentModule.includes(JSON.stringify(match[1]))) problems.push(`Inline SVG differs from module ${item.module}: ${match[0]}`);
  }
  return {...item, sha256: hash(literal)};
});
for (const text of ['phase1: false', 'guest: true', 'tab: ArmoryTab::Browse',
  'history: vec![ArmoryTab::Browse]', 'self.history.truncate', 'fn visible(',
  'self.phase1', 'self.guest', 'synapse/armory-introduction.png',
  'filter_open: false', 'sort_open: false', 'filter_selection:', 'fn filtering_bar(',
  'id("armory-filtering-bar")', 'armory-filter', 'armory-sort',
  'armory-filter-options', 'armory-sort-options', 'armory-filter-laptop',
  'armory-filter-never-downloaded', 'armory-sort-trending',
  'synapse/armory-filter.svg', 'synapse/armory-sort.svg', 'fn armory_option(',
  'fn search_control(', 'armory-search-wrapper', 'armory-search-clear',
  'Duration::from_millis(300)']) {
  if (!local.includes(text)) problems.push(`Missing local default/history contract: ${text}`);
}
if (!shell.includes('HistoryTarget::Armory')) problems.push('Armory history not connected to shell');
const report = {schema_version: 1, method: 'Acorn module scopes and export getters; static CSS declarations',
  components, locales, css, inline_icons: inlineIcons, verified_control_locale_keys: controlKeys,
  behavior: {settled_without_features: ['BROWSE_HEADER', 'MY_DOWNLOADS_HEADER'],
    guest: true, feature_flags: false, data_grid_when_empty: null,
    banner_default: true, banner_close_source: 'localStorage boolean via module 95889',
    banner_title: {source_module: 3026, locale_module: 54693, source_keys: bannerKeys,
      native_no_response: 'WORKSHOP_GET_STARTED', initial_is_exchange_enabled: false,
      resolved_all_false_features_is_exchange_enabled: true,
      explanation: 'No service response preserves the hook initial value; a successful all-false feature response would select Exchange.'},
    filtering_bar: {mounted_for: ['BROWSE_HEADER', 'MY_DOWNLOADS_HEADER'],
      source_modules: [54408, 49496, 38198, 52259, 70017, 8679],
      local_state: ['filter_open', 'sort_open', 'filter_selection', 'sort_choice'],
      filter_options: ['ALL_CONNECTED_DEVICES_TEXT', 'LAPTOPS', 'MICE', 'KEYBOARDS', 'HEADSETS', 'SPEAKERS', 'NEVER_DOWNLOADED', 'ALREADY_DOWNLOADED', 'VIEW_ALL_ITEMS'],
      sort_options: ['MOST_LIKES', 'MOST_DOWNLOADS', 'LATEST', 'TITLE', 'CREATOR_NAME', 'TEXT_TRENDING'],
      option_panels: ['armory-filter-options', 'armory-sort-options'], service_actions: 'deferred'},
    search: {source_module: 68142, debounce_ms: 300, service_actions: 'deferred'},
    content: {card_module: 27588, lazy_list_module: 86024, detail_module: 13476,
      share_module: 66517, empty_dataset_render: 'null',
      local_share: {entry: 'ArmoryPage::open_share_profile', evidence: 'docs/re/armory-share-current-evidence.json',
        profile_snapshot: true, form: true, preview: 'local profile summary and available mouse DPI', service_submission: false}},
  },
  limitations: ['The source initially chooses Spotlight, then settles on Browse when feature loading ends with phase1 false.',
    'The source localStorage key is mirrored under APPDATA/razer_ui; service-backed persistence remains local to this shell.',
    'Filter controls mirror the source device/download groups and six sort entries with local state; search/debounce remain local. An explicit existing-profile Share command opens the current form and local summary/DPI preview. Remote filtering/sorting, contribution cards, full remote detail, account/moderation and submission remain incomplete; see the separate share audit.',
    'No application execution or pixel validation.'], problems};
const target = 'docs/re/armory-default-source.json', rendered = JSON.stringify(report, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (read(target) !== rendered) problems.push('Stored Armory default audit is stale');
} else fs.writeFileSync(path.join(root, target), rendered);
if (problems.length) { console.error(problems.join('\n')); process.exitCode = 1; }
else console.log(`Armory default: ${components.length} scoped modules, ${locales.length} locales, ${css.length} CSS receipts.`);
