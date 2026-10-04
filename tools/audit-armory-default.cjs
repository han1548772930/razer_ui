// Current Armory root, navigation reducer, feature fallback and introduction.
// Reference JavaScript is parsed by Acorn only; never evaluated.
const fs = require('fs'), path = require('path');
const {Source, hash} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const read = name => fs.readFileSync(path.join(root, name), 'utf8');
const source = new Source('synapse/armory');
const keys = ['SPOTLIGHT_HEADER', 'BROWSE_HEADER', 'MY_DOWNLOADS_HEADER', 'MY_UPLOADS_HEADER',
  'DASHBOARD_WORKSHOP', 'DASHBOARD_EXCHANGE', 'WORKSHOP_GET_STARTED', 'EXCHANGE_GET_STARTED',
  'AI_MACRO_DESC', 'MACRO_USAGE_DESC', 'ASSIGN_MACRO_DESC', 'CLOSE'];
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
  locales.push({locale, module: module.id, path: file, sha256: hash(source.text(file)), values});
}
const components = [29770, 20540, 77989, 60094, 95889, 3026, 86024].map(id => ({
  module: id, ...source.receipt(id, source.module(id).fn),
}));
const nav = source.binding(20540, 'c');
const navSource = source.snippet(20540, nav);
if (!navSource.includes('navs:[{id:i.ftd') || !navSource.includes('global.armory')) throw Error('Navigation changed');
const css = [];
for (const name of ['main.c0e644c4.css', '458.d86e844b.chunk.css']) {
  const file = `${source.directory}/static/css/${name}`, text = read(file);
  for (const match of text.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (/armory-introduction-banner|^\.nav-tabs|^div.nav-tabs|^\.main-nav|filteringBar/.test(match[1])) {
      css.push({path: file, sha256: hash(text), offset: match.index, selector: match[1], declarations: match[2]});
    }
  }
}
const local = read('src/shell/armory_page.rs'), shell = read('src/shell.rs');
const problems = [];
for (const text of ['phase1: false', 'guest: true', 'tab: ArmoryTab::Browse',
  'history: vec![ArmoryTab::Browse]', 'self.history.truncate', 'fn visible(',
  'self.phase1', 'self.guest', 'synapse/armory-introduction.png']) {
  if (!local.includes(text)) problems.push(`Missing local default/history contract: ${text}`);
}
if (!shell.includes('HistoryTarget::Armory')) problems.push('Armory history not connected to shell');
const report = {schema_version: 1, method: 'Acorn module scopes and export getters; static CSS declarations',
  components, locales, css,
  behavior: {settled_without_features: ['BROWSE_HEADER', 'MY_DOWNLOADS_HEADER'],
    guest: true, feature_flags: false, data_grid_when_empty: null,
    banner_default: true, banner_close_source: 'localStorage boolean via module 95889'},
  limitations: ['The source initially chooses Spotlight, then settles on Browse when feature loading ends with phase1 false.',
    'Local banner dismissal is retained for the lifetime of this page; source localStorage persistence remains to be connected.',
    'Search/filter controls, feature-enabled content, detail/share dialogs and services remain incomplete.',
    'No application execution or pixel validation.'], problems};
const target = 'docs/re/armory-default-source.json', rendered = JSON.stringify(report, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (read(target) !== rendered) problems.push('Stored Armory default audit is stale');
} else fs.writeFileSync(path.join(root, target), rendered);
if (problems.length) { console.error(problems.join('\n')); process.exitCode = 1; }
else console.log(`Armory default: ${components.length} scoped modules, ${locales.length} locales, ${css.length} CSS receipts.`);
