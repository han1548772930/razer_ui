// Current 226 Polling UI/producer receipts. Only static AST/CSS/byte parsing.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {Source, walk, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), check = process.argv.includes('--check');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const directory = '.ref/devices/226', manifestPath = directory + '/asset-manifest.json';
const manifest = JSON.parse(read(manifestPath));
const source = Object.assign(Object.create(Source.prototype), {directory,
  files: [...new Set(Object.values(manifest.files))].filter(f => f.endsWith('.js')).map(f => directory + '/' + f.slice(2)),
  modules: new Map(), texts: new Map(), parsed: new Set()});
source.parse(directory + '/static/js/8355.3d5e573e.chunk.js');
const receipts = ['wi','Ri','zs','Xs','Ps','js','Ls','$s','Ys'].map(symbol => ({symbol,
  ...source.receipt(4125, source.binding(4125, symbol))}));
walk(source.module(4125).fn,n=>{
  if(n.type==='VariableDeclarator' && n.id.type==='ObjectPattern' && n.id.properties.some(p=>p.key?.name==='POLLING_RATE_WIRELESS'))
    receipts.push({symbol:'polling configuration bindings',...source.receipt(4125,n)});
});
receipts.push({symbol:'CONFIG', ...source.receipt(1057, source.module(1057).fn)});
const config = source.literal(1057, source.exported(1057, 'DeviceInfo'));
for (const [key, value] of Object.entries({productId:226,dongleId:227,bleId:228,supportBluetoothPollingRate:false,dualLinkPollingRateLimitHz:1000})) {
  if (config[key] !== value) throw Error('Re-audit CONFIG.' + key);
}
const labels = {};
for (const symbol of ['FGZ','lGq','iiK','orU','rJp','H_3','kcN','KOq','Vjf','HkB']) {
  labels[symbol] = source.literal(4693, source.exported(4693,symbol));
  receipts.push({symbol:'label:'+symbol, ...source.receipt(4693, source.binding(4693,source.exported(4693,symbol).name))});
}
const mounted = receipts.find(r => r.symbol === 'zs').source;
for (const text of ['this.props.isDongle||this.props.isBle?"wireless":"wired"', '"pollingRateWireless":"pollingRate"',
  'this.props.pollingRateWireless>e&&this.props.setPollingRateWireless(e)', 'setInterval(this.handleDuallinkDevicesChange,1500)',
  'https://www.razer.com/technology/razer-hyperpolling#best-practices-tips']) {
  if (!mounted.includes(text)) throw Error('Re-audit polling behavior: ' + text);
}
// Re-read the main reducer and actual MW producers, not historical prose.
const windows = [
  {path:directory+'/static/js/main.08f95762.js',symbol:'deviceReducer defaults',anchor:145119,type:'VariableDeclarator'},
  {path:directory+'/static/js/main.08f95762.js',symbol:'polling reducer',anchor:175730,type:'VariableDeclarator'},
  {path:'.ref/middleware/226/7846.b84800eaaf18ff1145e5.js',symbol:'runtimeData publisher',anchor:1149497,type:'function'},
  {path:'.ref/middleware/226/7846.b84800eaaf18ff1145e5.js',symbol:'OTFS publisher',anchor:1254110,type:'function'},
  {path:'.ref/middleware/226/7846.b84800eaaf18ff1145e5.js',symbol:'connection identity',anchor:2114050,type:'function'},
  {path:'.ref/middleware/226/7846.b84800eaaf18ff1145e5.js',symbol:'polling getter',anchor:2130090,type:'function'},
];
const parsed = new Map();
for (const item of windows) {
  if (!parsed.has(item.path)) parsed.set(item.path,{text:read(item.path),ast:acorn.parse(read(item.path),{ecmaVersion:'latest'})});
  const {text,ast} = parsed.get(item.path);
  let found;
  walk(ast,n => {
    const type = item.type === 'function' ? /Function/.test(n.type) : n.type === item.type;
    if(type && n.start <= item.anchor && n.end > item.anchor && (!found || n.end-n.start < found.end-found.start)) found=n;
  });
  if(!found)throw Error('Missing AST window '+item.symbol);
  receipts.push({symbol:item.symbol,path:item.path,sha256:hash(text),offset:found.start,end:found.end,source:text.slice(found.start,found.end)});
}
const css=[];
for(const relative of [...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css'))){
  const file=directory+'/'+relative.slice(2),text=read(file);
  const rules=parseCSS(text).filter(r=>/polling-btn-set|customize-polling-rate-button|polling-dock-limit|polling-warn|polling-learn-more|external-link-icon|^\.h1-body$/.test(r.selector));
  if(rules.length)css.push({path:file,sha256:hash(text),rules});
}
const assets = [['info-icon.svg','mouse-226-polling-info.svg'],['icon_external_link.svg','mouse-226-polling-external.svg']].map(([key,name])=>{
  const source=directory+'/'+manifest.files['static/media/'+key].slice(2),output='assets/synapse/'+name;
  const bytes=fs.readFileSync(path.join(root,source));
  if(check){if(!bytes.equals(fs.readFileSync(path.join(root,output))))throw Error('Asset differs '+output);}
  else fs.writeFileSync(path.join(root,output),bytes);
  return {source,output,sha256:hash(bytes)};
});
const localeKeys=[...new Set(Object.values(labels))];
const locale = ['en','zh-CN'].map(language=>{
  const file='locales/'+language+'.json',text=read(file),values=JSON.parse(text);
  return {path:file,sha256:hash(text),labels:Object.fromEntries(localeKeys.map(key=>[key,values[key]??null]))};
});
const output=JSON.stringify({product_id:226,method:'Current AST/CSS and resource byte parsing only; UTF-16 half-open ranges',
  manifest:{path:manifestPath,sha256:hash(read(manifestPath))},config:{productId:config.productId,dongleId:config.dongleId,bleId:config.bleId,supportBluetoothPollingRate:config.supportBluetoothPollingRate,dualLinkPollingRateLimitHz:config.dualLinkPollingRateLimitHz},receipts,labels,css,assets,locale},null,2)+'\n';
const destination='docs/re/mouse-226-polling-current-evidence.json';
if(check){if(read(destination)!==output)throw Error('Stale '+destination);}
else fs.writeFileSync(path.join(root,destination),output);
console.log(`226 polling: ${receipts.length} AST receipts; ${css.reduce((n,c)=>n+c.rules.length,0)} CSS; ${assets.length} assets`);
