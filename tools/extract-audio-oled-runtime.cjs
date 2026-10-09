// Parse current 1383 reducer/UI/CSS as data. Never execute the vendor bundle.
const fs=require('fs'),path=require('path');
const {Source,walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),directory='.ref/devices/1383';
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
const manifest=JSON.parse(read(`${directory}/asset-manifest.json`));
const source=Object.create(Source.prototype);
Object.assign(source,{directory,files:[...new Set(Object.values(manifest.files))].filter(f=>f.includes('/static/js/')&&f.endsWith('.js')).map(f=>`${directory}/${f.slice(f.indexOf('static/'))}`),modules:new Map(),texts:new Map(),parsed:new Set()});
const receipts=[];
const binding=(module,symbol)=>{const node=source.binding(module,symbol);receipts.push({module,symbol,...source.receipt(module,node)});return node;};
const property=(node,name)=>{const found=node.properties.filter(p=>key(p.key)===name);if(found.length!==1)throw Error(`Expected ${name}`);return found[0].value;};
const device=binding(33397,'I'),language=source.literal(33397,binding(33397,'hn')),loading=source.literal(33397,binding(33397,'Cn')).oledLoading;
const connection=Object.fromEntries(['isBle','isDongle'].map(name=>[name,source.literal(33397,property(device,name))]));
const error=source.literal(79826,property(binding(79826,'T'),'errorPayload'));
if(connection.isBle!==false||connection.isDongle!==false||error!==null||language.oledLanguage!==0||language.oledLanguageChanged!==0||loading.type!=='none'||loading.target!=='animation')throw Error('Changed runtime defaults');
for(const module of [33397,79826])walk(source.module(module).fn,n=>{
 if(n.type==='Property'&&['oledLanguageReducer','oledLoadingReducer'].includes(key(n.key)))receipts.push({module,symbol:key(n.key),...source.receipt(module,n.value)});
 if(n.type==='SwitchCase'&&n.test?.type==='MemberExpression'&&/^(?:SET_OLED_HOME_SCREEN_DISPLAY_ERROR_(?:REVERT|RETRY)|GET_OLED_DATA)$/.test(key(n.test.property)))receipts.push({module,symbol:key(n.test.property),...source.receipt(module,n)});
});
for(const name of ['xx','Dv','Hp','Bp','Bv','gx','ix','Qv','cx','hx','ux','vx','pp','Yv','k','px'])binding(51278,name);
receipts.push({module:50151,symbol:'button',...source.receipt(50151,source.module(50151).fn)});
const labels={};for(const symbol of ['QAd','l2t','QWf','bOp','Jf5','$PN','JQR','B3A','p6C','sUR','ZV_','_Hj','kM7','dt6','R42','stg','tus','De8','XVF']){const node=source.exported(54693,symbol);labels[symbol]=source.literal(54693,node);receipts.push({module:54693,symbol,...source.receipt(54693,node)});}
const warning=binding(27875,'r'),paths=[];walk(warning,n=>{if(n.type==='CallExpression'&&n.callee.property?.name==='createElement'&&n.arguments[0]?.value==='path')paths.push(n)});
if(paths.length!==1)throw Error('Changed warning icon');
const warningPath=source.literal(27875,property(paths[0].arguments[1],'d'));
const warningSvg=`<svg xmlns="http://www.w3.org/2000/svg" width="20" height="27" viewBox="0 0 20 20"><path fill="#fd8611" d="${warningPath}"/></svg>\n`;
const cssPaths=[...new Set(Object.values(manifest.files))].filter(f=>f.includes('/static/css/')&&f.endsWith('.css')).map(f=>`${directory}/${f.slice(f.indexOf('static/'))}`);
const css=cssPaths.flatMap(file=>parseCSS(read(file)).filter(r=>/WarningAlert_|ProgressLoading_|SimpleLoading_|DisplayWidget_|turn-off-ble-tooltip|ble-edit|customize-setting-button|^\.disabled$|^\.progress-bar(?: |$)/.test(r.selector)).map(r=>({path:file,sha256:hash(read(file)),...r})));
if(css.some(r=>r.selector.includes('ble-edit:hover')))throw Error('Reaudit new BLE hover behavior');
const evidence={method:'Current manifest-selected static AST/CSS; UTF16 source offsets and full-file SHA256.',manifest:{path:`${directory}/asset-manifest.json`,sha256:hash(read(`${directory}/asset-manifest.json`))},receipts,css,icons:[{output:'synapse/audio-oled-runtime-warning.svg',svg:warningSvg,receipt:source.receipt(27875,warning)}]};
const data={connection,language:{value:language.oledLanguage,changed:language.oledLanguageChanged},loading,error,labels};
const check=process.argv.slice(2).join(' ')==='--check';if(process.argv.length>2&&!check)throw Error('Unknown argument');
for(const [file,payload] of [['crates/razer-pages/src/features/audio_oled_runtime_data.json',JSON.stringify(data,null,2)+'\n'],['docs/re/audio-oled-runtime-current-evidence.json',JSON.stringify(evidence,null,2)+'\n'],['assets/synapse/audio-oled-runtime-warning.svg',warningSvg]]){if(check){if(read(file)!==payload)throw Error(`Stale ${file}`);}else fs.writeFileSync(path.join(root,file),payload);}
console.log(`1383 OLED runtime: ${receipts.length} source receipts, ${css.length} CSS rules, source defaults and one warning icon.`);
