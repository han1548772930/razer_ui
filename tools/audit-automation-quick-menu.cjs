// 3946 aH/tH dropdown: current manifest -> mounted JSX -> ordered CSS.
// Downloaded JavaScript is parsed only. --check performs no writes.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const fact = (value, message) => { if (!value) throw Error(message); };
function one(ast, predicate, label) {
  const found = []; walk(ast, node => { if (predicate(node)) found.push(node); });
  fact(found.length === 1, `${label}: expected one node; found ${found.length}`);
  return found[0];
}
const manifestPath = '.ref/devices/3946/asset-manifest.json';
const manifest = JSON.parse(read(manifestPath));
const main = Object.values(manifest.files).find(file => /main\.[a-f0-9]+\.js$/.test(file));
fact(main, 'Current 3946 main absent');
const sourcePath = '.ref/devices/3946/' + main.replace(/^\.\//, '');
const source = read(sourcePath), ast = acorn.parse(source, {ecmaVersion:'latest'});
const receipt = node => ({path:sourcePath, sha256:hash(source), offset:node.start, end:node.end, source:source.slice(node.start,node.end)});
const component = one(ast, node => node.type === 'VariableDeclarator' && node.id.name === 'aH' && node.init?.start === 6920385, 'current aH').init;
const icon = one(ast, node => node.type === 'VariableDeclarator' && node.id.name === 'tH' && node.init?.start === 6917447, 'current tH').init;
const types = one(ast, node => node.type === 'VariableDeclarator' && node.id.name === 'Jv' && node.start > 6910000 && node.start < 6920000, 'type table');
const propertyClass = (node, name) => node.type === 'ObjectExpression' && node.properties.some(p => key(p.key) === 'className' && p.value.value === name);
const control = one(component, node => propertyClass(node, 'macro-type-select__control'), 'control');
const menu = one(component, node => propertyClass(node, 'macro-type-select__menu'), 'menu');
const chevron = one(component, node => propertyClass(node, 'macro-type-select__chevron'), 'chevron');
const mount = one(component, node => node.type === 'LogicalExpression' && node.operator === '&&' && node.left.name === 'k' && node.right.type === 'CallExpression' && node.right.arguments[1] === menu, 'conditional menu mount');
const outside = one(component, node => node.type === 'ArrowFunctionExpression' && source.slice(node.start,node.end).startsWith('()=>{if(!k)return;'), 'outside effect');
fact(receipt(control).source.includes('onClick:()=>z(e=>!e)'), 'Trigger toggle changed');
fact(receipt(menu).source.includes('Jv.map(') && receipt(menu).source.includes('e.value===l?"selected":""'), 'Menu order/selection changed');
fact(receipt(menu).source.includes('I(t),S([]),D("program"),P(""),y(""),B(""),Y(""),void z(!1)'), 'Type choice no longer clears action and closes');
fact(receipt(outside).source.includes('ee.current&&!ee.current.contains(e.target)&&z(!1)') && receipt(outside).source.includes('document.addEventListener("mousedown",e)'), 'Outside behavior changed');
fact(!/onKeyDown|onKeyUp|setTimeout|requestAnimationFrame/.test(receipt(menu).source), 'Menu gained keyboard or delayed mounting behavior');
const css = [];
for (const file of Object.values(manifest.files).filter(file => /\.css$/.test(file))) {
  const cssPath = '.ref/devices/3946/' + file.replace(/^\.\//, ''), raw = read(cssPath);
  css.push(...parseCSS(raw).filter(rule => /macro-type-select|quick-macro-row|quick-macro-label/.test(rule.selector) || rule.selector === '.quick-macro-popup__body button,.quick-macro-popup__body span').map(rule => ({path:cssPath, sha256:hash(raw), ...rule})));
}
const rules = selector => css.filter(rule => rule.selector === selector);
const property = (selector, name) => rules(selector).flatMap(rule => rule.properties).filter(p => p.property === name).at(-1)?.value;
fact(property('.macro-type-select__option','padding') === '8px 12px', 'Final option padding changed');
fact(property('.macro-type-select__option','min-height') === '30px', 'Option minimum changed');
fact(property('.macro-type-select__control','height') === '27px' && property('.macro-type-select__control','line-height') === '17px', 'Control geometry changed');
fact(property('.macro-type-select__chevron','width') === '29px' && property('.macro-type-select__chevron','height') === '25px' && property('.macro-type-select__chevron','background-size') === '10px', 'Chevron geometry changed');
fact(property('.macro-type-select__chevron','transition') === 'transform .3s', 'Chevron timing changed');
fact(property('.macro-type-select.open .macro-type-select__menu','height') === 'auto' && property('.macro-type-select.open .macro-type-select__menu','max-height') === '180px', 'Open menu geometry changed');
fact(property('.macro-type-select__menu','margin-top') === '1px', 'Menu gap changed');
const assets = JSON.parse(read('assets/synapse/automation-manifest.json')).filter(asset => asset.output.includes('automation-quick-macro-') && !asset.output.endsWith('-delete.svg') || asset.output.endsWith('automation-icon_expand.svg'));
fact(assets.length === 5, 'Expected four type icons and current chevron');
for (const asset of assets) fact(hash(read(asset.source)) === asset.source_sha256 && hash(read(asset.output)) === asset.output_sha256, `Changed current asset ${asset.output}`);
const nativePath = 'crates/razer-pages/src/features/automation/quick_macro.rs', native = read(nativePath), compact = native.replace(/\s+/g,'');
fact(!/Select::new|SelectState|SelectEvent|SelectItem/.test(native), 'Framework Select is still rendered or retained');
for (const fragment of ['fn type_selector', 'if self.type_menu_open', 'this.type_menu_open = !this.type_menu_open', 'self.type_menu_open = false', 'type_trigger_bounds.contains(&event.position)', 'Duration::from_millis(300)', '.easing(motion::Easing::Ease)', 'std::f32::consts::PI', 'automation-icon_expand.svg', '.w(surface::css(29.)).h(surface::css(25.))', '.size(surface::css(10.))', '.max_h(surface::css(180.))', '.px(surface::css(12.)).py(surface::css(8.))', '.min_h(surface::css(30.))', 'item.content(self.kind == item.value)', 'item.content(false)', 'this.set_kind(item.value, window, cx)', 'deferred(menu).priority(3)']) {
  fact(compact.includes(fragment.replace(/\s+/g,'')), `Missing native menu contract: ${fragment}`);
}
const nativeSelector = native.slice(native.indexOf('fn type_selector'), native.indexOf('fn next_name_for'));
fact(!/IconName::Check|Presence::|Sequence::|with_animation|from_millis\(200\)/.test(nativeSelector), 'Unproven menu checkmark or entrance/exit animation');
const data = JSON.parse(read('crates/razer-pages/src/features/automation_data.json')).quick_macro_types;
let previousType = -1;
for (const type of data) {
  const index = compact.indexOf(`value:"${type.value}",label:"${type.label}"`.replace(/\s+/g,''));
  fact(index > previousType, 'Native type labels/order changed'); previousType = index;
}
const report = {
  method:'Current manifest and Acorn JSX; full selector, conditional mount, tH, Jv, click-away effect and ordered CSS. Reference code never executes.',
  manifest:{path:manifestPath,sha256:hash(read(manifestPath))}, component:receipt(component), control:receipt(control), menu:receipt(menu), mounted_by:receipt(mount), chevron:receipt(chevron), outside:receipt(outside), icons:receipt(icon), types:receipt(types), css, assets,
  native:{path:nativePath,
    selection:'Source order and original colored tH icons. Only the committed menu row text is green; the trigger stays #ccc. No framework checkmark, automatic list cursor or extra listbox key commands.',
    geometry:'27px trigger, 17px line-height, 29x25 chevron slot and 10px original SVG background. Final option padding is 8px/12px; 20px artwork yields 36px rows. Black menu has #515151 border, 1px gap, width max(trigger, measured content), max-height 180px, horizontal clipping and vertical overflow.',
    motion:'Persistent trigger border and arrow use 300ms ease. The source menu node is conditional k&&JSX, so it mounts in the already-open selector and unmounts immediately; declared height .2s/max-height .2s is not translated into an invented mounted closed-state animation.',
    input:'Native buttons provide Tab, Enter and Space. Trigger toggles; outside mousedown closes except the trigger. Selecting even the same type clears the action, closes the menu and restores native trigger focus; root Escape retains the original quick-macro close/capture condition.',
    limits:['Browser-default button padding and baseline layout are not measured; the native trigger retains a 6px horizontal button inset.', 'Original browser focus after removal of its active option differs from native explicit trigger focus restoration.', 'Rendered clipping, font metrics, tab traversal, viewport placement and animation are unverified because app/build/test execution is prohibited.'],
  },
};
const target='docs/re/automation-quick-menu-current-evidence.json', output=JSON.stringify(report,null,2)+'\n';
if (process.argv.includes('--check')) fact(read(target)===output, 'Stale quick menu evidence');
else fs.writeFileSync(path.join(root,target),output);
console.log('Current 3946 quick menu: JSX mount, option cascade, selected text, original icons and persistent trigger transitions statically verified.');
