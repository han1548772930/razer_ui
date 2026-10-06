// Resolve current numeric-editor CSS as data. Never evaluate vendor JavaScript.
const fs = require('node:fs'), path = require('node:path'), crypto = require('node:crypto');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const hash = file => crypto.createHash('sha256').update(fs.readFileSync(path.join(root, file))).digest('hex');
const assert = (condition, message) => { if (!condition) throw Error(message); };
const args = process.argv.slice(2);
assert(args.every(arg => arg === '--check'), 'Only --check is supported');

function matches(token, node) {
  if (!/^(?:[a-z]+)?(?:\.[\w-]+|:(?:hover|focus-within))*$/.test(token)) return false;
  if (token.match(/^[a-z]+/)?.[0] && token.match(/^[a-z]+/)[0] !== node.tag) return false;
  return [...token.matchAll(/\.([\w-]+)|:([\w-]+)/g)].every(match =>
    match[1] ? node.classes.includes(match[1]) : node.states.includes(match[2]));
}
function cascade(rules, state, bound, modesArea) {
  const nodes = [
    ...(modesArea ? [{tag:'div', classes:['modes-area'], states:[]}] : []),
    {tag:'div', classes:['stepper'], states:state ? [state] : []},
    {tag:'div', classes:['icon','spinner','up', ...(bound ? ['disabled'] : [])], states:[]},
  ];
  const values = {};
  for (const rule of rules) for (const selector of rule.selector.split(',')) {
    const tokens = selector.trim().split(/\s+/);
    if (!matches(tokens.at(-1), nodes.at(-1))) continue;
    let cursor = nodes.length - 2, applies = true;
    for (let i = tokens.length - 2; i >= 0; i--) {
      while (cursor >= 0 && !matches(tokens[i], nodes[cursor])) cursor--;
      if (cursor-- < 0) { applies = false; break; }
    }
    if (!applies) continue;
    assert(!rule.conditions.length, `Conditional spinner rule needs review: ${selector}`);
    const specificity = [(selector.match(/[.:][\w-]+/g) || []).length,
      tokens.filter(token => /^[a-z]/.test(token)).length];
    for (const [index, property] of rule.properties.entries()) {
      const priority = [Number(property.important), ...specificity, rule.offset, index];
      const old = values[property.property];
      const difference = old ? priority.findIndex((value, index) => value !== old.priority[index]) : -1;
      if (!old || difference >= 0 && priority[difference] > old.priority[difference]) {
        values[property.property] = {value:property.value, selector, offset:rule.offset, priority};
      }
    }
  }
  return Object.fromEntries(Object.entries(values).map(([key, {priority, ...value}]) => [key, value]));
}
const products = [];
for (const pid of [1303,1304,3592,3594,3595,3596]) {
  const manifestPath = `.ref/devices/${pid}/asset-manifest.json`;
  const manifest = JSON.parse(read(manifestPath));
  const cssPath = `.ref/devices/${pid}/${manifest.files['main.css'].replace(/^\.\//, '')}`;
  const rules = parseCSS(read(cssPath)).filter(rule => /\.(?:spinner|stepper)(?![\w-])/.test(rule.selector));
  const nommo = pid === 1303 || pid === 1304;
  const states = Object.fromEntries(['idle','hover','focus-within'].flatMap(state => [false,true].map(bound =>
    [`${state}${bound ? '_bound' : ''}`, cascade(rules, state === 'idle' ? '' : state, bound, nommo)])));
  for (const [state, resolved] of Object.entries(states)) {
    const idle = state.startsWith('idle'), bound = state.endsWith('_bound');
    const opacity = nommo ? (idle ? (bound ? '.3' : '0') : '1') : (bound ? '.3' : '1');
    assert(resolved.opacity?.value === opacity, `${pid} ${state}: opacity changed`);
    assert(resolved.visibility?.value === (nommo && idle ? 'hidden' : 'visible'), `${pid} ${state}: visibility changed`);
    assert(resolved.transition?.value === 'visibility 0s,opacity .1s linear', `${pid} ${state}: transition changed`);
    if (bound) assert(resolved['pointer-events']?.value === 'none', `${pid} ${state}: bound input changed`);
  }
  products.push({product_id:pid, manifest:{path:manifestPath,sha256:hash(manifestPath)},
    css:{path:cssPath,sha256:hash(cssPath)},rules,states});
}
const nativePath = 'src/ui/stepper.rs', native = read(nativePath);
for (const marker of ['reveal_spinners_on_hover: false', 'self.hovered || self.focused',
  'Duration::from_millis(100)', 'Easing::Linear', 'button.invisible()',
  'else if at_limit && !self.custom_keymapping', '.on_hover(cx.listener(']) {
  assert(native.includes(marker), `Missing native spinner policy: ${marker}`);
}
const nommo = 'src/features/audio_nommo_effects.rs';
assert(read(nommo).includes('.reveal_spinners_on_hover()'), 'Nommo did not opt into source visibility');
assert(!read('src/features/source_controls.rs').includes('.reveal_spinners_on_hover()'), 'Camera lost always-visible policy');
const result = {method:'Static CSS cascade for the actual Nommo modes-area and camera stepper rules; no vendor execution',
  offset_unit:'UTF-16 code units', generator_sha256:hash('tools/audit-stepper-visibility.cjs'),products,
  native:{path:nativePath,sha256:hash(nativePath)},
  limitation:'Runtime motion, focus and hit testing have not been exercised. Other callers retain their existing explicit policy.'};
const target = 'docs/re/stepper-visibility-current-evidence.json', serialized = JSON.stringify(result,null,2)+'\n';
if (args.includes('--check')) assert(read(target) === serialized, `Stale ${target}`);
else fs.writeFileSync(path.join(root,target),serialized);
console.log('Validated Nommo hover/focus and four camera always-visible spinner cascades (36 states).');
