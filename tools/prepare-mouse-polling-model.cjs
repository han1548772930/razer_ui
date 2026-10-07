// Current source capability data, not device observations. No vendor code executes.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {Source,walk,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),check=process.argv.includes('--check');
const read=f=>fs.readFileSync(path.join(root,f),'utf8');
const products=[],evidence=[];
for(const [entry,behavior,names]of [
 [182,'firmware_expansion',['GM','OM','lM','dm','Rm','yE','Cm','Nm','MM','mM','DM','CM','PM','pM']],
 [226,'dual_link_restriction',['wi','Xs','zs','js','Ls']]
]) {
 const directory=`.ref/devices/${entry}`,manifestPath=directory+'/asset-manifest.json',manifest=JSON.parse(read(manifestPath));
 const files=[...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.js')).map(f=>directory+'/'+f.replace(/^\.\//,''));
 const source=Object.assign(Object.create(Source.prototype),{directory,files,modules:new Map(),texts:new Map(),parsed:new Set()});
 if(behavior==='dual_link_restriction')source.parse(directory+'/static/js/8355.3d5e573e.chunk.js');
 const main=directory+'/'+manifest.files['main.js'].replace(/^\.\//,''),text=read(main),ast=acorn.parse(text,{ecmaVersion:'latest'});
 const receipts=[];
 let target, dock, reducer;
 if(behavior==='firmware_expansion') {
  for(const name of names){const matches=[];walk(ast,n=>{if(n.start>4000000&&n.id?.name===name&&['ClassDeclaration','FunctionDeclaration','VariableDeclarator'].includes(n.type))matches.push(n.init??n);});
   if(matches.length!==1)throw Error(`Ambiguous current ${entry}:${name}`);const n=matches[0];
   receipts.push({symbol:name,path:main,sha256:hash(text),offset:n.start,end:n.end,source:text.slice(n.start,n.end)});
   if(name==='Rm')target=n;if(name==='yE')reducer=n;
  }
 } else {
  for(const name of names)receipts.push({symbol:name,...source.receipt(4125,source.binding(4125,name))});
  target=source.binding(4125,'zs');dock=source.binding(4125,'Ls');
  const matches=[];walk(ast,n=>{if(n.type==='ObjectExpression'&&n.properties.some(p=>p.key?.name==='pollingRate')&&n.properties.some(p=>p.key?.name==='pollingRateWireless')&&n.properties.some(p=>p.key?.name==='actionsFromUI'))matches.push(n);});
  if(matches.length!==1)throw Error('Ambiguous current polling reducer');reducer=matches[0];
  receipts.push({symbol:'polling reducer defaults',path:main,sha256:hash(text),offset:reducer.start,end:reducer.end,source:text.slice(reducer.start,reducer.end)});
 }
 const exported=(module,name)=>{const n=source.exported(module,name);const binding=n.name?source.binding(module,n.name):n;receipts.push({symbol:`${module}:${name}`,...source.receipt(module,binding)});return source.literal(module,n);};
 const info=exported(1057,'DeviceInfo');
 receipts.push({symbol:'CONFIG exports',...source.receipt(1057,source.module(1057).fn)});
 if(info.productId!==entry)throw Error('Source identity changed');
 const rateKeys=['POLLING_RATE','POLLING_RATE_WIRELESS','HYPER_POLLING_RATE','HYPER_POLLING_RATE_SUPPORT_8K'];
 const rates={};for(const key of rateKeys)if(source.module(1057).exports.has(key))rates[key]=exported(1057,key).map(x=>Number(x.content));
 const firmwareFloor=source.module(1057).exports.has('POLLING_RATE_8K_FW_VERSION')?exported(1057,'POLLING_RATE_8K_FW_VERSION'):null;
 const dongleHyper=source.module(1057).exports.has('isDongleHyperpollingDevice')?exported(1057,'isDongleHyperpollingDevice'):null;
 const masterIds=['zrS','WgN'].map(k=>exported(9937,k));
 const versionComparator=source.exported(1867,'U5');
 receipts.push({symbol:'firmware comparison U5',...source.receipt(1867,source.binding(1867,versionComparator.name))});
 for(const id of [8988,6130,4547,8490,8816,1468,2574,2314,8209])receipts.push({symbol:'version dependency module:'+id,...source.receipt(id,source.module(id).fn)});
 const labels={};for(const key of ['FGZ','lGq','iiK','orU','rJp','KOq','Vjf','HkB','aqm','yer','bSO','RKF','zzI','XyT'])labels[key]=exported(4693,key);
 const targetSource=receipts.find(r=>r.symbol===(behavior==='firmware_expansion'?'Rm':'zs')).source;
 for(const token of ['checkActivePollingRateValue','pollingRateWireless','checkIfPairedWithHyperPollingDevice','connectedToHyperPollingMasterDevice'])if(!targetSource.includes(token))throw Error('Re-audit current polling '+token);
 const thresholds=new Set();let dockCap=null,minDevices=null;
 walk(target,n=>{
  if(n.type==='BinaryExpression'&&n.operator==='>'&&n.right.type==='Literal'&&typeof n.right.value==='number')thresholds.add(n.right.value);
  if(n.type==='CallExpression'&&n.callee.property?.name==='applyPollingRateLimit'&&n.arguments[0]?.type==='Literal')dockCap=n.arguments[0].value;
 });
 thresholds.delete(0);
 if(thresholds.size!==1)throw Error(`Re-audit current warning threshold ${entry}: ${[...thresholds]}`);
 if(dock)walk(dock,n=>{if(n.type==='BinaryExpression'&&n.operator==='>='&&n.left.property?.name==='length'&&n.right.type==='Literal')minDevices=n.right.value;});
 const wireless=reducer.properties.find(p=>p.key?.name==='pollingRateWireless')?.value;
 if(wireless?.type!=='Literal')throw Error('Re-audit wireless initial value');
 const limits=behavior==='dual_link_restriction'?{dual_link_hz:info.dualLinkPollingRateLimitHz,dock_hz:dockCap,dock_min_devices:minDevices}:null;
 if(limits&&Object.values(limits).some(x=>!Number.isFinite(x)||x<=0))throw Error('Missing source link limits');
 const normalRoot=receipts.find(r=>r.symbol===(behavior==='firmware_expansion'?'OM':'wi')).source;
 if(!(normalRoot.includes('this.props.isBle?null:') || normalRoot.includes('!this.props.isBle||!0===') && info.supportBluetoothPollingRate!==true))throw Error('Re-audit mounted BLE polling root before enabling a field');
 products.push({product_id:info.productId,device_ids:[info.productId,info.dongleId].filter(x=>x!=null),pairing_dongle_id:info.dongleId,
  pair_identity:targetSource.includes('this.isCurrentDevice=')?'current_device_ids':'strict_dongle_id',
  master_supports_8k_flag:targetSource.includes('connectedToHyperPollingMasterDevice=o.includes')&&targetSource.includes('||r,'),
  is_dongle_hyperpolling_device:dongleHyper===true,
  rates,firmware_floor:firmwareFloor,hyper_master_ids:masterIds,high_rate_threshold_hz:[...thresholds][0],fallback_wireless_hz:wireless.value,
  ble_visibility:'hidden',limits,labels});
 const css=[];for(const relative of [...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css'))){const file=directory+'/'+relative.replace(/^\.\//,'');const value=read(file);const rules=parseCSS(value).filter(r=>/polling-btn-set|customize-polling-rate-button|polling-warn|polling-learn-more|external-link-icon|^\.img-text \.windows/.test(r.selector));if(rules.length)css.push({path:file,sha256:hash(value),rules});}
 evidence.push({product_id:entry,manifest:{path:manifestPath,sha256:hash(read(manifestPath))},receipts,css});
}
const assets=[['common-windows-11.d477cadb.svg','windows-11.svg'],['windows_logo.8fb1e7e2.svg','windows.svg'],['icon_external_link.48227e72.svg','external-link.svg']].map(([original,local])=>{
 const source='.ref/devices/182/static/media/'+original,output='assets/synapse/'+local,a=fs.readFileSync(path.join(root,source)),b=fs.readFileSync(path.join(root,output));if(!a.equals(b))throw Error('Current 182 asset differs '+local);return{source,output,sha256:hash(a)};
});
const infoSource=JSON.parse(read('.ref/devices/226/asset-manifest.json')).files['static/media/info-icon.svg'];
const infoPath='.ref/devices/226/'+infoSource.replace(/^\.\//,''),infoOutput='assets/synapse/polling-info.svg',infoBytes=fs.readFileSync(path.join(root,infoPath));
if(check){if(!infoBytes.equals(fs.readFileSync(path.join(root,infoOutput))))throw Error('Current polling info asset differs');}else fs.writeFileSync(path.join(root,infoOutput),infoBytes);
assets.push({source:infoPath,output:infoOutput,sha256:hash(infoBytes)});
const loader=read('src/resources.rs'),embedded=read('assets/synapse/embedded.rs');
for(const asset of assets){const key=asset.output.replace('assets/','');if(!embedded.includes('"'+key+'"')&&!loader.includes('"'+key+'"'))throw Error('Asset is not registered: '+key);}
if((loader.match(/\.chain\(MOUSE_POLLING_ASSETS\)/g)||[]).length!==2)throw Error('Polling resources must be registered in load and list');

for(const [file,value]of [['src/features/mouse_polling_source_data.json',products],['docs/re/mouse-polling-model-current-evidence.json',{method:'Current manifest/AST/CSS/resource parsing only; no reference execution. Descriptors describe UI capabilities, never observed device state.',scope:'Two independently re-audited current polling implementations; other products are not implicitly accepted.',completed_products:0,evidence,assets}]]){const text=JSON.stringify(value,null,2)+'\n';if(check){if(read(file)!==text)throw Error('Stale '+file);}else fs.writeFileSync(path.join(root,file),text);}
console.log(`Polling source model: ${products.length} products, ${evidence.reduce((n,p)=>n+p.receipts.length,0)} current AST receipts; ${assets.length} resource comparisons.`);
