// Current 3946 AutomationModal → EH deletion, parsed without executing source JS.
// --check only compares prepared assets/evidence; it never writes files.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), read = file => fs.readFileSync(path.join(root, file), 'utf8');
const fact = (ok, message) => { if (!ok) throw Error(message); };
function one(ast, predicate, label) {
  const matches = [];
  walk(ast, node => { if (predicate(node)) matches.push(node); });
  fact(matches.length === 1, `Expected one ${label}, found ${matches.length}`);
  return matches[0];
}
const manifestPath = '.ref/devices/3946/asset-manifest.json', manifest = JSON.parse(read(manifestPath));
const main = Object.values(manifest.files).find(file => /main\.[a-f0-9]+\.js$/.test(file));
fact(main, 'Missing current 3946 main bundle');
const file = `.ref/devices/3946/${main.replace(/^\.\//, '')}`, source = read(file);
const ast = acorn.parse(source, {ecmaVersion: 'latest'});
const receipt = node => ({path: file, sha256: hash(source), offset: node.start, end: node.end, source: source.slice(node.start, node.end)});
const binding = (scope, name, offset) => one(scope, node => node.type === 'VariableDeclarator'
  && node.id.name === name && (offset === undefined || node.start === offset), name).init;
const modal = one(ast, node => node.type === 'ArrowFunctionExpression' && node.start === 6936081, 'current AutomationModal');
const eh = binding(ast, 'EH', 6934326), ql = binding(ast, 'qL', 6617943);
const zl = one(ast, node => node.type === 'FunctionDeclaration' && node.id.name === 'ZL', 'mounted modal wrapper');
fact(ql.name === 'ZL', 'qL no longer resolves to ZL');
const position = binding(modal, 'Je');
const pos = receipt(position).source;
for (const part of ['popWidth:300', 'popHeight:130', 'left:a.left+150', 'top:n.top-n.height-12']) fact(pos.includes(part), `Delete anchor changed: ${part}`);
const hasProp = (node, name) => node.type === 'ObjectExpression' && node.properties.some(prop => key(prop.key) === name);
const prop = (node, name) => node.properties.find(prop => key(prop.key) === name)?.value;
const config = one(modal, node => hasProp(node, 'modalStyle') && hasProp(node, 'confirm') && hasProp(node, 'backDropStyle'), 'EH mounted configuration');
const trigger = one(modal, node => hasProp(node, 'ref') && prop(node, 'ref').name === 'Ze', 'footer delete trigger');
const injected = one(modal, node => node.type === 'AssignmentExpression' && node.left.type === 'MemberExpression'
  && key(node.left.property) === 'innerHTML' && node.right.type === 'Literal' && node.right.value.includes('.automation-delete-dialog-title'), 'injected delete CSS').right;
const injectedCss = injected.value.match(/<style>([\s\S]*?)<\/style>/)[1];
const confirm = prop(prop(config, 'confirm'), 'click'), cancel = prop(prop(config, 'cancel'), 'click');
fact(receipt(confirm).source === '()=>(et(),Fe(null),n(!1),Xe(!1),Qe({type:le.XpF,payload:o.id}),Qe({type:le.HTo}),i(),!0)', 'Delete ordering changed');
fact(receipt(cancel).source === '()=>(i(),Xe(!1),!0)', 'Cancel now changes the draft or editor');
fact(prop(trigger, 'disabled').type === 'UnaryExpression' && prop(trigger, 'disabled').operator === '!' && prop(trigger, 'disabled').argument.name === 'r', 'Delete is no longer disabled when adding');
const backDrop = prop(config, 'backDropStyle');
fact(prop(backDrop, 'position').value === 'absolute' && prop(backDrop, 'background-color').value === 'inherit', 'Mounted backdrop override changed');
for (const fragment of ['onClick', 'onKeyDown', 'keydown', 'Escape']) {
  if (fragment === 'onClick') continue;
  fact(!receipt(eh).source.includes(fragment) && !receipt(zl).source.includes(fragment), `Mounted dialog keyboard behavior changed: ${fragment}`);
}
const backdropElement = one(eh, node => node.type === 'ObjectExpression' && prop(node, 'className')?.name === 'rH', 'EH backdrop JSX');
fact(!prop(backdropElement, 'onClick'), 'EH now dismisses on backdrop click');
const deletion = one(ast, node => node.type === 'SwitchCase' && node.test?.type === 'MemberExpression'
  && key(node.test.property) === 'XpF' && receipt(node).source.includes('automationWidget.filter'), 'delete reducer');
const resize = one(modal, node => node.type === 'CallExpression' && node.callee.type === 'SequenceExpression'
  && node.callee.expressions.at(-1)?.property?.name === 'useEffect'
  && receipt(node).source.includes('window.addEventListener("resize"'), 'delete resize listener');
const vM = binding(ast, 'vM', 6734460);
const svg = one(vM, node => node.type === 'ObjectExpression' && prop(node, 'viewBox')?.value === '0 0 10 12', 'actual delete SVG');
const svgPath = one(vM, node => node.type === 'ObjectExpression' && prop(node, 'd')?.type === 'Literal', 'delete SVG path');
fact(prop(svgPath, 'fill').value === 'currentColor', 'Delete SVG is no longer currentColor');
const iconPath = 'assets/synapse/automation-delete-action.svg';
const icon = `<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="${prop(svg, 'viewBox').value}" fill="none"><path d="${prop(svgPath, 'd').value}" fill="currentColor"/></svg>\n`;
const cssPath = '.ref/devices/3946/static/css/main.1aa32102.css', cssText = read(cssPath);
fact(Object.values(manifest.files).some(file => file.endsWith('/main.1aa32102.css')), 'Delete CSS is not current-manifest source');
const css = parseCSS(cssText).filter(rule =>
  /^\.thx-btn(?::|\{|\s|\.|$)/.test(rule.selector) || rule.selector === '.keymap-action.flex>div.thx-btn'
  || /^\.automation-footer/.test(rule.selector) || /^\.Dialog_(backDrop|modal)__[^ ]+$/.test(rule.selector)
  || ['.modal', '.modal-patch-notes .modal-footer', '.automation-modal.modal'].includes(rule.selector))
  .map(rule => ({path: cssPath, sha256: hash(cssText), ...rule}));
const nativePath = 'src/features/automation/delete_confirmation.rs', native = read(nativePath), compact = native.replace(/\s+/g, '');
for (const fragment of ['footer.origin.y - footer.size.height - surface::css(12.).to_pixels(window.rem_size())', 'let left = anchor.origin.x',
  '.w(surface::css(300.))', '.max_h(surface::css(130.))', '.h(surface::css(27.))', '.min_w(surface::css(100.))',
  '.on_cancel(|_, _, _| false)', '.on_ok(|_, _, _| false)', '.close_on_backdrop_press(false)',
  'DialogPopup::new()', 'if !self.editing || self.delete_confirmation.is_some()',
  'let id = this.original.id', 'window.close_dialog(cx); cx.emit(EditorEvent::Delete(id))',
  'this.delete_trigger_focus.focus(window, cx)', 'Duration::from_millis(200)',
  'if self.resolved', 'synapse/automation-delete-action.svg']) {
  fact(compact.includes(fragment.replace(/\s+/g, '')), `Missing native deletion contract: ${fragment}`);
}
const editor = read('src/features/automation/editor.rs');
fact(editor.includes('.children(self.delete_confirmation.clone())') && editor.includes('.child(self.delete_trigger(window, cx))')
  && editor.includes('self.delete_footer_bounds.clone()') && !editor.includes('self.deleting'), 'Delete confirmation is not mounted in the editor');
const owner = read('src/features/automation.rs');
fact(owner.includes('EditorEvent::Delete(id) =>') && owner.includes('this.rules.retain(|r| r.id != *id)')
  && owner.includes('self.editor = Some(editor.clone())'), 'Real automation deletion flow is missing');
const report = {
  schema_version: 1, method: 'Current 3946 webpack parsed with Acorn; mounted caller, EH, wrapper, reducer, literal CSS and SVG checked without execution.',
  manifest: {path: manifestPath, sha256: hash(read(manifestPath))},
  source: {modal: receipt(modal), dialog: receipt(eh), outer_modal: receipt(zl), position: receipt(position),
    trigger: receipt(trigger), configuration: receipt(config), confirm: receipt(confirm), cancel: receipt(cancel),
    resize: receipt(resize), reducer: receipt(deletion), icon: receipt(vM)},
  css, injected_css: {receipt: receipt(injected), rules: parseCSS(injectedCss)},
  cascade: {
    panel: 'Inline width 300, height auto, maxHeight 130, padding 20, background #111, border 1px #FD4949, radius 3. Dialog CSS retains translateX(-50%) and overflow-y:scroll.',
    position: 'Je.left=trigger.left+150 and CSS translateX(-50%) cancel at width 300; native left=trigger.left. Both use top=footer.top-footer.height-12 and refresh on layout/resize.',
    footer: '.modal-patch-notes .modal-footer specificity (0,2,0) beats .automation-footer (0,1,0): padding 10px 30px, border #555.',
    buttons: '.keymap-action.flex>div.thx-btn specificity (0,3,1) beats injected .delete-dialog-custom .thx-btn (0,2,0): 27px height, min-width 100px, 12px/14px text, black 1px border, radius 3, padding 6px 10px 7px 6px. Cancel has 10px right margin; confirm margin:0 !important.',
    button_colors: 'Injected Cancel #666→#555/white, Confirm #FD4949→#cc3333/black. .thx-btn hover/active opacity .8/.6; injected transition all .2s ease. Icon color has inline .2s ease transition and edited hover #ff4444.',
    backdrop: 'EH is appended to .automation-modal.modal, whose background is transparent and top is 109px; mounted background-color:inherit replaces generic black70%. It has no click handler.',
    keyboard: 'Neither mounted EH nor ZL defines Escape/Enter dismissal. Native consumes dialog Cancel/Confirm actions without deleting or closing the parent. Explicit buttons remain keyboard accessible.',
  },
  asset: {path: iconPath, sha256: hash(icon), source: 'Current vM SVG with supplied customWidth/customHeight=24; preserves 0 0 10 12 viewBox and currentColor path.'},
  native: {editor: 'src/features/automation/editor.rs', popup: nativePath,
    reachability: 'Actual 3946 Customize → existing automation row Edit → footer Delete. Add mode disables Delete.',
    focus: 'Retained Base Dialog/DialogPopup focus trap; initial focus Cancel. Cancel returns focus to original Delete button and retains all draft fields.',
    commit: 'One-shot decision guard; Confirm closes editor then emits Delete(original.id), matching source reset/close-before-remove ordering. No draft Save and no backend success are synthesized.',
    motion: 'The confirmation mounts/unmounts directly, matching EH. Only source button/icon transitions are implemented; existing outer editor animation is not claimed as newly verified.',
    limits: 'Read-only source and formatting validation only. Pixel layout, accessibility tree, pointer/keyboard dispatch and resize behavior have not been run.'},
};
const markdown = [
  '# Automation 编辑器锚定删除确认', '',
  '2026-10-04。当前 PID 3946 AutomationModal（6936081）通过 EH（6934329）把确认框追加到实际编辑器容器。',
  '垃圾桶引用与 footer 的 `getBoundingClientRect()` 决定位置：框左边等于按钮左边，顶部为 footer.top−footer.height−12；窗口变化重测。', '',
  '确认框宽 300px、自动高度且上限 130px，padding 20px、#111 背景、#FD4949 边框。较长翻译按源容器滚动。',
  '静态 CSS 层叠说明：footer 由 `.modal-patch-notes .modal-footer` 决定 10px/30px 间距与 #555 边框；',
  '确认按钮由 `.keymap-action.flex>div.thx-btn` 保持最小 100×27px，不能直接采用较低优先级注入规则的尺寸。', '',
  '原生编辑器现已使用锚定弹层；添加模式禁用删除。Cancel 只关闭确认并返回垃圾桶焦点，全部编辑草稿保留；',
  'Confirm 先关闭编辑器，再向真实规则列表发送 Delete(original.id)，一次决策只发送一次删除，不顺带保存草稿。',
  '透明遮挡层截获底层点击；EH/ZL 没有 Escape、Enter 或外部点击关闭处理，原生不让这些操作误关父编辑器。', '',
  '实际 vM 图标已按当前 AST 提取；其 viewBox 为 10×12、按钮传入 24×24，并使用 currentColor。',
  '图标和按钮沿用 .2s ease；确认框自身源码没有入场/退场动画，因此不添加。', '',
  '`node tools/audit-automation-delete.cjs --check` 只读比较当前挂载链、CSS、图标与实现证据。',
  '未运行应用、构建、测试、下载 JavaScript 或 DLL；像素、焦点、滚动、窗口缩放行为仍未经运行验证。',
  '完整收据见 [automation-delete-current-evidence.json](automation-delete-current-evidence.json)。', '',
].join('\n');
const resourceManifest = [{source: file, output: iconPath, source_sha256: hash(source), output_sha256: hash(icon),
  source_kind: 'inline_svg', source_symbol: 'vM', source_offset: vM.start, source_end: vM.end,
  source_view_box: prop(svg, 'viewBox').value, supplied_dimensions: [24, 24]}];
const outputs = [[iconPath, icon], ['assets/synapse/automation-delete-manifest.json', JSON.stringify(resourceManifest, null, 2) + '\n'], ['docs/re/automation-delete-current-evidence.json', JSON.stringify(report, null, 2) + '\n'], ['docs/re/automation-delete-current-audit.md', markdown]];
if (process.argv.includes('--check')) {
  for (const [file, expected] of outputs) fact(read(file) === expected, `Stale automation delete evidence/resource: ${file}`);
  console.log('Automation delete: current mounted dialog, cascade, resource and native route verified; no files written.');
} else {
  for (const [file, value] of outputs) fs.writeFileSync(path.join(root, file), value);
  console.log('Automation delete: generated current dialog evidence and exact inline icon.');
}
