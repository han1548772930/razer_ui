const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const {Source, walk, key} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');

const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const hash = file => crypto.createHash('sha256').update(fs.readFileSync(path.join(root, file))).digest('hex');
const source = '.ref/applications/feedback/static/js/496.003ef6c6.chunk.js';
const css = '.ref/applications/feedback/static/css/496.d9e63f8f.chunk.css';
const js = read(source);
const cssText = read(css);
const shell = read('src/shell.rs');
const service = read('src/shell/service_pages.rs');
const independent = read('src/shell/independent_window.rs');
const page = read('src/shell/feedback_page.rs');
const problems = [];
const app = new Source('feedback');
const component = app.binding(4496, 'ps');
const render = component.body.body.find(node => node.type === 'MethodDefinition' && key(node.key) === 'render');
const handlers = new Map();
walk(component, node => {
  if (node.type === 'AssignmentExpression' && node.left.type === 'MemberExpression'
      && node.left.object.type === 'ThisExpression' && /FunctionExpression$/.test(node.right.type)) {
    handlers.set(key(node.left.property), node.right);
  }
});
const categories = app.literal(4496, app.binding(4496, 'ds'));
const constants = Object.fromEntries(['lR', 'rI', 'GV', 'BM'].map(name => [name, app.literal(4166, app.exported(4166, name))]));
const cssRules = parseCSS(cssText);
const rule = selector => cssRules.find(row => row.selector === selector && !row.conditions.length);

if (!js.includes('class ps extends') || !js.includes('feedback-container')) problems.push('Feedback root component was not found');
if (!js.includes('maxLength:W.lR') || !js.includes('maxLength:W.rI')) problems.push('source input length constants are missing');
if (!js.includes('W.lR') || !js.includes('W.rI')) problems.push('source constants module is not wired');
if (!rule('.page-feedback>*')?.declarations.includes('width:500px')) problems.push('source 500px form width is missing');
if (!rule('.page-feedback .right .form')?.declarations.includes('align-items:center')) problems.push('source centered form geometry is missing');
const renderText = render && app.snippet(4496, render);
if (!renderText?.includes('this.renderMainPage()') || !renderText.includes('this.renderFooter()') || renderText.includes('this.renderLeft()')) {
  problems.push('Feedback root must mount only the current right form and footer');
}
if (JSON.stringify(Object.values(categories)) !== JSON.stringify([1, 3, 4])) problems.push('source categories changed');
if (constants.lR !== 255 || constants.rI !== 32000 || constants.GV !== 504) problems.push('source input constants changed');
if (!app.snippet(4496, handlers.get('inputOnChange')).includes('t.value.length>50')) problems.push('source 50 UTF-16 subject guard is missing');
if (!service.includes('native_page: Some(ModulePage::Feedback)')) problems.push('feedback module is not directly opened');
if (!service.includes('window: "feedback-synapse"')) problems.push('feedback window name is missing');
if (!shell.includes('ModulePage::Feedback') || !shell.includes('SharedString::from("feedback-synapse")')) problems.push('feedback independent window route is missing');
if (!independent.includes('FeedbackWindow') || !independent.includes('FeedbackPage')) problems.push('feedback window root is missing');
for (const token of ['FEEDBACK_SOURCE', 'TEXT_SELECT_TYPE', 'TEXT_SUBJECT', 'TEXT_DETAIL_YOUR_FEEDBACK', 'TEXT_SEND_LOG_FILE']) {
  if (!page.includes(token)) problems.push(`feedback page lacks ${token}`);
}
for (const token of ['32000', '255']) {
  if (!page.replace(/_/g, '').includes(token)) problems.push(`feedback page lacks local boundary ${token}`);
}
if (!/服务(?:未连接|尚未连接)/.test(page)) problems.push('feedback page lacks an explicit unavailable-service status');

const report = {
  schema_version: 1,
  source: {path: source, sha256: hash(source), module: 4496, component_offset: js.indexOf('class ps extends')},
  render: app.receipt(4496, render),
  handlers: Object.fromEntries(['inputOnChange', 'emailOnBlur', 'enableSubmit', 'renderApps', 'renderEmail', 'renderForm', 'renderFooter', 'handleBeforeSubmit'].map(name => [name, app.receipt(4496, handlers.get(name))])),
  categories,
  css: {path: css, sha256: hash(css), rules: cssRules.filter(row => row.selector.startsWith('.page-feedback') || row.selector.startsWith('.page.page-feedback') || row.selector.startsWith('textarea.form-control') || row.selector === '.email~.input-container>textarea.form-control')},
  constants: {loader_module: 3272, constants_module: 4166, values: constants, subject_guard: 50},
  local_boundary: 'Feedback form remains a local draft; submit and log download show unavailable-service status and do not call the remote API.',
  problems,
};
fs.writeFileSync(path.join(root, 'docs/re/feedback-app-current-evidence.json'), JSON.stringify(report, null, 2) + '\n');
console.log(`feedback audit: problems=${problems.length}`);
if (process.argv.includes('--check') && problems.length) {
  for (const problem of problems) console.error(`  ${problem}`);
  process.exit(1);
}
