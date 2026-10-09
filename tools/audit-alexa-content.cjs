// Current Alexa mounted body and tooltip. Parse vendor bytes; never execute them.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {walk, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const directory = '.ref/applications/synapse/alexa';
const manifest = JSON.parse(read(`${directory}/asset-manifest.json`));
const main = `${directory}/${manifest.files['main.js'].replace(/^\.\//, '')}`;
const stylesheet = `${directory}/${manifest.files['main.css'].replace(/^\.\//, '')}`;
const source = read(main), css = read(stylesheet);
const ast = acorn.parse(source, {ecmaVersion: 'latest'});
const scopes = [];
walk(ast, node => {
  if (!/FunctionExpression$/.test(node.type) || node.body.type !== 'BlockStatement') return;
  if (node.body.body.some(statement => statement.type === 'VariableDeclaration'
      && statement.declarations.some(d => d.id.name === 'zE'))) scopes.push(node);
});
if (scopes.length !== 1) throw Error('Ambiguous Alexa outer bootstrap scope');
const bindings = new Map();
for (const statement of scopes[0].body.body) {
  if (statement.type === 'VariableDeclaration') {
    for (const d of statement.declarations) if (d.id.type === 'Identifier') bindings.set(d.id.name, d);
  } else if (['FunctionDeclaration', 'ClassDeclaration'].includes(statement.type)) {
    bindings.set(statement.id.name, statement);
  }
}
const names = ['ch', 'QE', 'zE', 'Jr', 'qr', 'Zr', 'Kr', 'Bp', 'Yp', 'zp',
  'yE', 'sE', 'oE', 'Jp', 'OE', 'vE'];
const components = names.map(name => {
  const node = bindings.get(name);
  if (!node) throw Error(`Missing bootstrap binding ${name}`);
  const snippet = source.slice(node.start, node.end);
  return {name, path: main, offset: node.start, end: node.end, sha256: hash(snippet), source: snippet};
});
const component = name => components.find(item => item.name === name).source;
const problems = [];
const requireText = (text, contract, context) => {
  if (!text.includes(contract)) problems.push(`${context}: missing ${contract}`);
};
for (const [name, contracts] of Object.entries({
  ch: ['(0,vt.jsx)(QE,{})'],
  QE: ['e=>!!e.update.installData', '(0,vt.jsx)(zE,{})'],
  zE: ['name:"home"', '(0,vt.jsx)(Bp,', 'name:"skills"', '(0,vt.jsx)(Yp,',
    'name:"settings"', '(0,vt.jsx)(yE,', 'name:"help"', '(0,vt.jsx)(OE,',
    '(0,vt.jsx)(qr,{className:"page-"', '(t&&r?te:ne)'],
  Jr: ['className:"main"', 'className:"wrapper"', '(0,vt.jsx)(Zr,'],
  Yp: ['"synapseSkills",!0', 'maxHeight:r.scrollHeight', 'e.content.map', 'className:"btn-more"'],
  yE: ['(0,vt.jsxs)(sE,{position:"bottom-left"', 'TEXT_TOOLTIPS_1', 'TEXT_TOOLTIPS_2'],
  sE: ['onMouseEnter:()=>i(!0)', 'onMouseLeave:()=>i(!1)', 'window.addEventListener("blur",e)', 'children:["?"'],
  oE: ['showClassName:"show"})},100)', 'position:"fixed",left:e.x,top:e.y+90', 'showClassName:""', 'style:{}})},100)'],
  Jp: ['document.getElementById("pageContainer")', 'x:i.left-o.left,y:i.top-o.top'],
})) for (const contract of contracts) requireText(component(name), contract, name);
const rules = parseCSS(css).filter(rule => /^(body|html|\.page(?:[>., :{-]|$)|\.page-(home|skills|settings)|\.page-desc|\.text-cancel|\.main-container$|\.body-wrapper$|\.tooltip-razer(?:[> .-]|$)|\.tooltip-btn|\.checkbox-razer|\.help-component|\.widget-col|\.body-widgets)/.test(rule.selector));
const finalBody = rules.filter(rule => rule.selector.split(',').includes('body')).at(-1);
const fontFaces = [...css.matchAll(/@font-face\{([^{}]*)\}/g)]
  .filter(match => match[1].includes('font-family:RazerF5;'))
  .map(match => ({offset: match.index, declarations: match[1]}));
if (!fontFaces.some(face => face.declarations.includes('font-weight:100;')
    && face.declarations.includes('RazerF5-Thin.woff2'))
    || fontFaces.some(face => face.declarations.includes('font-weight:200;'))) {
  problems.push('RazerF5 Thin/200 fallback faces changed');
}
for (const contract of ['font-family:Roboto,sans-serif', 'font-size:16px', 'color:#ccc']) {
  requireText(finalBody.declarations, contract, 'final body cascade');
}
const nativePaths = ['crates/razer-app-pages/src/alexa_page.rs', 'crates/razer-app-pages/src/alexa_page/sections.rs',
  'crates/razer-app-pages/src/alexa_page/controls.rs', 'crates/razer-widgets/src/source_tooltip.rs'];
const native = nativePaths.map(p => ({path: p, sha256: hash(read(p))}));
const local = read(nativePaths[0]), sections = read(nativePaths[1]), controls = read(nativePaths[2]), tooltip = read(nativePaths[3]);
for (const contract of ['.font_family("Roboto")', '.text_size(css(16.))', 'window.rem_size() * (42. / 16.)', '.mt(css(19.52))']) requireText(local, contract, 'native Alexa root/home');
for (const contract of ['"more-color"', '"more-opacity"', '.pl(css(20.))', '.left(css(6.))', 'controls::settings_help()']) requireText(sections, contract, 'native skills/settings');
for (const contract of ['SourceTooltipKind::Alexa', '"alexa-help-highlight"', '.child("?")']) requireText(controls, contract, 'native tooltip trigger');
for (const contract of ['kind == SourceTooltipKind::Alexa', 'observe_window_activation', '.delay(Duration::from_millis(', '.px(surface::css(8.))', '.py(surface::css(7.))']) requireText(tooltip, contract, 'native tooltip portal');
const iconFiles = [
  ['basic-lighting', 'lighting'], ['chroma-lighting', 'chroma'],
  ['launch-application', 'launch'], ['multimedia', 'media'], ['power', 'power'],
].map(([skill, output]) => {
  const rule = rules.find(rule => rule.selector === `.page-skills .collapsible.${skill}>.header>.icon`);
  const url = rule.properties.find(p => p.property === 'background-image').value.match(/url\(([^)]+)\)/)[1];
  const sourcePath = path.posix.normalize(`${path.posix.dirname(stylesheet)}/${url}`);
  const targetPath = `assets/synapse/alexa-skill-${output}.svg`;
  const original = hash(fs.readFileSync(path.join(root, sourcePath)));
  const target = hash(fs.readFileSync(path.join(root, targetPath)));
  if (original !== target) problems.push(`Skill icon differs: ${skill}`);
  return {skill, source: sourcePath, output: targetPath, source_sha256: original, output_sha256: target};
});
const report = {
  schema_version: 1, method: 'Acorn outer bootstrap lexical scope, mounted JSX references, CSS with enclosing conditions; no vendor execution',
  source: {path: main, sha256: hash(source)}, stylesheet: {path: stylesheet, sha256: hash(css)},
  components, css: rules, razer_font_faces: fontFaces, skill_icons: iconFiles, native,
  verified: ['installed root ch/QE/zE -> Jr/Zr/Kr -> Home/Skills/Settings/Help',
    'final body cascade is Roboto 16px/#ccc with inherited line-height 1.22',
    'Skills list text starts 20px inside content; 100ms more-label color/opacity',
    'Settings tooltip bottom-left 300px main, intrinsic wrapper 7x8 padding, 100ms delayed show and 100ms linear fade, blur dismiss',
    'source Home 80vh uses host WebContents below 42px TabUI'],
  limitations: ['No application execution or visual acceptance',
    'Skill disc marker uses a native circle; browser font-specific marker rasterization is not reproduced',
    'Home preview modal viewport remains a local preview adaptation',
    'Authentication, input devices, persistent source settings and service responses remain unconnected',
    'Help layout/navigation, installer, logout and patch-note dialogs need separate full content re-audit'],
  problems,
};
const output = 'docs/re/alexa-content-current-evidence.json';
const serialized = JSON.stringify(report, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (read(output) !== serialized) throw Error(`Stale ${output}`);
} else fs.writeFileSync(path.join(root, output), serialized);
if (problems.length) throw Error(problems.join('\n'));
process.stdout.write(`Alexa content: ${components.length} scoped bindings, ${rules.length} CSS rules, ${iconFiles.length} original icons, problems=0\n`);
