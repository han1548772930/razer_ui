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
const hostTabs = read('src/shell/host_tabs.rs');
const page = read('src/shell/feedback_page.rs');
const select = read('src/ui/synapse_select.rs');
const problems = [];
const app = new Source('feedback');
const component = app.binding(4496, 'ps');
const render = component.body.body.find(node => node.type === 'MethodDefinition' && key(node.key) === 'render');
const handlers = new Map();
for (const method of component.body.body) {
  if (method.type === 'MethodDefinition') handlers.set(key(method.key), method);
}
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
if (!shell.includes('ModulePage::Feedback')) problems.push('feedback native module route is missing');
if (!shell.includes('feedback_page: Option<Entity<feedback_page::FeedbackPage>>')
    || !/if self\.feedback_page\.is_none\(\)\s*\{\s*self\.feedback_page\s*=\s*Some\(cx\.new\(\|cx\|\s*feedback_page::FeedbackPage::new\(window, cx\)/.test(shell)) {
  problems.push('Feedback must retain its page entity when opening or returning to the host tab');
}
if (!/Location::Feedback\s*=>\s*self\s*\.feedback_page\s*\.as_ref\(\)\s*\.map\(\|page\| page\.clone\(\)\.into_any_element\(\)\)/.test(shell)) {
  problems.push('Location::Feedback must render the retained native page');
}
if (!hostTabs.includes('Self::Feedback => "feedback-synapse".into()')
    || !hostTabs.includes('Self::Feedback => Location::Feedback')
    || !hostTabs.includes('Location::Feedback => Some(Self::Feedback)')) {
  problems.push('Feedback host tab must preserve the feedback-synapse route identity');
}
for (const token of ['FEEDBACK_SOURCE', 'TEXT_SELECT_TYPE', 'TEXT_SUBJECT', 'TEXT_DETAIL_YOUR_FEEDBACK', 'TEXT_SEND_LOG_FILE']) {
  if (!page.includes(token)) problems.push(`feedback page lacks ${token}`);
}
for (const token of ['32000', '255']) {
  if (!page.replace(/_/g, '').includes(token)) problems.push(`feedback page lacks local boundary ${token}`);
}
if (!/服务(?:未连接|尚未连接)/.test(page)) problems.push('feedback page lacks an explicit unavailable-service status');

// Resolve from this current class and its imported constants, not historical
// symbol names or assertions that merely find a locale string in the bundle.
const body = name => app.snippet(4496, handlers.get(name));
const requireSource = (name, fragments) => {
  for (const fragment of fragments) if (!body(name).includes(fragment)) {
    problems.push(`${name} no longer contains ${fragment}`);
  }
};
requireSource('renderForm', ['this.renderApps(),this.renderEmail(),this.renderSynapseDevices(),this.renderPrivacyEnquiry(),this.renderSubmittingForm()', 'o&&o.length||0', 'this.renderSendLogCheckBox()']);
requireSource('renderEmail', ['!1!==t&&s!==ds.PRIVACY_ENQUIRE', '"TEXT_REQUIRE":"TEXT_OPTIONAL"', 'maxLength:W.lR']);
requireSource('inputOnChange', ['t.value.length>50?this.state[t.name]||"":t.value', '"email"===t.name&&t.value?{errorEmail:!gn(t.value)}:null']);
requireSource('emailOnBlur', ['this.state.category===ds.CUSTOMER_SUPPORT||s', 'n=!s||!gn(s)']);
requireSource('enableSubmit', ['(!s||gn(s))&&!hn(o)&&!hn(a)', '!0!==this.state.isGuest||gn(s)']);
requireSource('renderPrivacyEnquiry', ['TEXT_PRIVACY_ENQUIRES_DESCRIPTION_2', 'TEXT_PRIVACY_ENQUIRES_DESCRIPTION_3', 'RazerApp.openInDefaultBrowser(W.to)']);
requireSource('handleBeforeSubmit', ['!this.state.withLogs&&this.isCheckLogNeeded(this.state.category)', 'showLogRequestForm:!0']);
requireSource('renderSubmittingForm', ['TEXT_SUBMITTING_FORM_ACTION_1', 'TEXT_SUBMITTING_FORM_ACTION_2', 'withLogs:!1', 'withLogs:!0']);
requireSource('isCheckLogNeeded', ['[ds.REPORT_BUG_TAB,ds.CUSTOMER_SUPPORT].includes(e)']);
requireSource('onSubmit', ['Jn({appName:t,category:n,email:s,title:o,description:a,device:i,withLogs:r,deviceName:l})']);
const sendFeedback = app.binding(4496, 'Jn');
const collectLogs = app.binding(4496, 'qn');
const sendText = app.snippet(4496, sendFeedback);
const logsText = app.snippet(4496, collectLogs);
for (const fragment of ['if(d){const e=await qn()', 'url:"/api/feedback/report/submit"', 'method:"POST"', 'zip_log:p']) {
  if (!sendText.includes(fragment)) problems.push(`current feedback service changed: ${fragment}`);
}
for (const fragment of ['s+r>52428800', '"file size is too big, skip"', 'he.A.getLogFiles()', '"localStorage.json"', '"macros.json"']) {
  if (!logsText.includes(fragment)) problems.push(`current generated log archive changed: ${fragment}`);
}
const fileInputs = [];
walk(component, node => {
  if (node.type === 'Property' && key(node.key) === 'type' && node.value.type === 'Literal' && node.value.value === 'file') {
    fileInputs.push(app.receipt(4496, node));
  }
});
if (fileInputs.length) problems.push('Feedback gained a file input; re-audit attachment selection and validation');
const privacy = rule('.page-feedback .right .bug-bounty');
const logPopup = rule('.page-feedback .submitting-form');
if (!['bottom:98px', 'height:175px', 'width:520px'].every(value => logPopup?.declarations.includes(value))) {
  problems.push('Log confirmation source geometry changed');
}
if (!['.pb(surface::css(98.))', '.h(surface::css(175.))', '.w(surface::css(520.))',
  'gpui_kit::base::DialogPopup::new().child(panel)'].every(value => page.includes(value))) {
  problems.push('Log confirmation must retain its source position and occluding popup');
}
if (!['position:absolute', 'top:35px', 'bottom:0', 'z-index:1'].every(fragment => privacy?.declarations.includes(fragment))) {
  problems.push('Privacy must cover the form beneath its category selector');
}
if (!rule('.input-container.privacy-enquire')?.declarations.includes('flex:0 0 50%')) problems.push('privacy category half-width changed');
if (!rule('.email~.input-container>textarea.form-control')?.declarations.includes('height:213px')) problems.push('guest textarea height changed');
const emailValidator = app.binding(4496, 'gn');
let sourceEmailPattern;
walk(emailValidator, node => { if (node.regex) sourceEmailPattern = node.regex.pattern; });
const expectedEmailPattern = "^[-!#$%&'*+/0-9=?A-Z^_a-z{|}~](\\.?[-!#$%&'*+/0-9=?A-Z^_a-z`{|}~])*@[a-zA-Z0-9](-*\\.?[a-zA-Z0-9])*\\.[a-zA-Z](-?[a-zA-Z0-9])+$";
if (sourceEmailPattern !== expectedEmailPattern) problems.push('source email grammar changed; re-audit native ASCII parser');
const urls = Object.fromEntries(['to', 'v7'].map(name => [name, app.literal(4166, app.exported(4166, name))]));
for (const url of Object.values(urls)) if (!page.includes(url)) problems.push(`missing current privacy URL ${url}`);
const requiredLocal = [
  '.validate(|value, _| utf16_len(value) <= TITLE_LIMIT)',
  'clip_utf16(&value, EMAIL_LIMIT)', 'clip_utf16(&value, DESCRIPTION_LIMIT)',
  'InputEvent::Blur', 'has_content(&self.title.read(cx).value())',
  'has_content(&self.description.read(cx).value())', 'fn email_placeholder(required: bool)',
  'let Some((local, domain)) = value.split_once(\'@\')',
  'domain.rsplit_once(\'.\')', 'byte.is_ascii_alphanumeric()',
  '.placeholder(tr("TEXT_SELECT_TYPE"))', '.placeholder(tr("TEXT_DEVICE_NAME"))',
  'return root.child(self.privacy(cx)).into_any_element()',
  'TEXT_PRIVACY_ENQUIRES_DESCRIPTION_2', 'TEXT_PRIVACY_ENQUIRES_DESCRIPTION_3',
  'TEXT_SEND_LOG_FILE_HELPER', 'TEXT_SUBMITTING_FORM_ACTION_1', 'TEXT_SUBMITTING_FORM_ACTION_2',
  'self.confirm_logs', 'self.show_log_request', 'FeedbackColors::error()',
  '.replace("{{a}}", "[")', 'fn linked_copy',
  'gpui_kit::base::DialogPopup::new().child(panel)',
  '.close_on_backdrop_press(true)', 'self.submit_focus.focus(window, cx)',
  'pub(super) fn focus(&mut self, window: &mut Window, cx: &mut Context<Self>)',
];
// Listener closures name their owner `this`; avoid depending on formatting.
requiredLocal[requiredLocal.indexOf('self.confirm_logs')] = 'this.confirm_logs';
for (const fragment of requiredLocal) if (!page.includes(fragment)) problems.push(`missing native contract: ${fragment}`);
if (page.includes('&& !self.error_email')) problems.push('stale visual error must not override source enableSubmit');
if (page.includes('clip_utf16(&value, TITLE_LIMIT)')) problems.push('source rejects an overlength subject edit instead of clipping it');
if (!select.includes('placeholder: Option<SharedString>') || !select.includes('view.placeholder = self.placeholder')
    || !select.includes('self.placeholder.clone().unwrap_or_default()')) problems.push('select placeholder must remain presentation, not an option');

const report = {
  schema_version: 3,
  source: {path: source, sha256: hash(source), module: 4496, component_offset: js.indexOf('class ps extends')},
  render: app.receipt(4496, render),
  handlers: Object.fromEntries(['getCategoryById', 'categoryOnClick', 'inputOnChange', 'emailOnBlur', 'enableSubmit', 'renderApps', 'renderEmail', 'renderSynapseDevices', 'renderPrivacyEnquiry', 'renderSubmittingForm', 'renderSendLogCheckBox', 'renderForm', 'renderFooter', 'handleBeforeSubmit', 'isCheckLogNeeded', 'onSubmit'].map(name => [name, app.receipt(4496, handlers.get(name))])),
  validation: {email: app.receipt(4496, emailValidator), blank: app.receipt(4496, app.binding(4496, 'hn')), utf16_units: true, guest_only_locally: true},
  service: {
    submit: app.receipt(4496, sendFeedback),
    collect_logs: app.receipt(4496, collectLogs),
    attachments: {file_inputs_in_component: fileInputs, generated_zip_field: 'zip_log', accumulated_entry_compressed_limit_bytes: 52428800, overflow_behavior: 'Skip the entry whose compressed size would exceed the running sum; this is not a user attachment size validator or a final ZIP size guarantee.'},
    native_implementation: 'Unavailable service boundary; no log collection or feedback POST is invoked.',
  },
  privacy_urls: urls,
  categories,
  css: {path: css, sha256: hash(css), rules: cssRules.filter(row => row.selector.startsWith('.page-feedback') || row.selector.startsWith('.page.page-feedback') || row.selector.startsWith('textarea.form-control') || row.selector.startsWith('.input-container') || row.selector === '.email~.input-container>textarea.form-control')},
  constants: {loader_module: 3272, constants_module: 4166, values: constants, subject_guard: 50},
  local_boundary: 'Feedback remains a local guest draft. Submit (including either log choice) reports unavailable; no remote API or log collection runs. The unmounted renderLeft log-export controls are not reproduced. Privacy uses its current source URLs; service success is not synthesized.',
  native_page: {path: 'src/shell/feedback_page.rs', sha256: hash('src/shell/feedback_page.rs'), popup: 'GPUI Base DialogPopup provides popup hit-test occlusion; Dialog owns Escape, backdrop cancellation and focus trapping.'},
  native_route: {shell: 'src/shell.rs', tabs: 'src/shell/host_tabs.rs', location: 'Location::Feedback', identity: 'feedback-synapse', retained_entity: 'feedback_page', host_policy: 3, container: 'Named tab in the existing host window.'},
  verification_scope: 'Static AST, CSS, native source fragments and routing checks; no downloaded JavaScript, application or tests executed. Fragment checks are not an execution-equivalence proof. Dialog pixel geometry, popup hit testing, keyboard focus restoration and link rendering still require an authorized runtime review.',
  problems,
};
fs.writeFileSync(path.join(root, 'docs/re/feedback-app-current-evidence.json'), JSON.stringify(report, null, 2) + '\n');
console.log(`feedback audit: problems=${problems.length}`);
if (process.argv.includes('--check') && problems.length) {
  for (const problem of problems) console.error(`  ${problem}`);
  process.exit(1);
}
