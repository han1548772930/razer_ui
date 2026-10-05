// Parse current vendor sources as data only. Never execute reference code.
const fs = require('node:fs');
const path = require('node:path');
const acorn = require('acorn');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const dashboard = new Source('synapse/dashboard');
const macro = new Source('synapse/macro');
const main = macro.files.find(file => /\/main\.[a-f0-9]+\.js$/.test(file));
const text = macro.text(main);
let business;
walk(acorn.parse(text, {ecmaVersion: 'latest'}), node => {
  if (!/Function/.test(node.type) || node.body?.type !== 'BlockStatement') return;
  const definitions = new Map();
  for (const statement of node.body.body) {
    if (statement.type === 'VariableDeclaration') {
      for (const d of statement.declarations)
        if (d.id.type === 'Identifier') definitions.set(d.id.name, d.init);
    } else if (statement.id?.name) definitions.set(statement.id.name, statement);
  }
  if (['an', 'un', 'dn', 'Ct', 'Ys', 'Zs'].every(name => definitions.has(name))) {
    if (business) throw Error('Ambiguous Macro business scope');
    business = {node, definitions};
  }
});
if (!business) throw Error('Missing current Macro business scope');
function receipt(node) {
  if (!node) throw Error('Missing Macro declaration');
  return {path: main, sha256: hash(text), offset: node.start, end: node.end,
    source: text.slice(node.start, node.end)};
}
const scope = Object.fromEntries(['Mn', 'Bt', 'an', 'dn', 'zn', 'Gr', 'Co', 'Js', 'qs', 'gr', 'K', 'Zs', 'Qs']
  .map(name => [name, receipt(business.definitions.get(name))]));
const idb = [];
walk(business.node, node => {
  if (node.type === 'AssignmentExpression' && node.left.name === 'A'
      && node.right.type === 'NewExpression' && node.right.callee.name === 'Promise') idb.push(receipt(node));
});
if (idb.length !== 1 || !idb[0].source.includes('keyPath:"guid"')) throw Error('Changed IDB contract');
function moduleReceipts(source, ids) {
  return Object.fromEntries(ids.map(id => [id, source.receipt(id, source.module(id).fn)]));
}
function literals(source, id, names) {
  return Object.fromEntries(names.map(name => [name, source.literal(id, source.exported(id, name))]));
}
function css(route, pattern) {
  const directory = `.ref/applications/synapse/${route}`;
  const manifest = JSON.parse(fs.readFileSync(path.join(root, directory, 'asset-manifest.json'), 'utf8'));
  return [...new Set(Object.values(manifest.files))].filter(file => file.endsWith('.css')).flatMap(file => {
    const sourcePath = `${directory}/${file.slice(2)}`;
    const content = fs.readFileSync(path.join(root, sourcePath), 'utf8');
    const rules = parseCSS(content).filter(rule => pattern.test(rule.selector));
    return rules.length ? [{path: sourcePath, sha256: hash(content), rules}] : [];
  });
}
const data = {
  method: 'Manifest-scoped Acorn AST and CSS parsing; no vendor execution or obsolete sources',
  native: {
    verification: 'File fingerprints of reviewed integration, not semantic tests or rendered validation',
    files: Object.fromEntries([
      'src/features/macro_library.rs', 'src/store.rs', 'src/shell.rs',
      'src/features/shortcuts.rs', 'src/features/shortcuts_macro.rs',
      'src/features/shortcuts_mapping.rs', 'src/features/shortcut_engine.rs',
      'src/shell/macro_page.rs', 'src/shell/macro_page/state.rs',
      'src/shell/macro_page/unsaved.rs', 'src/shell/macro_page/body.rs',
      'src/shell/macro_page/chrome.rs', 'src/shell/macro_page/tree.rs',
      'src/shell/macro_page/nested.rs',
    ].map(file => [file, hash(fs.readFileSync(path.join(root, file)))])),
  },
  dashboard: moduleReceipts(dashboard, [82508, 3844, 97278, 3466, 44230, 59007]),
  playback: literals(dashboard, 67124, ['gJ', 'cv', 'kJ']),
  macroTypes: literals(dashboard, 69937, ['CKm', 'AX$']),
  macro: {scope, idb, documentHelpers: moduleReceipts(macro, [25572]),
    nested: {
      components: Object.fromEntries(['En', 'je', 'Ve', 'xe', 'Be'].map(name =>
        [name, macro.receipt(58190, macro.binding(58190, name))])),
      dropdown: moduleReceipts(macro, [38162, 97278]),
      mutation: macro.receipt(25572, macro.binding(25572, 'C')),
      placeholder: literals(macro, 37927, ['Adr']),
      action: literals(macro, 4173, ['Lt']),
      contract: 'Exclude current document; disable candidates with a live path to current; assign guid from original macroList index; missing target renders placeholder; do not retarget copied nested references',
      localAdaptation: 'Persistent local macro_id; recompute paths after edits with a visited set instead of source useMemo keyed only to current identity and list length; never infer identity from an old name-only row',
    },
    actions: literals(macro, 4173, ['XE', 'Mh', 'hc', 'hJ', 'KK', 'K3', 'O$', 'bB']),
    dialog: literals(macro, 54693, ['hJ_', 'Lvx', 'ENf', 'set', 'W0P']),
    storage: {...literals(macro, 69937, ['w9p', 'Y9B']), ...literals(macro, 68511, ['G4'])}},
  css: [...css('dashboard', /dropdown-area|s3-dropdown|s3-options|stepper|map-macro__playback/),
    ...css('macro', /save-alert|save-close|^\.backdrop|^\.thx-btn|keymap-action|CustomDropdown_|CustomInput_custom_input|^\.s3-dropdown|^\.s3-options/)],
  contract: {
    emptyMacrosSelectable: true,
    newSwitchRefresh: 'Suspend action; Save or discard current document, then run it; close cancels',
    duplicate: 'No dirty guard; clone live document into saved copy; preserve original draft',
    identity: 'Source uuidv4 client identity; local implementation uses workspace high-water IDs, not native GUIDs',
    persistence: 'Only savedMacros persisted; live macroList contains independent drafts',
    globalStandard: 'Once, NTimes, ContinuousToggle, Queue; no ContinuousHeld',
    sourceQuirks: ['render sorts array in place and selection remains numeric',
      'missing GUID restores index -1; generic dropdown returns null',
      'type-change branch reads AX$.SYNAPSE.SEQUENCE, but AX$ contains delay choices'],
    localAdaptation: 'Keep identity across reorder; retain missing nonzero reference without silently retargeting; allow one-choice repair; zero placeholder adopts first available document; validate playback against document type and repeat drafts before local persistence'
  }
};
const target = path.join(root, 'docs/re/shortcuts-macro-current-contract.json');
const output = JSON.stringify(data, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (fs.readFileSync(target, 'utf8') !== output) throw Error('Stale Macro/Shortcuts contract');
} else fs.writeFileSync(target, output);
console.log('Current Macro/Shortcuts contract: document persistence, unsaved guards, playback, dropdown and CSS');
