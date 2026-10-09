// Current 3946 manifest -> mounted aH -> keyboard listeners, artwork and CSS.
// Reference JavaScript is parsed only. --check does not write any files.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const fact = (ok, message) => { if (!ok) throw Error(message); };
function one(ast, predicate, label) {
  const found = []; walk(ast, node => { if (predicate(node)) found.push(node); });
  fact(found.length === 1, `${label}: expected one node, found ${found.length}`);
  return found[0];
}
const manifestPath = '.ref/devices/3946/asset-manifest.json';
const manifest = JSON.parse(read(manifestPath));
const main = Object.values(manifest.files).find(file => /main\.[a-f0-9]+\.js$/.test(file));
fact(main, 'Current main bundle is absent from manifest');
const file = '.ref/devices/3946/' + main.replace(/^\.\//, '');
const source = read(file), ast = acorn.parse(source, {ecmaVersion: 'latest'});
const receipt = node => ({path:file, sha256:hash(source), offset:node.start, end:node.end, source:source.slice(node.start,node.end)});
const component = one(ast, node => node.type === 'VariableDeclarator' && node.id.name === 'aH' && node.init?.start === 6920385, 'aH').init;
const componentText = receipt(component).source;
fact(component.end === 6931761, 'aH boundary changed');
const prior = JSON.parse(read('docs/re/automation-current-evidence.json')).components.find(node => node.offset === component.start);
fact(prior?.source === componentText, 'Main automation evidence does not match current aH');
const capture = one(component, node => node.type === 'ArrowFunctionExpression' && source.slice(node.start,node.end).startsWith('()=>{if(!X)return;'), 'capture effect');
const start = one(component, node => node.type === 'VariableDeclarator' && node.id.name === 'Te', 'capture start');
const keyboard = one(component, node => node.type === 'ObjectExpression' && node.properties.some(p => key(p.key) === 'className' && source.slice(p.value.start,p.value.end).startsWith('"macro-keyboard-capture ".concat')), 'keyboard JSX');
for (const fragment of ['document.addEventListener("keydown",e)', 'document.addEventListener("keyup",t)', 't=()=>{Z(!1),$(!1)}', 'e.preventDefault(),e.stopPropagation()', 'e.some(e=>e.key===t)||e.length>=10']) {
  fact(receipt(capture).source.includes(fragment), `Changed capture contract: ${fragment}`);
}
for (const fragment of ['0===O.length&&!X', 'X&&!J', 'placeholder:"Start typing"', 'className:"macro-pill__delete"', 'a<O.length-1']) {
  fact(receipt(keyboard).source.includes(fragment), `Changed keyboard JSX: ${fragment}`);
}
fact(componentText.includes('"Escape"!==e.key||X||'), 'Escape no longer depends on capture state');
const icon = one(ast, node => node.type === 'VariableDeclarator' && node.id.name === 'eH' && node.init?.start === 6915402, 'delete icon').init;
const svg = one(icon, node => node.type === 'CallExpression' && node.arguments[0]?.value === 'svg', 'delete SVG');
const pathNode = one(icon, node => node.type === 'CallExpression' && node.arguments[0]?.value === 'path', 'delete path');
const pathData = pathNode.arguments[1].properties.find(p => key(p.key) === 'd').value.value;
const resources = JSON.parse(read('assets/synapse/automation-manifest.json')).filter(asset => asset.output.includes('automation-quick-macro-'));
fact(resources.length === 5, 'Expected four type icons and delete SVG');
for (const asset of resources) {
  fact(hash(read(asset.source)) === asset.source_sha256 && hash(read(asset.output)) === asset.output_sha256, `Changed ${asset.output}`);
}
const deletion = resources.find(asset => asset.output.endsWith('-delete.svg'));
fact(deletion.source_offset === svg.start && deletion.source_end === svg.end, 'Wrong delete SVG receipt');
fact(read(deletion.output).includes(`d="${pathData}"`) && read(deletion.output).includes('width="24"'), 'Delete shape or mounted size changed');
const css = [];
for (const value of Object.values(manifest.files).filter(file => /\.css$/.test(file))) {
  const cssPath = '.ref/devices/3946/' + value.replace(/^\.\//, '');
  const raw = read(cssPath);
  css.push(...parseCSS(raw).filter(rule => /macro-keyboard|macro-pill|macro-type-select|quick-macro-section__icon/.test(rule.selector)).map(rule => ({path:cssPath, sha256:hash(raw), ...rule})));
}
fact(css.some(rule => rule.selector === '.macro-pill>span,.macro-pill__delete'), 'Missing opacity transition CSS');
const nativePath = 'crates/razer-pages/src/features/automation/quick_macro.rs', native = read(nativePath), compact = native.replace(/\s+/g,'');
for (const fragment of ['capturing: bool', 'capture_modifiers: Modifiers', 'capture_key_down', '.on_key_up(', '.on_modifiers_changed(', 'self.keys.is_empty() && !self.capturing', 'self.keys.len() < 10', 'window.prevent_default()', 'cx.stop_propagation()', 'self.capture.focus(window, cx)', 'fn type_selector', 'item.content(self.kind == item.value)', 'Duration::from_millis(200)', 'motion::Easing::Ease', 'automation-quick-macro-delete.svg', 'this.keys.retain(|key| key != &remove_key)', '"keyboard" | "" => self.keyboard_field(window, cx)']) {
  fact(compact.includes(fragment.replace(/\s+/g,'')), `Missing native contract: ${fragment}`);
}
fact(read('crates/razer-pages/src/features/automation/editor.rs').includes('.keyboard(false)'), 'Outer dialog must delegate Escape to quick editor');
fact(!/std::process|Command::new|ShellExecute|LoadLibrary/.test(native), 'Quick macro unexpectedly executes a target');
const report = {
  method: 'Current manifest and Acorn AST; literal SVG and ordered CSS receipts; reference code is never evaluated.',
  manifest: {path:manifestPath, sha256:hash(read(manifestPath))}, component:receipt(component), capture:receipt(capture), start:receipt(start), keyboard:receipt(keyboard), icon:receipt(icon), css, resources,
  native: {path:nativePath,
    capture:'Only empty + starts a session. Any normal keyup or modifier release stops it. Duplicates are ignored and the limit is ten. Escape records during capture, closes otherwise. No extra + appears beside existing keys.',
    input:'The source 28px temporary Start typing input owns initial focus. After the first key it unmounts and the enclosing capture listener retains focus.',
    modifiers:'Windows GPUI emits Ctrl/Alt/Shift/Windows as ModifiersChanged, so rising edges append source key labels and falling edges stop capture.',
    presentation:'Four exact tH images appear in both select trigger and rows; empty Action uses its disabled keyboard image. Pills use source padding/size, + separators, 24px eH delete overlay, hover opacity and 200ms ease transitions.',
    persistence:'Only local QuickMacroSaved payload changes; no execution, vendor service response or device observation is synthesized.',
    remaining:['Caps Lock physical press/release is unavailable through GPUI Windows ModifiersChanged, which exposes only the toggled state. It is not synthesized.', 'The type selector now follows the current aH menu; exact conditional mounting, cascade and persistent-trigger transitions are separately audited in automation-quick-menu-current-evidence.json.', 'Native focus, typography, pixel geometry and animation have not been run. CSS timing values do not prove rendered parity.']},
};
const outputPath = 'docs/re/automation-quick-keyboard-current-evidence.json';
const output = JSON.stringify(report, null, 2) + '\n';
if (process.argv.includes('--check')) fact(read(outputPath) === output, 'Stale quick keyboard audit');
else fs.writeFileSync(path.join(root,outputPath),output);
console.log('Automation quick keyboard: current manifest, capture session, key pills, icons and CSS statically verified.');
