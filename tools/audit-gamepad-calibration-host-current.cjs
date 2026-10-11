// Current host JS and package boundary receipts only; no modules are loaded.
const fs=require('fs'),acorn=require('acorn'),crypto=require('crypto');
const base='local-ui-reverse/source/installed/app-4.0.827/';
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
function walk(n,f){if(!n?.type)return;f(n);for(const v of Object.values(n)){if(Array.isArray(v))for(const c of v)walk(c,f);else if(v?.type)walk(v,f);}}
const files=[];
for(const relative of ['package.json','electron/preload.js','electron/main.js','electron/UsbRzDeviceAction.js','electron/Protocol/protocol25.js','electron/Protocol/protocol25Const.js','node_modules/node-rz-hid/package.json','node_modules/node-rz-hid/nodehid.js']){
 const source=base+relative;if(!fs.existsSync(source))continue;
 const bytes=fs.readFileSync(source),raw=bytes.toString('utf8'),item={source,sha256:hash(bytes),bytes:bytes.length,receipts:[]};
 if(source.endsWith('.json'))item.data=JSON.parse(raw);
 else{
  const ast=acorn.parse(raw,{ecmaVersion:'latest'});
  walk(ast,n=>{
   let name=null;
   if(n.type==='MethodDefinition'&&relative!=='electron/main.js')name=n.key.name;
   if(n.type==='PropertyDefinition'&&relative!=='electron/main.js')name=n.key.name;
   if(n.type==='SwitchCase'&&['hid.sendFeatureReport','hid.sendFeatureReportMutex','hid.getFeatureReport','hid.acquireMutex','hid.releaseMutex'].includes(n.test?.value))name='case:'+n.test.value;
   if(n.type==='Property'&&n.key.name==='doRzDeviceAction')name='preload:doRzDeviceAction';
   if(relative==='electron/main.js'&&n.type==='CallExpression'&&n.arguments[0]?.value==='rzDeviceAction')name='ipc:rzDeviceAction';
   if(name)item.receipts.push({name,offset:n.start,end:n.end,source:raw.slice(n.start,n.end)});
  });
  if(relative!=='electron/main.js')item.complete_source=raw;
 }
 files.push(item);
}
const identity=files.find(x=>x.source.endsWith('/package.json')&&!x.source.includes('node_modules')).data;
if(identity.version!=='4.0.827')throw Error('Host version changed');
fs.writeFileSync('docs/re/gamepad-calibration-host-live-source.json',JSON.stringify({host_version:identity.version,parser:'Acorn static syntax only',offset_units:'UTF-16 code units',files},null,2)+'\n');
console.log(JSON.stringify({files:files.map(x=>({source:x.source,sha256:x.sha256,receipts:x.receipts.map(r=>({name:r.name,offset:r.offset,end:r.end}))}))}));
