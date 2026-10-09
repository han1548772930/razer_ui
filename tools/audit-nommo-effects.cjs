// Current Nommo lighting effect receipts. Reference JavaScript is parsed, never executed.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const hash = s => crypto.createHash('sha256').update(s).digest('hex');
function walk(node, visit) {
  if (!node?.type) return;
  visit(node);
  for (const v of Object.values(node)) {
    if (Array.isArray(v)) v.forEach(n => walk(n, visit));
    else if (v?.type) walk(v, visit);
  }
}
const args = process.argv.slice(2);
const check = args.includes('--check');
for(const arg of args)if(arg.startsWith('-')&&arg!=='--check')throw Error(`Unknown option ${arg}`);
const declarations = args.filter(arg=>arg!=='--check');
if(check && declarations.length)throw Error('--check cannot be combined with declaration queries');
const products = [];
for (const pid of [1303, 1304]) {
  const directory = `.ref/devices/${pid}`;
  const manifestPath = `${directory}/asset-manifest.json`;
  const manifestText = fs.readFileSync(path.join(root, manifestPath), 'utf8');
  const manifest = JSON.parse(manifestText);
  const file = `${directory}/${manifest.files['main.js'].replace(/^\.\//, '')}`;
  const cssFile = `${directory}/${manifest.files['main.css'].replace(/^\.\//, '')}`;
  const source = fs.readFileSync(path.join(root, file), 'utf8');
  const css = fs.readFileSync(path.join(root, cssFile), 'utf8');
  const names = new Set(declarations.length ? declarations : pid === 1303
    ? ['wm','ym','bm','Kp','xp','Mm','Um','iP','DP','xP','rp','Mp','Wp','CP','cP','sP','um','Cm','Lp','Pp']
    : ['kM','zM','wM','qm','Qm','fM','HM','Tp','Mp','Qp','Sm','fm','Km','pp','Pp','Ap','mM','pM','Um','hm']);
  const receipts = [], modules = new Map(), directionMaps = [];
  walk(acorn.parse(source, {ecmaVersion: 'latest'}), node => {
    if(node.type==='ObjectExpression' && node.properties.some(p=>(p.key?.name??p.key?.value)==='CWCCW') && node.properties.every(p=>p.value?.type==='Literal'))
      directionMaps.push({offset:node.start,end:node.end,source:source.slice(node.start,node.end),value:Object.fromEntries(node.properties.map(p=>[p.key.name??p.key.value,p.value.value]))});
    if (node.type === 'Property' && Number.isInteger(node.key?.value) && /Function/.test(node.value.type)) modules.set(node.key.value,node.value);
    if ((node.type === 'VariableDeclarator' || node.type === 'ClassDeclaration' || node.type === 'FunctionDeclaration') && names.has(node.id?.name)) {
      const value = node.type === 'VariableDeclarator' ? node.init : node;
      if (!value || value.type === 'Literal' || (!declarations.length && (value.start < (pid===1303?4324000:4341000) || value.end > (pid===1303?4419700:4436007)))) return;
      receipts.push({name: node.id.name, offset: value.start, end: value.end, source: source.slice(value.start, value.end)});
    }
  });
  if(!declarations.length)for(const name of names) {
    const matches=receipts.filter(receipt=>receipt.name===name);
    if(matches.length!==1)throw Error(`Expected exactly one mounted declaration ${pid}:${name}, found ${matches.length}`);
  }
  function moduleValue(id, exported) {
    const module = modules.get(id), defs = new Map(), exports = new Map();
    if (!module) throw Error(`Missing source module ${id}`);
    for (const statement of module.body.body) {
      if (statement.type === 'VariableDeclaration') for (const decl of statement.declarations) defs.set(decl.id.name,decl.init);
      if (statement.type === 'ExpressionStatement') walk(statement, n => {
        if (n.type === 'CallExpression' && n.callee.type === 'MemberExpression' && n.callee.property.name === 'd' && n.arguments[1]?.type === 'ObjectExpression')
          for (const p of n.arguments[1].properties) exports.set(p.key.name ?? p.key.value,p.value.body);
      });
    }
    function literal(n) {
      if (n.type === 'Literal') return n.value;
      if (n.type === 'Identifier') return literal(defs.get(n.name));
      if (n.type === 'UnaryExpression' && n.operator === '!') return !literal(n.argument);
      if (n.type === 'UnaryExpression' && n.operator === '-') return -literal(n.argument);
      if (n.type === 'ArrayExpression') return n.elements.map(literal);
      if (n.type === 'ObjectExpression') return Object.fromEntries(n.properties.map(p=>[p.computed?literal(p.key):p.key.name??p.key.value,literal(p.value)]));
      if (n.type === 'MemberExpression') {
        const field = n.computed?literal(n.property):n.property.name;
        const binding = n.object.type === 'Identifier' ? defs.get(n.object.name) : null;
        if (binding?.type === 'CallExpression' && binding.arguments[0]?.type === 'Literal') return moduleValue(binding.arguments[0].value,field).value;
        return literal(n.object)[field];
      }
      throw Error(`Nonliteral ${id}: ${source.slice(n.start,n.end).slice(0,100)}`);
    }
    const node = exports.get(exported); if(!node)throw Error(`Missing export ${id}.${exported}`);
    const resolved = node.type === 'Identifier' ? defs.get(node.name) : node;
    return {module:id, export:exported, offset:resolved.start,end:resolved.end,source:source.slice(resolved.start,resolved.end),value:literal(node)};
  }
  const defaults = moduleValue(3254,'U2A');
  const configuration = Object.fromEntries(['DeviceInfo','QUICK_EFFECTS','DEFAULTPROFILE'].map(key=>[key,moduleValue(8193,key)]));
  if(directionMaps.length!==1)throw Error(`Ambiguous default direction map ${pid}`);
  const waveDirection=directionMaps[0];
  const copy = Object.fromEntries(['tQN','Zsw','zRV','uRW','TC$','E6M','eYT','wFO','WqG','arT','o$r','XE0','G2f','WLd'].map(key=>[key,moduleValue(4693,key)]));
  const palette = receipts.find(r=>r.name===(pid===1303?'sP':'Ap'));
  if(!declarations.length && !palette)throw Error(`Missing palette ${pid}`);
  if (palette) {
    let node=acorn.parseExpressionAt(palette.source,0,{ecmaVersion:'latest'});
    if(node.type==='SequenceExpression')node=node.expressions.at(-1);
    if(node.type!=='ArrayExpression')throw Error(`Missing palette array ${pid}`);
    palette.value=node.elements.map(n=>{if(n.type!=='Literal')throw Error('Nonliteral palette');return n.value});
    const rust=fs.readFileSync(path.join(root,'crates/razer-pages/src/features/lighting_color.rs'),'utf8');
    const values=[...rust.match(/const PRESETS:[\s\S]*?= \[([\s\S]*?)\];/)[1].matchAll(/0x([0-9a-f]{6})/g)].map(m=>'#'+m[1]);
    if(JSON.stringify(palette.value)!==JSON.stringify([...values,'no-color']))throw Error(`Palette mismatch ${pid}`);
  }
  const cssRules = parseCSS(css).filter(rule => /effect|chroma|color|radio|modes-tab|modes-area|twoway-lighting|toggle-btn|dir-cw|dir-ccw|dir-in|dir-out|stepper|\.icon.spinner|\.body-text/.test(rule.selector));
  products.push({product_id:pid, manifest:{path:manifestPath,sha256:hash(manifestText)}, javascript:{path:file,sha256:hash(source)}, css:{path:cssFile,sha256:hash(css)}, defaults, configuration, wave_direction:waveDirection, copy, declarations:receipts, css_rules:cssRules});
}
if (declarations.length) console.log(JSON.stringify(products.map(p=>({product_id:p.product_id,declarations:p.declarations})),null,2));
else {
  const artifacts = [
    ['docs/re/nommo-effects-source.json', {method:'Acorn AST and static CSS only; no reference JavaScript evaluation',offset_unit:'UTF-16 code units in the decoded JavaScript/CSS strings, not byte offsets',products}],
    ['crates/razer-pages/src/features/audio_nommo_effects_data.json',products.map(p=>({product_id:p.product_id, effects:p.configuration.QUICK_EFFECTS.value, wave_default_direction:p.wave_direction.value[p.configuration.DeviceInfo.value.WAVE_DIRECTION], defaults:Object.fromEntries(p.configuration.QUICK_EFFECTS.value.map(e=>[e.id,p.defaults.value[e.id]??{}]))}))]
  ];
  for(const [file,value] of artifacts) {
    const expected=JSON.stringify(value,null,2)+'\n', target=path.join(root,file);
    if(check) {
      if(fs.readFileSync(target,'utf8')!==expected)throw Error(`Stale current source artifact: ${file}`);
    } else fs.writeFileSync(target,expected);
  }
  console.log(`${check?'Validated':'Recorded'} current Nommo effect declarations, palettes, CSS receipts and generated defaults for 1303/1304.`);
}
