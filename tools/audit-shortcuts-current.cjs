// Current Global Shortcuts JSX/CSS and literal tables. No downloaded code runs.
const fs = require('fs'), path = require('path');
const {Source, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), source = new Source('synapse/dashboard');
const sourceIds = [94608, 21368, 46114, 62324, 60362, 79515, 40489, 74278, 82508, 34198, 48693, 27068, 97569];
const functions = source.literal(21368, source.binding(21368, 'E')).functionList;
const labels = Object.fromEntries(['qST', 'd8g', 'PKF', 'jrk', 'iiW', 'jxF', 'Oih', 'XD', 'RXF', 'IY1', 'f_P', 'lvj', 'mTg', 'OJ0', 'hIH']
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
  profile_actions: source.literal(67124, source.exported(67124, 'lg')),
  sensitivity_actions: source.literal(67124, source.exported(67124, 'aP')),
  excluded_keys: source.literal(94608, source.binding(94608, 'Ce')),
  labels,
};
const cssPath = '.ref/applications/synapse/dashboard/static/css/55.4e8559cb.chunk.css';
const css = fs.readFileSync(path.join(root, cssPath), 'utf8');
const rules = parseCSS(css);
const assets = [];
function asset(original, output, inline) {
  const sourcePath = `.ref/devices/182/static/media/${original}`;
  const bytes = inline ? Buffer.from(inline, 'base64') : fs.readFileSync(path.join(root, sourcePath));
  const destination = `assets/synapse/${output}`;
  if (!bytes.equals(fs.readFileSync(path.join(root, destination)))) throw Error(`Asset differs: ${destination}`);
  assets.push({source: inline ? cssPath : sourcePath, current_dashboard_css_reference: original,
    output: destination, sha256: hash(bytes), byte_equal: true,
    method: inline ? 'Decode current CSS data URI' : 'Current Dashboard CSS names this exact hashed shared resource; byte comparison uses current 182 cache'});
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
  assets,
  modules: sourceIds.map(id => ({id, ...source.receipt(id, source.module(id).fn)})),
  css: {path: cssPath, sha256: hash(css), rules: rules.filter(rule =>
    /shortcut|display_name|key-config|keymap-head|keymap-action|action-wrapper|\.actions(?:\b|\{)|launch-input|\.folder\b|\.maptext\b|\.titleRow/.test(rule.selector))},
};
for (const [file, value] of [
  ['src/features/shortcuts_current.json', data],
  ['docs/re/shortcuts-current-ui-evidence.json', evidence],
]) {
  const text = JSON.stringify(value, null, 2) + '\n';
  if (process.argv.includes('--check')) {
    if (fs.readFileSync(path.join(root, file), 'utf8') !== text) throw Error(`${file} is stale`);
  } else fs.writeFileSync(path.join(root, file), text);
}
console.log(`Current Global Shortcuts: ${functions.length} mapping branches, ${data.media.length} media and ${data.windows.length} Windows choices.`);
