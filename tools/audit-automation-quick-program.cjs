// Parse the current 3946 quick macro and verify the native Program draft.
// Never load or execute reference JavaScript. --check performs no writes.
const fs = require('fs');
const path = require('path');
const acorn = require('acorn');
const {walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const fact = (condition, message) => { if (!condition) throw Error(message); };
const one = (ast, predicate, label) => {
  const found = [];
  walk(ast, node => { if (predicate(node)) found.push(node); });
  fact(found.length === 1, `Expected one ${label}; found ${found.length}`);
  return found[0];
};
const manifestPath = '.ref/devices/3946/asset-manifest.json';
const manifest = JSON.parse(read(manifestPath));
const main = Object.values(manifest.files).find(file => /main\.[a-f0-9]+\.js$/.test(file));
fact(main, 'Current 3946 main bundle missing');
const file = `.ref/devices/3946/${main.replace(/^\.\//, '').replace(/^\//, '')}`;
const source = read(file), sourceHash = hash(source);
const ast = acorn.parse(source, {ecmaVersion: 'latest'});
const quick = one(ast, node => node.type === 'VariableDeclarator' && node.id.name === 'aH'
  && node.init?.start === 6920385, 'current aH binding').init;
fact(quick.end === 6931761, 'Current quick macro boundary changed');
const receipt = node => ({path: file, sha256: sourceHash, offset: node.start, end: node.end, source: source.slice(node.start, node.end)});
const callbacks = Object.fromEntries(['ie', 'ne', 'oe', 'te', 'ae', 'se', '_e', 'ce'].map(name => {
  const declaration = one(quick, node => node.type === 'VariableDeclarator' && node.id.name === name, `aH.${name}`);
  return [name, receipt(declaration.init)];
}));
const pickerCall = one(quick, node => node.type === 'CallExpression' && node.callee.type === 'MemberExpression'
  && key(node.callee.property) === 'showFileOpenDialog', 'program file picker');
fact(pickerCall.arguments[0]?.value === 'Open' && pickerCall.arguments[1]?.operator === '!'
  && pickerCall.arguments[1]?.argument.value === 1 && pickerCall.arguments[2]?.value === '[".exe"]', 'Program picker arguments changed');
fact(callbacks.ie.source.includes('if(!e)return') && callbacks.ie.source.includes('t.length>0&&P(t[0])'), 'Picker Cancel/first-selection contract changed');
fact(callbacks.ne.source === '()=>D("program")' && callbacks.oe.source === '()=>D("website")', 'Launch mode no longer retains both drafts');
fact(callbacks.te.source.includes('P("")') && callbacks.te.source.includes('y("")'), 'Close reset changed');
fact(callbacks.ae.source.includes('te(),a()'), 'Cancel/Close does not reset then close');
const jsxObject = name => one(quick, node => node.type === 'ObjectExpression'
  && node.properties.some(property => key(property.key) === 'className' && property.value.value === name), name);
const launch = jsxObject('quick-macro-launch');
const clear = jsxObject('quick-macro-clear');
const launchText = receipt(launch).source, clearText = receipt(clear).source;
for (const value of ['readOnly:!0,value:L', 'disabled:t,value:f,onChange:re', 'onClick:e?void 0:ie']) fact(launchText.includes(value), `Launch branch changed: ${value}`);
fact(clearText.includes('P("")') && clearText.includes('y("")') && clearText.includes('D("program")'), 'Clear no longer resets both launch fields');
const prior = JSON.parse(read('docs/re/automation-current-evidence.json')).components.find(component => component.offset === quick.start);
fact(prior?.source === receipt(quick).source, 'Main automation evidence no longer matches current aH');
const cssPath = '.ref/devices/3946/static/css/main.1aa32102.css';
fact(Object.values(manifest.files).some(file => file.endsWith('/main.1aa32102.css')), 'CSS is not declared by the current manifest');
const cssText = read(cssPath);
const css = parseCSS(cssText).filter(rule => rule.selector.includes('quick-macro-launch'))
  .map(rule => ({path: cssPath, sha256: hash(cssText), ...rule}));
const asset = JSON.parse(read('assets/synapse/automation-manifest.json'))
  .find(asset => asset.output === 'assets/synapse/automation-icon_folder.svg');
fact(asset && read(asset.source) === read(asset.output) && hash(read(asset.source)) === asset.source_sha256, 'Folder asset differs from current source');
const nativePath = 'src/features/automation/quick_macro.rs', native = read(nativePath), compact = native.replace(/\s+/g, '');
for (const fragment of ['program_path: String', 'website: Entity<InputState>', 'multiple: false', 'directories: false',
  'extension.eq_ignore_ascii_case("exe")', 'Ok(Ok(None)) => {}', 'this.picker_generation == generation',
  'self.picker_generation = self.picker_generation.wrapping_add(1)', 'self.program_path.clear()',
  'self.website.update(cx, |state, cx| state.set_value("", window, cx))', 'self.picker_task = Some(cx.spawn',
  '"quick-macro-program-browse"', 'self.program_path.trim().to_owned()',
  'cx.emit(QuickMacroSaved(result))', 'synapse/automation-icon_folder.svg']) {
  fact(compact.includes(fragment.replace(/\s+/g, '')), `Native Program contract missing: ${fragment}`);
}
fact(native.includes('this.browse_program(cx)'), 'Program picker is unreachable');
fact(!/std::process|Command::new|cx\.open_url|ShellExecute|LoadLibrary/.test(native), 'Quick macro unexpectedly executes or opens its target');
const report = {
  schema_version: 1,
  method: 'Current device-manifest main parsed with Acorn; callbacks, JSX, CSS and folder resource checked as static data.',
  source_manifest: {path: manifestPath, sha256: hash(read(manifestPath))},
  component: receipt(quick), callbacks, picker: receipt(pickerCall), launch: receipt(launch), clear: receipt(clear), css, asset,
  native: {path: nativePath, selection: 'One native file, accepted only with .exe extension (case insensitive). No executable is read or run.',
    program: 'Read-only full-path display and source folder icon activate the same picker. Long paths truncate visually; accessibility retains the path.',
    drafts: 'Program path and Website input have independent retained values. Switching modes only changes the active branch.',
    cancellation: 'Picker cancellation and invalid/error results retain the old path and all other draft fields. Dialog Cancel/Close drops the unsaved editor and emits no saved event.',
    clear: 'Source Clear deletes both launch drafts and returns to Program; selecting another macro type also resets the action. Generation checks reject late picker results after either reset.',
    pending: 'Retained picker task disables re-entry and Save. Closing drops the editor; the weak task cannot write to a later editor.',
    save: 'Save emits the selected local target only through QuickMacroSaved; parent editor retains its existing explicit save boundary.',
    limits: ['GPUI 0.7 PathPromptOptions exposes files/directories/multiple/prompt but no extension filter. Other file types may be visible in the OS dialog; native validation accepts .exe only.',
      'Actual picker errors add a local message and expand the popup to 409px; the ordinary source-size popup remains 369px.',
      'Native file-dialog behavior, focus restoration and visual layout have not been run. No application/build/tests/reference JavaScript/DLL execution.']},
};
const outputs = [
  ['docs/re/automation-quick-program-current-evidence.json', JSON.stringify(report, null, 2) + '\n'],
  ['docs/re/automation-quick-program-current-audit.md', [
    '# Automation 快捷宏 Program 选择器', '',
    '2026-10-04。当前 PID 3946 `aH`（6920385–6931761）调用 `showFileOpenDialog("Open", false, \'[".exe"]\')`，',
    '取消或空结果不改路径；Program 只读，整行与文件夹图标调用相同选择动作。Program/Website 分别保留草稿，切换不清空。', '',
    '原生实现已补同一入口、单文件选择、`.exe` 校验、独立草稿和源 Clear 行为。Clear 删除两个字段并回到 Program；',
    '改变宏类型清空动作，晚到的旧选择结果不能恢复已删除内容。原生选择器取消、错误或无效文件保留先前路径。',
    '保存只向上层发送本地宏配置，取消整个编辑器不发送保存事件；任何路径均不执行。', '',
    'GPUI 路径选择接口没有扩展名过滤参数，因此系统对话框可能显示其他文件类型，返回后只接受 `.exe`。',
    '发生真实选择错误时显示提示，弹框从源 369px 增至 409px 容纳信息；普通状态保持 369px。',
    '源文件夹资源逐字节复核；没有新增图标替代。', '',
    '`node tools/audit-automation-quick-program.cjs --check` 静态解析当前源码并比较证据，不写文件。',
    '未运行应用、构建、测试、下载 JavaScript 或 DLL；系统选择器、焦点和像素仍待运行验证。',
    '证据见 [automation-quick-program-current-evidence.json](automation-quick-program-current-evidence.json)。', '',
  ].join('\n')],
];
if (process.argv.includes('--check')) {
  for (const [file, expected] of outputs) fact(read(file) === expected, `Stale quick Program evidence: ${file}`);
  console.log('Automation quick Program: current picker/draft/clear evidence verified; no files written.');
} else {
  for (const [file, output] of outputs) fs.writeFileSync(path.join(root, file), output);
  console.log('Automation quick Program: generated current picker/draft/clear evidence.');
}
