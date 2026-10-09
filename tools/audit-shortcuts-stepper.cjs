// Current Dashboard Macro repeat Stepper: parse source and CSS as data only.
const fs = require('fs'), path = require('path');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), source = new Source('synapse/dashboard');
const manifestPath = '.ref/applications/synapse/dashboard/asset-manifest.json';
const manifest = JSON.parse(fs.readFileSync(path.join(root, manifestPath), 'utf8'));
const cssFiles = [...new Set(Object.values(manifest.files))].filter(file => file.endsWith('.css'))
  .map(file => `${source.directory}/${file.slice(2)}`);
const cssReceipts = cssFiles.map(file => {
  const bytes = fs.readFileSync(path.join(root, file));
  return {path: file, sha256: hash(bytes), rules: parseCSS(bytes.toString())
    .filter(rule => /\.(?:stepper|spinner)(?![\w-])/.test(rule.selector) || rule.selector === '.disabled')};
});
const css = cssReceipts.find(file => file.path.endsWith('/55.4e8559cb.chunk.css'));
if (!css || cssReceipts.some(file => file !== css && file.rules.length)) {
  throw Error('Review changed Stepper stylesheet scope/order');
}
let propsNode;
walk(source.binding(82508, 'k'), node => {
  if (node.type === 'ObjectExpression' && node.properties.some(property =>
    key(property.key) === 'extraClass' && property.value.value === 'custom-keymapping-stepper')) {
    if (propsNode) throw Error('Multiple Macro repeat Stepper mounts');
    propsNode = node;
  }
});
if (!propsNode) throw Error('Missing current Macro repeat Stepper mount');
const props = Object.fromEntries(propsNode.properties.filter(property =>
  !['value', 'setParentState'].includes(key(property.key)))
  .map(property => [key(property.key), source.literal(82508, property.value)]));
const expected = {active:true, maxLength:2, maxValue:99, minValue:1, allowDecimals:false,
  stepValue:1, extraClass:'custom-keymapping-stepper', allowLiveUpdate:true};
if (JSON.stringify(props) !== JSON.stringify(expected)) throw Error('Macro repeat props changed');
// This module is duplicated across lazy chunks with different local names.
// Resolve the export in the same parsed chunk as the Macro caller.
if (source.module(44230).file !== source.module(82508).file) throw Error('Stepper/caller chunk changed');
let stepperNode = source.exported(44230,'A');
const visited = new Set();
while (stepperNode.type === 'Identifier') {
  if (visited.has(stepperNode.name)) throw Error('Circular Stepper export');
  visited.add(stepperNode.name);
  stepperNode = source.binding(44230,stepperNode.name);
}
if (stepperNode.type !== 'ClassDeclaration') throw Error('Stepper export is not a class');
const methods = {};
walk(stepperNode, node => {
  if (node.type === 'AssignmentExpression' && node.left.type === 'MemberExpression'
      && node.left.object.type === 'ThisExpression' && /FunctionExpression$/.test(node.right.type)) {
    methods[key(node.left.property)] = source.receipt(44230, node.right);
  }
});
for (const name of ['handleChange','parseInput','handleBlur','volumeUp','volumeDown','sliderOnMouseWheel']) {
  if (!methods[name]) throw Error(`Missing current Stepper method ${name}`);
}
if (!methods.handleChange.source.includes('this.props.allowLiveUpdate&&this.sendToParent(s)')) {
  throw Error('Current live draft emission changed');
}

// Exact matching for this mount's simple selectors. Other product/context
// rules stay in receipts but cannot win in the .key-config ancestor path.
const ancestor = {tag:'div', classes:['key-config']};
function matchesToken(token, node, state) {
  if (!/^(?:[a-z]+)?(?:\.[\w-]+|:(?:hover|active|focus-within))*$/.test(token)) return false;
  const tag = token.match(/^[a-z]+/)?.[0];
  if (tag && tag !== node.tag) return false;
  return [...token.matchAll(/\.([\w-]+)|:([\w-]+)/g)].every(match =>
    match[1] ? node.classes.includes(match[1]) : state.includes(match[2]));
}
function cascade(part, state = [], disabled = false) {
  const stepper = {tag:'div', classes:['stepper','custom-keymapping-stepper', ...(part === 'root' && disabled ? ['disabled'] : [])]};
  const nodes = [ancestor, stepper];
  if (part !== 'root') nodes.push(part === 'input' ? {tag:'input', classes:[]} :
    {tag:'div', classes:['icon','spinner',part,...(disabled ? ['disabled'] : [])]});
  const result = {};
  for (const rule of css.rules) for (const selector of rule.selector.split(',')) {
    const tokens = selector.trim().split(/\s+/);
    if (!matchesToken(tokens.at(-1), nodes.at(-1), state)) continue;
    let cursor = nodes.length - 2, matched = true;
    for (let i = tokens.length - 2; i >= 0; i--) {
      while (cursor >= 0 && !matchesToken(tokens[i], nodes[cursor], cursor === 1 ? state : [])) cursor--;
      if (cursor-- < 0) { matched = false; break; }
    }
    if (!matched) continue;
    if (rule.conditions.length) throw Error('Applicable conditional Stepper CSS needs review');
    const specificity = [0, (selector.match(/[.:][\w-]+/g) || []).length,
      tokens.filter(token => /^[a-z]/.test(token)).length];
    for (const [declaration_order, property] of rule.properties.entries()) {
      const priority = [Number(property.important), ...specificity, rule.offset, declaration_order];
      const old = result[property.property];
      const difference = old ? priority.findIndex((value, index) => value !== old.priority[index]) : -1;
      const wins = !old || difference >= 0 && priority[difference] > old.priority[difference];
      if (wins) result[property.property] = {value:property.value, selector,
        specificity, offset:rule.offset, important:property.important, priority};
    }
  }
  return Object.fromEntries(Object.entries(result).map(([name, {priority, ...entry}]) => [name,entry]));
}
const resolved = {
  root:cascade('root'), root_hover:cascade('root',['hover']),
  root_focus:cascade('root',['focus-within']), root_disabled:cascade('root',[],true),
  input:cascade('input'), up:cascade('up'), down:cascade('down'),
  up_disabled:cascade('up',[],true), up_hover:cascade('up',['hover']),
  up_active:cascade('up',['active']),
};
for (const [part, property, value] of [
  ['root','width','58px'], ['root','height','25px'], ['root','position','relative'],
  ['root','padding','5px 18px 5px 5px'], ['root','border','1px solid #5d5d5d'],
  ['root_hover','border','1px solid #44d62c'], ['root_focus','border','1px solid #5d5d5d'],
  ['root','transition','opacity .2s'], ['root_disabled','opacity','.3'],
  ['input','width','90%'], ['input','height','24px'], ['input','left','6px'],
  ['input','line-height','14px'], ['input','padding','0'],
  ['up','height','12px'], ['up','width','14px'], ['up','top','0'], ['down','bottom','0'],
  ['up_disabled','opacity','1'], ['up_disabled','pointer-events','none'],
  ['up_hover','background-color','#ffffff1a'], ['up_active','background-color','#0000001a'],
]) if (resolved[part][property]?.value !== value) throw Error(`Changed cascade: ${part} ${property}`);

const nativePath = 'crates/razer-widgets/src/stepper.rs';
const assets = [['up','dcb04520'],['down','349f755c']].map(([direction,fingerprint]) => {
  const original = `stepper_${direction}.${fingerprint}.svg`;
  const sourcePath = `.ref/devices/182/static/media/${original}`;
  const output = `assets/synapse/wired-argb-3871-stepper_${direction}.svg`;
  const bytes = fs.readFileSync(path.join(root,sourcePath));
  if (!resolved[direction]['background-image'].value.includes(original)
      || !bytes.equals(fs.readFileSync(path.join(root,output)))) throw Error('Current Stepper arrow differs');
  const embedded = fs.readFileSync(path.join(root,'assets/synapse/embedded.rs'),'utf8');
  const manifest = JSON.parse(fs.readFileSync(path.join(root,'assets/synapse/manifest.json'),'utf8'));
  if (!embedded.includes(`"${output.slice('assets/'.length)}"`)
      || !manifest.entries.some(entry => entry.output === output && entry.sha256 === hash(bytes))) {
    throw Error('Current Stepper arrow is not registered');
  }
  return {current_dashboard_css_reference:original, source:sourcePath, output, sha256:hash(bytes),
    method:'Exact shared hashed resource named by current Dashboard CSS; native bytes equal current 182 cached bytes, with embedded/manifest registration checked.'};
});
const evidence = {
  source_date:'2026-10-05', generator_sha256:hash(fs.readFileSync(__filename)),
  method:'Current manifest scope, Acorn AST receipts and static specificity/order cascade; no reference JavaScript or application execution.',
  manifest:{path:manifestPath, sha256:hash(fs.readFileSync(path.join(root, manifestPath)))},
  props, mount:source.receipt(82508,propsNode),
  stepper:source.receipt(44230,stepperNode),
  parent:source.receipt(82508,source.binding(82508,'k')),
  methods, css:cssReceipts, resolved, assets,
  geometry:{root_border_box:[58,25], containing_padding_box:[56,23], input:[50.4,24],
    input_left:6, spinner:[14,12], spinner_overlap:1, margin_bottom:10,
    root_padding_note:'All three children are absolute. Native renders the equivalent border-relative offsets without allocating unused root content padding.'},
  behavior:{
    live:'Accepted integer strings, including empty and minus-only drafts, are sent immediately without parse, step snap, or range clamp; parent changeTimes stores the argument unchanged.',
    native_event:'Opt-in live_update emits draft:Some(raw) with the previous committed finite value in value. Existing numeric consumers retain their committed-value contract. Commit/step emits draft:None.',
    commit:'Empty or minus-only input parses as NaN -> 0 -> min 1; integer step snap precedes clamp. Enter and Escape blur and commit.',
    increment:'For a typed nondecimal string, source + concatenates before parseInput (e.g. typed 2 -> 21); decrement coerces numerically.',
    wheel:'Source props-value >=/<= guards coerce raw strings, unlike spinner strict === bounds; live native wheel guards preserve this distinction.',
    sync:'sync_value is the external non-emitting numeric synchronization API; unchanged numeric values preserve active drafts. set_value is private and emits committed events.',
    visibility:'Custom spinner specificity overrides hidden/disabled opacity to 1; bound pointer-events remains none. Focus alone does not override the custom gray border; hover does.',
    motion:'Final root transition shorthand is opacity .2s with default ease, replacing the generic border transition. Native custom root fades enabled/disabled opacity over 200ms. Border changes immediately; spinner opacity is always 1, so its declared opacity transition has no varying endpoint.',
    limit:'Source maxLength is 2, with +1 when current e < 0. Existing native validation allows the additional character whenever the draft starts with minus; this narrow intermediate-edit difference is retained.',
    save:'Current parent changeTimes requests enableSave without validating repeatCount. Native typed persistence may reject incomplete drafts; this is a storage adaptation, not evidence of a source validation rule.',
  },
  native:{path:nativePath,sha256:hash(fs.readFileSync(path.join(root,nativePath)))},
  limitations:['Static cascade receipts do not certify rendered pixel equality.'],
};
const output = path.join(root,'docs/re/shortcuts-stepper-current-evidence.json');
const text = JSON.stringify(evidence,null,2)+'\n';
if (process.argv.includes('--check')) {
  if (fs.readFileSync(output,'utf8') !== text) throw Error('Stepper evidence is stale');
} else fs.writeFileSync(output,text);
console.log('Current Macro repeat Stepper: 58x25, raw live drafts, final CSS specificity verified.');
