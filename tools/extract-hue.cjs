// Current Hue literals and source receipts. Acorn parses data; no vendor code runs.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const hash = p => crypto.createHash('sha256').update(p).digest('hex');
const sourcePath = '.ref/devices/769/static/js/main.ad1113f8.js', source = read(sourcePath);
if (hash(source) !== '3d1660626594af99c54c622b2e1caf3190e172e8703392a65286e81b9a3fff9d') throw Error('Re-audit changed Hue source');
const ast = acorn.parse(source, {ecmaVersion:'latest'});
const declarations = [], modules = [];
function walk(n, fn) {
  if (!n?.type || fn(n) === false) return;
  for (const v of Object.values(n)) {
    if (Array.isArray(v)) v.forEach(c => walk(c, fn));
    else if (v?.type) walk(v, fn);
  }
}
walk(ast, n => {
  if (n.type === 'VariableDeclarator') declarations.push(n);
  if (n.type === 'Property' && Number.isInteger(n.key.value)) modules.push(n);
});
const binding = (name, start, end) => {
  const matches = declarations.filter(n => n.id.name === name && n.start >= start && n.end <= end);
  if (matches.length !== 1) throw Error(`Ambiguous ${name}`);
  return matches[0];
};
function literal(n, aliases = {}) {
  if (n.type === 'Literal') return n.value;
  if (n.type === 'Identifier' && Object.hasOwn(aliases, n.name)) return aliases[n.name];
  if (n.type === 'MemberExpression') {
    const object=literal(n.object,aliases), field=n.computed?literal(n.property,aliases):n.property.name;
    if (Object.hasOwn(object,field)) return object[field];
  }
  if (n.type === 'UnaryExpression' && n.operator === '!') return !literal(n.argument, aliases);
  if (n.type === 'ArrayExpression') return n.elements.map(v => literal(v, aliases));
  if (n.type === 'ObjectExpression') return Object.fromEntries(n.properties.map(p => [p.computed ? literal(p.key, aliases) : p.key.name ?? p.key.value, literal(p.value, aliases)]));
  throw Error('Nonliteral: ' + source.slice(n.start,n.end).slice(0,80));
}
const receipt = n => ({path:sourcePath, sha256:hash(source), offset:n.start, end:n.end, source:source.slice(n.start,n.end)});
const languageAliases = Object.fromEntries(['$o','ei','Ei'].map(name => [name, literal(binding(name,6494900,6495021).init)]));
const dictionary = binding('_i',6494900,6539770);
const translations = literal(dictionary.init,languageAliases);
if (Object.keys(translations).length !== 10 || !translations.en || !translations['zh-CN']) throw Error('Changed Hue locales');
// Verify the fallback exported by module 37, avoiding a stale minified name.
const languageModule = modules.find(n => n.key.value === 37);
if (!languageModule) throw Error('Missing language module');
let fallbackSymbol;
walk(languageModule.value, n => {
  if (n.type === 'Property' && n.key.name === 'Nm' && n.value.type === 'ArrowFunctionExpression') fallbackSymbol = n.value.body.name;
});
const fallback = literal(binding(fallbackSymbol,languageModule.start,languageModule.end).init);
if (fallback !== 'en') throw Error('Changed Hue fallback');
const names = ['s_','I_','O_','A_','S_','R_','N_','l_','C_','d_','D_','c_'];
const constants = Object.fromEntries(names.map(name => [name,literal(binding(name,6309900,6310200).init)]));
const hue = literal(binding('wp',6824100,6824500).init);
const integration = literal(binding('kp',6824100,6824700).init,constants);
const brightness = literal(binding('tR',6710200,6710600).init).brightness;
function exportedNode(id,key) {
  const module=modules.find(m=>m.key.value===id);
  const exports=[];
  walk(module.value,n=>{if(n.type==='Property'&&n.key.name===key&&n.value.type==='ArrowFunctionExpression')exports.push(n.value.body);});
  if(exports.length!==1||exports[0].type!=='Identifier')throw Error(`Ambiguous export ${id}/${key}`);
  return binding(exports[0].name,module.start,module.end).init;
}
const labels=['Rb7','ncy','xkP','k9y','DU0','MJD','a1g','WpG','KFn','zrT','tQN','Zsw','arT','XE0','o$r','WLd','_ec'];
const commonLabels=Object.fromEntries(labels.map(key=>[key,literal(exportedNode(4693,key))]));
const effectEnum=literal(exportedNode(3254,'AoV'));
const effectsNode=binding('x',6260900,6261400);
const effects=literal(effectsNode.init,{w:commonLabels,X:{AoV:effectEnum}});
const effectDefaultsNode=exportedNode(3254,'U2A');
const defaults=literal(effectDefaultsNode,{HE:effectEnum});
const lightingNode=binding('TR',6716000,6716600);
const lightingDefaults=literal(lightingNode.init);
const paletteNode=binding('dn',6559100,6559900);
const palette=literal(paletteNode.init);
const sharedPalette=read('src/features/lighting_color.rs').match(/const PRESETS: \[u32; 40\] = \[([\s\S]*?)\];/);
const sharedColors=[...sharedPalette[1].matchAll(/0x([a-f0-9]{6})/gi)].map(m=>'#'+m[1].toLowerCase());
if(JSON.stringify(palette)!==JSON.stringify([...sharedColors,'no-color']))throw Error('Hue palette differs from the shared native picker');
const effectComponents=[];
walk(ast,n=>{
  if(n.type==='ClassDeclaration'&&['Fn','RT','qT','nr'].includes(n.id.name)&&n.start>6500000&&n.end<6641000)effectComponents.push(receipt(n));
});
if(effectComponents.length!==4)throw Error('Changed Hue effect component bindings');
const bridgeIcons=[];
for(const [name,offset,end] of [['refresh',6676253,6677381],['remove',6675099,6676200]]) {
  const paths=[],groups=[];
  walk(ast,n=>{
    if(n.start<offset||n.end>end)return;
    if(n.type==='CallExpression'&&n.callee.property?.name==='createElement'&&n.arguments[1]?.type==='ObjectExpression') {
      if(n.arguments[0]?.value==='path')paths.push(literal(n.arguments[1]));
      if(n.arguments[0]?.value==='g')groups.push(literal(n.arguments[1]));
    }
  });
  if(paths.length!==1)throw Error('Ambiguous Hue icon');
  bridgeIcons.push({name,paths,groups,offset,end});
}
const archetypeNode = binding('L_',6310000,6360000);
const archetypes = literal(archetypeNode.init);
const cssPath = '.ref/devices/769/static/css/main.341edb83.css', css = read(cssPath);
const cssRules = css.split('}').filter(r => /\.(Home_|BridgeLogo_|Bridge_|Brightness_|Devices_|screen|install-chroma-img|chroma-studio-btn|chroma-sync|quickeffect-text|modes-area|twoway-lighting|effects-area)/.test(r)).map(r => r+'}');
const requests = [...new Set(cssRules.flatMap(r => [...r.matchAll(/url\(\.\.\/\.\.\/(static\/media\/[^)]+)\)/g)].map(m => m[1])))];
const manifestPath = '.ref/devices/769/asset-manifest.json';
const manifest = JSON.parse(read(manifestPath));
const assets = requests.map(request => {
  if (!Object.values(manifest.files).some(p => p.replace(/^\.\//,'') === request)) throw Error('Undeclared Hue asset: '+request);
  const name = 'hue-' + path.basename(request).replace(/\.[a-f0-9]{8}\.(svg|avif)$/,(all,ext)=>'.'+(ext==='avif'?'png':ext));
  return {source:'.ref/devices/769/'+request, url:'https://apps.razer.com/synapse/products/769/ui/'+request,
    asset:'synapse/'+name, output:'assets/synapse/'+name, context:cssPath, context_sha256:hash(css)};
});
const evidence = JSON.parse(read('docs/re/pending-product-769-source.json'));
const components = evidence.pages[0].components.filter(c => c.offset >= 6540541 && c.offset <= 6545643 || c.offset >= 6677381 && c.offset <= 6687100);
const mounted = source.slice(6689360,6689820);
if (!mounted.includes('E===w._$r?') || !mounted.includes('_?(0,q.jsx)(pA,{}):(0,q.jsx)(Fi,{})')) throw Error('Changed actual Hue root');
const data = {product_id:769, fallback, translations, constants, common_labels:commonLabels,
  initial:{hue,integration,brightness}, lighting:{effects,defaults,initial:lightingDefaults}, archetypes, assets, bridge_icons:bridgeIcons};
function emit(file,value) {
  const text = JSON.stringify(value,null,2)+'\n';
  if (process.argv.includes('--check')) {
    if (read(file) !== text) throw Error('Stale Hue output: '+file);
  } else fs.writeFileSync(path.join(root,file),text);
}
emit('src/features/hue_data.json',data);
emit('docs/re/hue-current-evidence.json',{
  method:'Acorn literals and mounted source receipts; downloaded JavaScript is never evaluated',
  generator_sha256:hash(fs.readFileSync(__filename)), source:{path:sourcePath,sha256:hash(source)},
  dictionaries:receipt(dictionary), archetypes:receipt(archetypeNode),
  constants:names.map(name=>receipt(binding(name,6309900,6310200))),
  defaults:['wp','kp'].map(name=>receipt(binding(name,6824100,6824700))),
  brightness:receipt(binding('tR',6710200,6710600)),
  lighting:[receipt(effectsNode),receipt(effectDefaultsNode),receipt(lightingNode)],
  palette:receipt(paletteNode), effect_components:effectComponents,
  common_labels:labels.map(key=>({export:key,...receipt(exportedNode(4693,key))})),
  mounted_root:{offset:6689360,end:6689820,source:mounted},
  css:{path:cssPath,sha256:hash(css),rules:cssRules},
  manifest:{path:manifestPath,sha256:hash(read(manifestPath))}, components,
});
console.log(`Resolved Hue: ${Object.keys(translations).length} locales, ${archetypes.length} archetypes, ${assets.length} assets.`);
