// Current Global Shortcuts JSX/CSS and literal tables. No downloaded code runs.
const fs = require('fs'), path = require('path');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), source = new Source('synapse/dashboard');
const sourceIds = [94608, 21368, 46114, 62324, 60362, 79515, 40489, 74278, 82508, 34198, 81787, 48693, 27068, 97569];
const functions = source.literal(21368, source.binding(21368, 'E')).functionList;
const labels = Object.fromEntries(['qST', 'd8g', 'PKF', 'jrk', 'iiW', 'jxF', 'Oih', 'XD', 'RXF', 'IY1', 'f_P', 'lvj', 'mTg', 'OJ0', 'hIH', 'pjv', 'rYZ', 'Y6o']
  .map(name => [name, source.literal(54693, source.exported(54693, name))]));
const data = {
  functions,
  // Shifted symbol descriptors have no inputID and do not participate in the
  // source's inputID lookup; retain the actual keyboard-name rows only.
  key_names: Object.fromEntries(source.binding(46114, 'i').elements.filter(item => item.properties.some(p => key(p.key) === 'inputID')).map(item => {
    const property = name => item.properties.find(p => key(p.key) === name)?.value;
    return [source.literal(46114, property('inputID')), source.literal(46114, property('name'))];
  })),
  media: source.literal(67124, source.exported(67124, 'hx')),
  windows: source.literal(67124, source.exported(67124, 'Zi')),
  macro_playback: source.literal(67124, source.exported(67124, 'gJ')).filter(item => item.id !== 'ContinuousHeld'),
  macro_sequence_playback: source.literal(67124, source.exported(67124, 'cv')),
  macro_phased_playback: source.literal(67124, source.exported(67124, 'kJ')),
  profile_actions: source.literal(67124, source.exported(67124, 'lg')),
  sensitivity_actions: source.literal(67124, source.exported(67124, 'aP')),
  excluded_keys: source.literal(94608, source.binding(94608, 'Ce')),
  labels,
};
// Interpret only the exact literal split/map shape; do not evaluate vendor JS.
const emojiGroups = source.binding(34198, 'c').elements.map(group => {
  const property = name => group.properties.find(p => key(p.key) === name)?.value;
  const map = property('values'), split = map?.callee?.object;
  if (map?.type !== 'CallExpression' || key(map.callee.property) !== 'map'
      || split?.type !== 'CallExpression' || key(split.callee.property) !== 'split'
      || split.callee.object.type !== 'Literal' || split.arguments[0]?.value !== ' '
      || source.snippet(34198, map.arguments[0]) !== 'e=>({emo:e})') throw Error('Emoji literal shape changed');
  return {label: source.literal(34198, property('label')), values: split.callee.object.value.split(' ')};
});
let emojiTabs;
walk(source.binding(34198, 'b'), node => {
  if (node.type === 'AssignmentExpression' && node.left.type === 'MemberExpression'
      && node.left.object.type === 'ThisExpression' && key(node.left.property) === 'emojiList') {
    emojiTabs = source.literal(34198, node.right);
  }
});
if (emojiTabs?.length !== emojiGroups.length) throw Error('Emoji category/tab mismatch');
data.emoji = {
  groups: emojiGroups,
  tabs: emojiTabs,
  search: source.literal(34198, source.binding(34198, 'g')).split(',').map(value => {
    const words = value.split(' '); return {emo: words.shift(), name: words.join(' ')};
  }),
  variants: source.literal(34198, source.binding(34198, 'd')),
  excluded_without_windows_10: source.literal(34198, source.binding(34198, 'm')),
};
const cssPath = '.ref/applications/synapse/dashboard/static/css/55.4e8559cb.chunk.css';
const css = fs.readFileSync(path.join(root, cssPath), 'utf8');
const rules = parseCSS(css);
const textRules = Object.fromEntries([
  '.maptext .tip', '.maptext .emoji_popup_wrapper',
  '.maptext .emoji_popup_wrapper .emoji_popup_main_item .showtip',
  '.maptext .emoji_popup_wrapper .emoji_popup_header_container .tip',
  '.maptext .action_bar_wrapper .tip',
].map(selector => {
  const rule = rules.find(rule => rule.selector === selector);
  if (!rule) throw Error(`Missing current text presentation rule ${selector}`);
  return [selector, Object.fromEntries(rule.properties.map(p => [p.property, p.value]))];
}));
if (textRules['.maptext .tip'].transition !== 'visibility 0s,opacity .3s linear'
    || textRules['.maptext .tip']['transition-delay'] !== '1s'
    || textRules['.maptext .emoji_popup_wrapper'].transition !== 'visibility 0s,opacity .5s linear'
    || textRules['.maptext .action_bar_wrapper .tip'].bottom !== '-38px'
    || textRules['.maptext .emoji_popup_wrapper .emoji_popup_header_container .tip'].top !== '100%') {
  throw Error('Current text presentation timing/placement changed');
}
const popupComponent = source.snippet(34198, source.binding(34198, 'v'));
const outerWrapper = source.snippet(81787, source.binding(81787, 'l'));
if (!popupComponent.includes('extraClasses:e.classes') || !popupComponent.includes('className:e.classes')
    || !outerWrapper.includes('className:e.extraClasses')) throw Error('Double popup wrapper changed');
const textNativePath = 'crates/razer-pages/src/features/shortcuts_text.rs';
const assets = [];
function asset(original, output, inline, directory = '.ref/devices/182/static/media') {
  const sourcePath = `${directory}/${original}`;
  const bytes = inline ? Buffer.from(inline, 'base64') : fs.readFileSync(path.join(root, sourcePath));
  const destination = `assets/synapse/${output}`;
  if (!bytes.equals(fs.readFileSync(path.join(root, destination)))) throw Error(`Asset differs: ${destination}`);
  assets.push({source: inline ? cssPath : sourcePath, current_dashboard_css_reference: original,
    output: destination, sha256: hash(bytes), byte_equal: true,
    method: inline ? 'Decode current CSS data URI' : 'Current Dashboard CSS names this exact hashed shared resource; byte comparison uses current 182 cache'});
}
for (const [original, output] of [
  ['icon_emoji.07a71bf8.svg', 'shortcuts-emoji.svg'],
  ['icon_emoji_active.b1a4efad.svg', 'shortcuts-emoji-active.svg'],
  ['icon_charactermap.025aae0b.svg', 'shortcuts-character.svg'],
  ['icon_charactermap_active.3c3d7fb3.svg', 'shortcuts-character-active.svg'],
  ['icon_search.b84dee08.svg', 'shortcuts-emoji-search.svg'],
  ['icon_close_enclosed.6056b667.svg', 'shortcuts-search-clear.svg'],
  ['icon_close_enclosed_a.83aff4bb.svg', 'shortcuts-search-clear-active.svg'],
]) {
  if (!css.includes(original) && !source.snippet(34198, source.binding(34198, 't')).includes(original)) throw Error('Missing text resource');
  asset(original, output);
}
for (const [original, output] of [
  ['icon_more_g.32e1e984.svg', 'macro/binding-more.svg'], ['icon_more.fb688d78.svg', 'nav-more-hover.svg'],
  ['icon_warning.6c0cd78b.svg', 'automation-icon_warning.svg'], ['icon_folder.e5ceb5b6.svg', 'automation-icon_folder.svg'],
  ['icon_add.c95a8d74.svg', 'dashboard-add.svg'], ['icon_add_w.0fc3f789.svg', 'dashboard-add-hover.svg'],
]) {
  if (!css.includes(`../../static/media/${original}`)) throw Error('Missing current CSS resource');
  asset(original, output);
}
const stems = ['profile', 'sensitivity', 'lighting', 'macro', 'text', 'launch', 'multimedia', 'windows'];
for (const [index, category] of functions.entries()) for (const selected of [false, true]) {
  const selector = `.action-wrapper.${category}${selected ? '.open' : ''} .head:before`;
  const rule = rules.find(rule => rule.selector.split(',').includes(selector));
  if (!rule) throw Error('Missing rail rule ' + selector);
  const url = rule.declarations.match(/background-image:url\(([^)]+)\)/)?.[1];
  if (!url) throw Error('Missing rail asset ' + selector);
  const stem = stems[index], ext = stem === 'lighting' ? 'png' : 'svg';
  asset(url.startsWith('data:') ? selector : path.posix.basename(url),
    `mapping-${stem}${selected ? '-active' : ''}.${ext}`, url.startsWith('data:') ? url.split(',')[1] : undefined);
}
const evidence = {
  source_date: '2026-10-05', generator_sha256: hash(fs.readFileSync(__filename)),
  method: 'Acorn module scopes and CSS rule parsing; current manifest only. No application execution.',
  data,
  text_presentation: {
    rules: textRules,
    popup_wrapper_count: 2,
    popup_component: source.receipt(34198, source.binding(34198, 'v')),
    outside_click_wrapper: source.receipt(81787, source.binding(81787, 'l')),
    native: {path: textNativePath, sha256: hash(fs.readFileSync(path.join(root, textNativePath)))},
    verification: 'Static timing/placement/source receipts and native hash only; no window or visual execution.',
  },
  assets,
  missing_assets: ['icon_close.130e45fb.svg'],
  modules: sourceIds.map(id => ({id, ...source.receipt(id, source.module(id).fn)})),
  css: {path: cssPath, sha256: hash(css), rules: rules.filter(rule =>
    /shortcut|display_name|key-config|keymap-head|keymap-action|action-wrapper|\.actions(?:\b|\{)|launch-input|\.folder\b|\.maptext\b|\.titleRow/.test(rule.selector))},
};
// Text controls are actual consumers: verify registration as well as byte
// equality, so a correct loose SVG cannot silently become a missing UI asset.
const manifestPath = path.join(root, 'assets/synapse/manifest.json');
const resourceManifest = JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
const embeddedPath = path.join(root, 'assets/synapse/embedded.rs');
let embedded = fs.readFileSync(embeddedPath, 'utf8');
for (const asset of assets.filter(asset => path.basename(asset.output).startsWith('shortcuts-'))) {
  const registration = {source:asset.source, source_sha256:hash(fs.readFileSync(path.join(root, asset.source))), output:asset.output, sha256:asset.sha256,
    purpose:'Current Dashboard MapText toolbar/search; exact current shared source bytes', current_dashboard_css_reference:asset.current_dashboard_css_reference};
  const existing = resourceManifest.entries.find(entry => entry.output === asset.output);
  const key = asset.output.slice('assets/'.length);
  const filename = path.basename(asset.output);
  const line = `    ("${key}", include_bytes!("${filename}") as &[u8]),`;
  if (process.argv.includes('--check')) {
    if (!existing || Object.entries(registration).some(([key,value]) => existing[key] !== value) || !embedded.includes(line)) throw Error(`Unregistered text asset ${asset.output}`);
  } else {
    if (existing) Object.assign(existing, registration); else resourceManifest.entries.push(registration);
    if (!embedded.includes(`"${key}"`)) embedded = embedded.trimEnd().replace(/\]$/, `${line}\n]`) + '\n';
  }
}
if (!process.argv.includes('--check')) {
  fs.writeFileSync(manifestPath,JSON.stringify(resourceManifest,null,2)+'\n');
  fs.writeFileSync(embeddedPath,embedded);
}
for (const [file, value] of [
  ['crates/razer-pages/src/features/shortcuts_current.json', data],
  ['docs/re/shortcuts-current-ui-evidence.json', evidence],
]) {
  const text = JSON.stringify(value, null, 2) + '\n';
  if (process.argv.includes('--check')) {
    if (fs.readFileSync(path.join(root, file), 'utf8') !== text) throw Error(`${file} is stale`);
  } else fs.writeFileSync(path.join(root, file), text);
}
console.log(`Current Global Shortcuts: ${functions.length} mapping branches, ${data.media.length} media and ${data.windows.length} Windows choices.`);
