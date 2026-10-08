// Current existing-adapter power pages. Static AST/CSS receipts, no vendor execution.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {walk, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), read = p => fs.readFileSync(path.join(root, p), 'utf8');
const assert = (ok, message) => { if (!ok) throw Error(message); };
const products = [], sourceInputs = {};
for (const pid of [182, 777]) {
  const base = `.ref/devices/${pid}/`, manifestPath = base + 'asset-manifest.json';
  const manifest = JSON.parse(read(manifestPath));
  const declared = name => {
    const file = manifest.files[name];
    assert(file?.startsWith('./static/') && !file.includes('..'), 'Undeclared source');
    return base + file.slice(2);
  };
  const jsPath = declared('main.js'), cssPath = declared('main.css');
  for (const file of [manifestPath, jsPath, cssPath]) sourceInputs[file] = hash(fs.readFileSync(path.join(root, file)));
  const js = read(jsPath), ast = acorn.parse(js, {ecmaVersion:'latest'}), nodes = [];
  const names = pid === 182 ? ['MM', 'mM', 'DM', 'CM', 'PM', 'pM', 'CD'] : ['KG', 'kG', 'zG', 'wG', 'SM', 'AM'];
  walk(ast, node => {
    let symbol, expression;
    if (['ClassDeclaration','FunctionDeclaration'].includes(node.type) && names.includes(node.id?.name)) {
      symbol = node.id.name; expression = node;
    } else if (node.type === 'VariableDeclarator' && names.includes(node.id.name)) {
      symbol = node.id.name; expression = node.init;
    }
    if (expression && expression.start > 4000000)
      nodes.push({symbol, offset:expression.start, end:expression.end, source:js.slice(expression.start,expression.end)});
    // Product 182 imports module 130's A export; resolve T and I inside that
    // actual module instead of matching an unrelated slider by its class name.
    if (pid === 182 && node.type === 'Property' && node.key.value === 130) {
      for (const child of node.value.body.body) {
        if (child.type === 'ClassDeclaration' && child.id.name === 'T')
          nodes.push({symbol:'130.T', offset:child.start, end:child.end, source:js.slice(child.start,child.end)});
        if (child.type === 'VariableDeclaration') for (const binding of child.declarations) {
          if (binding.id.name === 'I') nodes.push({symbol:'130.I', offset:binding.init.start,
            end:binding.init.end, source:js.slice(binding.init.start,binding.init.end)});
        }
      }
    }
  });
  for (const name of names) assert(nodes.filter(node => node.symbol === name).length === 1, 'Ambiguous power component: ' + pid + '/' + name);
  const source = name => nodes.find(node => node.symbol === name)?.source || '';
  const chain = pid === 182 ? source('MM') + source('mM') + source('DM') + source('PM') : source('KG') + source('kG') + source('zG');
  if (pid === 182) {
    assert(source('CD') === 'a(130)' && source('130.I').includes('(T)'), 'Mouse range export changed');
    assert(chain.includes('(DM,{})') && chain.includes('(PM,{})') && chain.includes('(CM)') && chain.includes('(pM)'), 'Mouse power mount changed');
    assert(source('CM').includes('min:1,max:15,step:1') && source('pM').includes('min:5,max:100,step:5'), 'Mouse power ranges changed');
    assert(source('pM').includes('<=1e3') && source('pM').includes('className:"warning-text"'), 'Low-power disabled condition changed');
  } else {
    assert(source('KG') === 'kG' && source('kG').includes('(zG,{min:5,minTag:"5"})') && source('zG').endsWith('(wG)'), 'Headset power mount changed');
    assert(source('SM') === 'OM(AM)' && source('wG').includes('(SM,{min:a,max:_,step:i'), 'Headset range mount changed');
    assert(js.includes('wG.defaultProps={min:15,minTag:"15",max:60,maxTag:"60",step:1}'), 'Headset range defaults changed');
  }
  const range = source(pid === 182 ? '130.T' : 'AM');
  assert(range.includes('this.mouseIsDown?') && range.includes('this.props.callOnChangeOnEveryStep'), 'Drag preview contract changed');
  assert(range.includes('this.props.changeValue(this.state.value,') && range.includes('this.getPercent()*(E-16)-e/2+8'), 'Release/tip geometry changed');
  const css = parseCSS(read(cssPath)), rules = [];
  const expected = {
    '.slider-container': {height:'64px', opacity:'.3', 'pointer-events':'none'},
    '.slider': {bottom:'25px', height:'6px'},
    '.slider::-webkit-slider-thumb': {height:'16px', width:'16px', 'border-radius':'8px'},
    '.slider-container .track': {background:'#44d62c4d', bottom:'25px', height:'6px'},
    '.slider-tip': {bottom:'42px', padding:'4px 8px', 'line-height':'14px'},
    '.slider-container .foot': {bottom:'-2px'},
    '.h1-body': {'margin-bottom':'10px'},
    '.warning-text': {'margin-top':'15px'},
  };
  for (const [selector, values] of Object.entries(expected)) {
    const rule = css.find(rule => rule.selector === selector && Object.entries(values).every(([key,value]) =>
      rule.properties.some(property => property.property === key && property.value === value)));
    assert(rule, 'Different power CSS: ' + pid + '/' + selector);
    rules.push({selector, offset:rule.offset, declarations:rule.declarations});
  }
  products.push({product_id:pid, path:jsPath, nodes, rules});
}
const pages = read('src/features/device_pages.rs'), workspace = read('src/features/workspace.rs');
const powerPage = pages.slice(pages.indexOf('pub(super) fn power_page'), pages.indexOf('pub(super) fn lighting_page'));
assert((powerPage.match(/self\.power_range\(/g) || []).length === 2 && !powerPage.includes('self.source_range('), 'Power page still uses the generic slider');
assert((powerPage.match(/\.gap_0\(\)/g) || []).length === 2 && powerPage.includes('.mt(surface::css(15.))'), 'Power margins differ');
const powerRange = pages.slice(pages.indexOf('fn power_range('), pages.indexOf('pub(super) fn calibration_page'));
assert(powerRange.includes('SourceSlider::new(state, progress)') && powerRange.includes('.bottom(surface::css(-2.))')
  && powerRange.includes('.tip(Some(format!("{value:.0}")))') && !powerRange.includes('.mb('), 'Power slider geometry differs');
assert(workspace.includes('SliderEvent::Change(_) if power_range') && workspace.includes('SliderEvent::Release(value) if power_range'), 'Power preview commits before release');
const evidence = {
  schema_version:1, source_inputs:sourceInputs, products,
  changes:['Power sliders reuse the source drawing layer, with inset thumb/tip geometry and bottom -2px endpoint tags.',
    'Power panels use the description margin alone, with no extra gap or invented slider bottom margin.',
    'Low-power warning restores the 15px source margin.',
    'Idle/low-power drag changes preview in retained slider state; release updates the local draft once.'],
  remaining:['Keyboard parity, interruption by external observations, actual focus and pixel geometry remain unverified.',
    'Device write-back is deferred; local edits do not claim a successful device save.',
    'This is a power-control review, not complete product or page acceptance.'],
  runtime_validation:'not_run'
};
const serialized = JSON.stringify(evidence,null,2)+'\n', output = 'docs/re/power-range-current-evidence.json';
if (process.argv.includes('--write')) fs.writeFileSync(path.join(root,output),serialized);
else assert(read(output).replace(/\r\n/g,'\n') === serialized, 'Power receipts changed; inspect before --write');
console.log('Power controls: two current manifest mount chains, slider CSS, local preview/release policy statically checked; runtime not run.');
