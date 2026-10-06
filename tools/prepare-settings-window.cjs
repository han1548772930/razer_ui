// Current /settings/ is parsed as data. No reference JS is imported/evaluated.
const fs = require('fs');
const path = require('path');
const acorn = require('acorn');
const {parseCSS} = require('./css-source.cjs');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const check = process.argv.includes('--check');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const emit = (file, value) => {
  const bytes = typeof value === 'string' ? value : JSON.stringify(value, null, 2) + '\n';
  if (check) { if (read(file) !== bytes) throw Error(`Stale ${file}`); }
  else fs.writeFileSync(path.join(root, file), bytes);
};
const requireFact = (test, message) => {if (!test) throw Error(message);};
const source = new Source('settings');
const manifestPath = '.ref/applications/settings/asset-manifest.json';
const manifest = JSON.parse(read(manifestPath));
// This application uses absolute public URLs, unlike product relative URLs.
source.files = [...new Set(Object.values(manifest.files))]
  .filter(file => /^\/settings\/static\/js\/[^/]+\.js$/.test(file))
  .map(file => '.ref/applications' + file);
const receipts = [];
const binding = (module, name) => {
  const node = source.binding(module, name);
  receipts.push({module, name, ...source.receipt(module, node)});
  return node;
};
const rootNode = binding(8821, 'ss');
const rootText = source.snippet(8821, rootNode);
const nav = ['GD6', 'n6W', 'iRj'].map(name => source.literal(4693, source.exported(4693, name)));
requireFact(JSON.stringify(nav) === '["TEXT_SOFTWARE","TEXT_SYSTRAY_SETTING","TEXT_GENERAL_SETTING"]', 'Settings navigation changed');
requireFact(rootText.includes('n=t[0].name') && rootText.includes('isShowProfileMigrationIcon:!1'), 'Settings initial root changed');
for (const [id, names] of [[8821,['de','ns']], [9302,['ie','ee','$']], [9762,['x','fe','ge','ve','de','ye']], [6584,['u','h']], [3414,['u','be','Se']]])
  for (const name of names) binding(id, name);
const appFiles = source.files.map(file => ({path:file, sha256:hash(read(file))}));
for (const {path:file} of appFiles) requireFact(!read(file).includes('settingsScrollToSection'), `New legacy section listener: ${file}`);
const sdk = source.module(2954);
const sdkLaunch = [];
walk(sdk.fn, node => {
  if (node.type === 'CallExpression' && node.arguments[1]?.value === 'launch'
      && node.arguments[2]?.type === 'CallExpression') sdkLaunch.push(node);
});
requireFact(sdkLaunch.length === 1, 'SDK launch binding changed');
receipts.push({module:2954,name:'launch',...source.receipt(2954,sdkLaunch[0])});
const hostPath = '.ref/host-4.0.827/electron/main.js';
const hostText = read(hostPath), hostAst = acorn.parse(hostText,{ecmaVersion:'latest'});
const hostCases = [];
walk(hostAst, n => {if(n.type === 'SwitchCase' && n.test?.value === 'razer-settings') hostCases.push(n);});
requireFact(hostCases.length === 1, 'Host settings launch changed');
const hostCase = hostCases[0];
receipts.push({name:'host-launch',path:hostPath,sha256:hash(hostText),offset:hostCase.start,end:hostCase.end,source:hostText.slice(hostCase.start,hostCase.end)});
requireFact(hostText.slice(hostCase.start,hostCase.end).includes('width:1280,height:720'), 'Host initial bounds changed');
const constantsPath = '.ref/host-4.0.827/electron/constants.js';
const constantsText = read(constantsPath), defaults = [];
walk(acorn.parse(constantsText,{ecmaVersion:'latest'}), n => {
  if(n.type === 'AssignmentExpression' && n.left.type === 'MemberExpression'
      && key(n.left.property) === 'WINDOW_SIZE_DEFAULTS') defaults.push(n);
});
requireFact(defaults.length === 1, 'Ambiguous host size defaults');
const limits = Object.fromEntries(defaults[0].right.properties.filter(p => p.value.type === 'Literal').map(p => [key(p.key),p.value.value]));
requireFact(limits.minimumWidth === 600 && limits.minimumHeight === 500, 'Host minimum size changed');
requireFact(hostText.includes('WINDOW_SIZE_DEFAULTS:M') && hostText.includes('}=require("./constants")'), 'Host defaults import changed');
requireFact(hostText.slice(hostCase.start,hostCase.end).includes('minimum_width:M.minimumWidth,minimum_height:M.minimumHeight'), 'Settings minimum binding changed');
receipts.push({name:'host-window-size-defaults',path:constantsPath,sha256:hash(constantsText),offset:defaults[0].start,end:defaults[0].end,source:constantsText.slice(defaults[0].start,defaults[0].end)});

const toolbarKeys = Object.fromEntries([['back','IUp'],['forward','OXp'],['refresh','aIK']].map(([name, exported]) => [name, source.literal(4693, source.exported(4693, exported))]));
const keys = [...nav, ...Object.values(toolbarKeys), 'SETTINGS_HEADER','TEXT_INSTALLED','TEXT_AUTO_UPDATE_ENABLED','TEXT_QUICK_LAUNCHER',
  'PREVIEW','ORDER_FROM_LEFT_TO_RIGHT','SLOT','NONE','GENERAL','TEXT_WIDGETS','SYSTRAY_ICON','SYSTRAY_ICON_DESC',
  'DROPDOWN_SYSTRAY_1','DROPDOWN_SYSTRAY_2','LANGUAGE','ABOUT','VERSION','COPYRIGHT','TRADEMARK','FAQ',
  'TEXT_TERMS_OF_SERVICE','TEXT_PRIVACY_POLICY','TEXT_OPEN_SOURCE_SOFTWARE_NOTICE','CONNECT_WITH_US',
  'FOLLOW_SOCIAL_FACEBOOK','FOLLOW_SOCIAL_IG','FOLLOW_SOCIAL_TWITTER','FOLLOW_SOCIAL_YOUTUBE',
  'FOLLOW_SOCIAL_TIKTOK','FOLLOW_SOCIAL_TWITCH','FOLLOW_SOCIAL_DISCORD'];
const importer = source.binding(8442,'i');
const translations = {};
for (const prop of importer.properties) {
  const locale = key(prop.key), calls=[];
  walk(prop.value,n=>{if(n.type==='CallExpression'&&key(n.callee.property)==='bind')calls.push(n);});
  requireFact(calls.length===1,'Locale module resolution changed');
  const id=calls[0].arguments[1].value;
  translations[locale.toLowerCase()] = Object.fromEntries(keys.map(k=>[k,source.literal(id,source.exported(id,k))]));
}
const languageNames = source.literal(5596,source.exported(5596,'Ay'));
const languages = source.literal(5596,source.exported(5596,'Fc')).map(code=>({code,label:languageNames['TEXT_LANGUAGE_'+code.toUpperCase().replace('-','_')]}));
const links = {};
for (const [name, exported] of [['FAQ','my'],['TEXT_TERMS_OF_SERVICE','FG'],['TEXT_PRIVACY_POLICY','rg'],['TEXT_OPEN_SOURCE_SOFTWARE_NOTICE','g4']]) {
  const node = source.binding(5596,source.exported(5596,exported).name);
  requireFact(node.type==='LogicalExpression'&&node.operator==='||'&&node.right.type==='Literal', 'Policy URL shape changed');
  const left=node.left;
  requireFact(left.type==='MemberExpression'&&left.object.type==='ObjectExpression'
    &&!left.object.properties.some(p=>key(p.key)===key(left.property)), 'Policy override now exists');
  links[name] = node.right.value;
  receipts.push({name:'policy-'+name,module:5596,...source.receipt(5596,node)});
}
const systrayManifestPath='.ref/applications/systray/systrayv2/manifest.json';
const systrayManifest=JSON.parse(read(systrayManifestPath));
const version=(systrayManifest.version||'').replace('1.','4.')+'.'+systrayManifest.buildVersion;
const social = [];
const socialLinksNode = binding(3414, 'ye');
const socialLinks = socialLinksNode.properties.map(p => {
  const fields = Object.fromEntries(p.value.properties.filter(q => ['link', 'translationType'].includes(key(q.key))).map(q => [key(q.key), source.literal(3414, q.value)]));
  return {name: key(p.key), ...fields};
});
for (const [name,symbol] of [['facebook','L'],['instagram','D'],['twitter','ie'],['youtube','he'],['tiktok','X'],['twitch','Q'],['discord','N'],['insider','ge']]) {
  const node=binding(3414,symbol), shapes=[],styles=[];
  walk(node,n=>{
    if(n.type!=='CallExpression'||key(n.callee.property)!=='createElement')return;
    const tag=n.arguments[0]?.value;
    if(tag==='style' && n.arguments.length>2) {requireFact(n.arguments[2]?.type==='Literal','Dynamic inline social style');styles.push(n.arguments[2].value);}
    if(!['circle','path','rect'].includes(tag))return;
    const attrs={};
    requireFact(n.arguments[1].type==='ObjectExpression','Dynamic social shape');
    for(const p of n.arguments[1].properties){requireFact(p.value.type==='Literal','Dynamic social attribute');attrs[key(p.key)==='className'?'class':key(p.key)]=String(p.value.value);}
    shapes.push({tag,attrs});
  });
  requireFact(shapes.length>0,'Missing social shapes');
  social.push({name,shapes,styles,source:source.receipt(3414,node)});
}
// Preserve CSS rule text and its media ancestors: a renamed selector is not a re-audit.
const css=[];
for(const url of [...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css'))) {
  const file='.ref/applications'+url, text=read(file);
  for (const rule of parseCSS(text)) {
    if(!/(?:^\*|^body|^html|^#root|^\.disabled$|^\[tooltip\]|\.main-container|\.body-wrapper|\.body-widgets|\.widget-col|\.widget(?:\s|\.|\{|$)|\.toolbar|\.nav-tabs|\.software-tab|\.installed-|\.section|\.settings|\.social-|\.dropdown|\.slots)/.test(rule.selector))continue;
    if(rule.declarations.length>6000)continue;
    css.push({path:file,sha256:hash(text),...rule});
  }
}
const hasDeclaration = (selector, property, value) => css.some(rule => rule.selector === selector && rule.properties.some(p => p.property === property && p.value === value));
for (const [selector, property, value] of [
  ['.toolbar .arrow','width','40px'],['.toolbar .arrow','height','38px'],
  ['.toolbar .arrow','background-size','20px 20px'],['.toolbar','z-index','301'],
  ['.toolbar .navigation','flex','auto'],['.toolbar .title','flex','3 1 340px'],
  ['.toolbar .right','flex','0 0 20%'],
  ['.social-container','gap','24px'],['.social-container','margin-bottom','10px'],
  ['.social-container .tooltip','bottom','37px'],['.social-container .tooltip','padding','8px 10px'],
  ['[tooltip]:before','line-height','16px'],['[tooltip]:before','top','calc(100% + 5px)'],
]) requireFact(hasDeclaration(selector, property, value), `Settings presentation changed: ${selector} ${property}`);
emit('src/shell/settings_window_data.json',{nav,translations,languages,links,version,socialLinks,toolbarKeys});
emit('docs/re/settings-window-current-evidence.json',{
  schema_version:1,method:'Acorn and maintained CSS receipt parser only; reference JavaScript is never loaded or evaluated.',
  manifest:{path:manifestPath,sha256:hash(read(manifestPath))},
  systray_manifest:{path:systrayManifestPath,sha256:hash(read(systrayManifestPath)),version},
  navigation:nav,legacy_section_listener:{matches:0,files:appFiles},receipts,css,social,
});
console.log(`Settings window source ${check?'checked':'prepared'}: ${receipts.length} AST receipts, ${css.length} CSS rules, ${Object.keys(translations).length} locales`);
