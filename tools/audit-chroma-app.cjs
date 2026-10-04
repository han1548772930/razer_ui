// Static parsing only. Never evaluates a production webpack factory.
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const acorn = require('./node_modules/acorn');
const root = path.resolve(__dirname, '..');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const hash = s => crypto.createHash('sha256').update(s).digest('hex');
const base = '.ref/applications/chroma-app/dashboard';
const files = new Map(), modules = new Map();
function walk(node, visit) {
  if (!node || typeof node !== 'object') return;
  visit(node);
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) value.forEach(v => walk(v, visit));
    else if (value && typeof value === 'object') walk(value, visit);
  }
}
const key = n => n.name ?? n.value;
function parse(p) {
  if (files.has(p)) return files.get(p);
  const source = read(p), ast = acorn.parse(source, {ecmaVersion: 'latest'});
  const file = {path: p, source, ast}; files.set(p, file);
  walk(ast, n => {
    if (n.type !== 'Property' || typeof n.key.value !== 'number' || !n.value.body?.body) return;
    const module = {file, node: n, bindings: new Map(), exports: new Map()};
    for (const statement of n.value.body.body) {
      if (statement.type === 'VariableDeclaration') for (const d of statement.declarations)
        if (d.id.type === 'Identifier') module.bindings.set(d.id.name, d.init);
      if (statement.type === 'ClassDeclaration' || statement.type === 'FunctionDeclaration')
        module.bindings.set(statement.id.name, statement);
    }
    walk(n.value.body, x => {
      if (x.type === 'CallExpression' && x.callee.type === 'MemberExpression'
          && key(x.callee.property) === 'd' && x.arguments[1]?.type === 'ObjectExpression')
        for (const p of x.arguments[1].properties) {
          if (p.type === 'Property' && p.value.type === 'ArrowFunctionExpression')
            module.exports.set(key(p.key), p.value.body);
        }
    });
    modules.set(n.key.value, module);
  });
  return file;
}
for (const name of fs.readdirSync(path.join(root, base, 'static/js')).sort())
  if (name.endsWith('.js') && !name.startsWith('rzHardware')) parse(`${base}/static/js/${name}`);
function evaluate(module, node, seen = new Set()) {
  if (!node) return undefined;
  if (node.type === 'Literal') return node.value;
  if (node.type === 'Identifier') {
    if (seen.has(node.name)) return undefined;
    return evaluate(module, module.bindings.get(node.name), new Set([...seen, node.name]));
  }
  if (node.type === 'UnaryExpression') {
    const v = evaluate(module, node.argument, seen);
    return node.operator === '!' ? !v : node.operator === '-' ? -v : undefined;
  }
  if (node.type === 'MemberExpression' && node.object.type === 'Identifier') {
    const binding = module.bindings.get(node.object.name);
    if (binding?.type === 'CallExpression' && typeof binding.arguments[0]?.value === 'number') {
      const other = modules.get(binding.arguments[0].value);
      if (other) return evaluate(other, other.exports.get(key(node.property)));
    }
  }
  if (node.type === 'MemberExpression') {
    const object = evaluate(module, node.object, seen);
    if (object && typeof object === 'object') return object[key(node.property)];
  }
  if (node.type === 'LogicalExpression') {
    const left=evaluate(module,node.left,seen);
    if(node.operator==='||')return left||evaluate(module,node.right,seen);
    if(node.operator==='??')return left??evaluate(module,node.right,seen);
  }
  if (node.type === 'ArrayExpression') return node.elements.map(x => evaluate(module, x, seen));
  if (node.type === 'ObjectExpression') return Object.fromEntries(node.properties
    .filter(p => p.type === 'Property').map(p => [key(p.key), evaluate(module, p.value, seen)]));
  return undefined;
}
function value(id, name) {
  const m = modules.get(id);
  if (!m) throw Error(`Missing module ${id}`);
  return evaluate(m, m.exports.get(name) || m.bindings.get(name));
}
function receipt(file, node) {
  return {path: file.path, offset: node.start, end: node.end,
    file_sha256: hash(file.source), source_sha256: hash(file.source.slice(node.start, node.end)),
    source: file.source.slice(node.start, node.end)};
}
function component(id, name) {
  const m = modules.get(id), node = m?.bindings.get(name);
  if (!node) throw Error(`Missing Chroma component ${id}/${name}`);
  return {module_id: id, symbol: name, ...receipt(m.file, node)};
}
const contracts = [component(23322, 'Ks'), component(23322, 'Fs'),
  component(62296, 'Wn'), component(62296, 'Kt'), component(62296, 'yn'), component(62296, 'ys'),
  component(62296, 'Ss'), component(62296, 'Cs'), component(62296, 'Pn'),
  component(77778, 'Se'), component(77778, 'De'), component(77778, 'Ne'),
  component(77778, 'u'), component(77778, 'fe'), component(77778, 'le'), component(77778, 'ye'),
  component(40554, 'K'), component(40554, 'Z'), component(18442, 'd'),
  component(77778, 'be'), component(77778, 'je'), component(77778, 'Ce'),
  component(62296, 'Qe'), component(62296, 'gt'), component(62296, 'Ye'),
  component(62296, 'qe'), component(62296, 'Je'), component(62296, 'Xe')];
const rootSource = contracts[0].source;
for (const text of ['name:c.DASHBOARD_HEADER', 'name:c.DEVICES_AND_MODULES_HEADER', 'name:c.CHROMA_APPS',
  'n=s[a]||t[0].name', 's.bind(s,62296)', 's.bind(s,77778)', 's.bind(s,40554)'])
  if (!rootSource.includes(text)) throw Error(`Chroma root changed: ${text}`);
if (!contracts.find(c=>c.symbol === 'ys').source.includes('displayMode=chromaApp&containerId='))
  throw Error('Product iframe identity changed');
const keys = new Set([
  'DASHBOARD_HEADER','DEVICES_AND_MODULES_HEADER','CHROMA_APPS','TEXT_RAZER_CHROMA_DEVICES',
  'ONLINE_SERVICES_HEADER','MODULES_HEADER','FIRMWARE_UPDATES','AVAILABLE_MODULES','UPDATED_RECENTLY',
  'BACK','FORWARD','REFRESH','SETTINGS_HEADER','CLOSE','BRIGHTNESS_HEADER','SWITCH_OFF_LIGHTING',
  'QUICK_EFFECTS','ADVANCED_EFFECTS','QUICK_EFFECTS_MSG','EFFECTS','SPECTRUM_CYCLING','STATIC',
  'BREATHING','REACTIVE','AUDIO_METER','FIRE','RIPPLE','STARLIGHT','WAVE','WHEEL','APPLY','STATIC_GREEN','BREATHING_GREEN',
  'INTRODUCTION_BANNER_HEADING_1','INTRODUCTION_BANNER_HEADING_2','INTRODUCTION_BANNER_HEADING_3',
  'INTRODUCTION_BANNER_SYNAPSE_BODY_1','INTRODUCTION_BANNER_SYNAPSE_BODY_2','INTRODUCTION_BANNER_SYNAPSE_BODY_3',
  'INTRODUCTION_BANNER_CHROMA_APP_BODY_1','INTRODUCTION_BANNER_CHROMA_APP_BODY_2','INTRODUCTION_BANNER_CHROMA_APP_BODY_3',
  'INTRODUCTION_BANNER_START_TOUR','INSTALL','DASHBOARD_CHROMA_STUDIO','DASHBOARD_CHROMA_CONNECT',
  'DASHBOARD_AUDIO_VISUALIZER','DASHBOARD_PHILIPS_HUE','NANO_LEAF','DASHBOARD_SENSA',
  'RAZER_STORE','RAZER_STORE_DESC','RAZER_GOLD_AND_SILVER','RAZER_GOLD_AND_SILVER_DESC',
  'RAZER_COMMUNITY','RAZER_COMMUNITY_DESC','RAZER_SUPPORT','RAZER_SUPPORT_DESC',
]);
const appSymbols = ['Vo','T4Z','Ktu','s84','puU','WEP','saE','vBx','fC_','kB8','BsS','rOE','sGO','col','v2q','izv','h7Q','WYy','Her','$tG','MON','JqF','xjj','WWc','zX5','PWu','nKY','nr7'];
const appKeyMap = Object.fromEntries(appSymbols.map(symbol => {
  const result = value(37927, symbol);
  if (typeof result !== 'string') throw Error(`Missing app language key ${symbol}`);
  keys.add(result); return [symbol, result];
}));
const locales = {};
for (const [locale,id] of [['en',51971],['zh-CN',40994],['zh-TW',10582],['de',81655],['es',84520],
  ['fr',93828],['ja',61121],['kr',70497],['pt-BR',56853],['ru',83189]]) {
  locales[locale] = {};
  for (const key of [...keys].sort()) {
    const generic=[...modules.values()].find(m=>m.file.path.includes(`${base}/static/js/trans-${locale}.`)&&m.exports.has('LANGUAGE'));
    const result = value(id, key) ?? (generic&&evaluate(generic,generic.exports.get(key)));
    if (typeof result === 'string') locales[locale][key] = result;
  }
}
const css = [];
for (const name of fs.readdirSync(path.join(root,base,'static/css')).filter(n=>n.endsWith('.css'))) {
  const p = `${base}/static/css/${name}`, source = read(p);
  for (const match of source.matchAll(/[^{}]*\{[^{}]*\}/g)) {
    if (/ChromaOnboard_|AppContainer_|AppToolbar_|ChromaWorkshop_|ChromaAppsContainer_|lighting-modal|chroma-app-backdrop|^\.box-item-device(?:\{| .box-img-container\{| .name-tag)|^\.chroma-app-dashboard-wrapper\{|^\.items\{|^\.item-header/.test(match[0]))
      css.push({path:p,offset:match.index,sha256:hash(source),rule:match[0]});
  }
}
const manifest = JSON.parse(read(`${base}/asset-manifest.json`));
const media = [...new Set(contracts.flatMap(c=>[...c.source.matchAll(/static\/media\/[\w.-]+\.(?:svg|avif|png|webp)/g)].map(x=>x[0]))
  .concat(css.flatMap(c=>[...c.rule.matchAll(/static\/media\/[\w.-]+\.(?:svg|avif|png|webp)/g)].map(x=>x[0]))))].sort();
const mediaInventory = media.map(asset => ({path: asset,
  in_manifest: Object.values(manifest.files).includes(`/chroma-app/dashboard/${asset}`),
  source: 'exact mounted JSX or CSS URL'}));
const report = {source: base, manifest_sha256: hash(read(`${base}/asset-manifest.json`)),
  window: {name: value(69937, 'HqM'), flags: value(69937, 'oj6'),
    module: receipt(modules.get(69937).file, modules.get(69937).node)},
  root_module:23322, root_component:'Ks', default_tab:'DASHBOARD_HEADER',
  tabs:[{key:'DASHBOARD_HEADER',module:62296},{key:'DEVICES_AND_MODULES_HEADER',module:40554},{key:'CHROMA_APPS',module:77778}],
  settings:{window:'settings-chroma',url:'/chroma-app/settings/',flags:['sameWindow','autoFocus'],
    boundary:'The actually called Fs.clickSetting name differs from unused settings-chroma-app table metadata.'},
  device_open:{owner:'62296/yn',kind:'in-page modal iframe',mode:'chromaApp',identity:['productId','deviceContainerId','serialNumber'],optional:'isSensaDevice=true'},
  links: {workshop_apps:value(65596,'$I'),workshop_games:value(65596,'uz'),workshop_profiles:value(65596,'rj')},
  appKeyMap, locales, contracts, css, media: mediaInventory,
  boundaries:['No services, application, downloaded JavaScript/WASM/DLL, build or tests executed.',
    'Chroma service app list, installed modules, firmware updates, WDL device discovery and immersive capability are not synthesized.',
    'A registered product chromaApp branch does not by itself establish that hardware is a Chroma device.']};
// The product root reuses the same Fh/yh child as the ordinary 653 Lighting tab.
const productPath='.ref/devices/653/static/js/main.7b71cce5.js';
const productSource=read(productPath), productAst=acorn.parse(productSource,{ecmaVersion:'latest'});
const productContracts=[];
walk(productAst,node=>{
  if(node.start>7700000 && ['ClassDeclaration','VariableDeclarator'].includes(node.type)
      && ['lg','rg','_g','yh','fh','rP','AP','Gh'].includes(node.id?.name))
    productContracts.push({symbol:node.id.name,...receipt({path:productPath,source:productSource},node)});
});
if(!productContracts.some(r=>r.symbol==='_g'&&r.source.includes('(0,ka.jsx)(Fh,{isIframe:!0})'))
    ||!productContracts.some(r=>r.symbol==='yh'&&r.source.includes('(0,ka.jsx)(rP,{})')))
  throw Error('653 Chroma lighting root changed');
report.product653={contracts:productContracts,
  native:'The same current local 653 lighting content is rendered through its existing entity; navigation and profile chrome are omitted.',
  boundaries:['Other product roots are not routed through 653.', 'Service-driven WDL warning, global Chroma profiles and device synchronization remain outside this local projection.']};
// Settings is a separate current application, not the Synapse settings sample.
const settingsBase='.ref/applications/chroma-app/settings';
for(const name of fs.readdirSync(path.join(root,settingsBase,'static/js')).sort())
  if(name.endsWith('.js'))parse(`${settingsBase}/static/js/${name}`);
const settingsContracts=['Zo','ui','Ei','ni','ci','di','mi','vi','Hs','ye','An','Zs','de','Ae','Ps','Me'].map(n=>component(1743,n));
const settingsModule=modules.get(1743), settingsTextModule=settingsModule.bindings.get('V').arguments[0].value;
const settingsKeys=new Set(['CHROMA_APP','GENERAL','SETTINGS_HEADER','BACK','FORWARD','CLOSE']);
for(const c of settingsContracts)for(const m of c.source.matchAll(/V\.([\w$]+)/g)){
  const k=value(settingsTextModule,m[1]);if(typeof k==='string')settingsKeys.add(k);
}
const settingsLocales={};
for(const locale of Object.keys(locales)){
  const m=[...modules.values()].find(m=>m.file.path.includes(`${settingsBase}/static/js/trans-${locale}.`)
    &&m.exports.has('LANGUAGE'));
  if(!m)throw Error(`Missing Chroma settings locale ${locale}`);
  settingsLocales[locale]={};
  for(const k of [...settingsKeys].sort()){
    const v=evaluate(m,m.exports.get(k));if(typeof v==='string')settingsLocales[locale][k]=v;
  }
}
const versionManifest=JSON.parse(read(`${base}/manifest.json`));
report.settings_source={base:settingsBase,default_tab:'CHROMA_APP',tabs:['CHROMA_APP','GENERAL'],
  contracts:settingsContracts,locales:settingsLocales,
  app_version:['4',...versionManifest.version.split('.').slice(1),String(versionManifest.buildVersion)].join('.'),
  wdl_gate:'OS build >=22631 or build22621 revision>=2506; hidden while OS capability is unknown',
  boundaries:['Auto-launch is a local draft preference; no Windows startup registration is changed.',
    'Migration, release notes service, WDL handoff and external app operations are not simulated.']};
const target='docs/re/chroma-app-current-evidence.json', rendered=JSON.stringify(report,null,2)+'\n';
function output(p,content){if(process.argv.includes('--check')){if(read(p)!==content)throw Error(`${p} differs from current source`);}else fs.writeFileSync(path.join(root,p),content);}
output(target,rendered);
for(const [locale,strings] of Object.entries(locales)) {
  const p=`locales/${locale}.json`, data=JSON.parse(read(p));
  data.CHROMA_SOURCE=strings;
  data.CHROMA_SETTINGS_SOURCE=settingsLocales[locale];
  output(p,JSON.stringify(data,null,2)+'\n');
}
console.log(`Chroma: ${contracts.length} mounted contracts, ${css.length} CSS rules and ${keys.size} locale keys parsed statically.`);
