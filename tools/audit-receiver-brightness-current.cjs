// Static current-source inspection only; never evaluate vendor JavaScript.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {CurrentMiddlewareSource}=require('./current-middleware-source.cjs');
const {walk,key,hash}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..'),read=p=>fs.readFileSync(path.join(root,p),'utf8');
const receipts=[],products=[];
function record(s,id,name,node){receipts.push({product_id:s.product,module:id,name,...s.receipt(id,node)});}
function exported(s,id,name){let n=s.exported(id,name);while(n.type==='Identifier')n=s.binding(id,n.name);return n;}
function command(s,name){let n=exported(s,78196,name);record(s,78196,'command-'+name,n);
 if(n.type==='SequenceExpression')n=n.expressions.at(-1);
 if(n.type!=='NewExpression'||n.callee.name!=='Uint8Array'||n.arguments.length!==1)throw Error('Command representation changed');
 return s.literal(78196,n.arguments[0]);}
for(const product of [164,241]){
 const s=new CurrentMiddlewareSource(product);s.parse(s.chunkFile(6120));
 const main=s.text(s.mainFile),registrations=[];
 walk(acorn.parse(main,{ecmaVersion:'latest'}),node=>{
  if(node.type==='CallExpression'&&node.callee.type==='MemberExpression'&&key(node.callee.property)==='useFeature'&&node.arguments[0]?.value==='rzDeviceType')registrations.push(node);
 });
 if(registrations.length!==1)throw Error('Ambiguous active rzDeviceType');
 const registration=registrations[0],cap=s.literal(0,registration.arguments[2]);
 if(registration.arguments[1]?.value!=='rzDevice25'||cap.brightness.regionId!==15||cap.brightness.setByLightingEngine||cap.duallinkDeviceType!==(product===164?'LINKER':'LINKERMULTIDEVICES'))throw Error('Active brightness capability changed');
 receipts.push({product_id:product,name:'active-device-type-registration',path:s.mainFile,sha256:hash(main),offset:registration.start,end:registration.end,source:main.slice(registration.start,registration.end)});
 const cls=exported(s,7755,'default'),methods=[];
 walk(cls,node=>{if(node.type==='MethodDefinition'&&['getBrightness','setBrightness','_createDataSend','_calculateChecksum','_getUSBTransferInResult'].includes(key(node.key))){record(s,7755,key(node.key),node);if(['getBrightness','setBrightness'].includes(key(node.key)))methods.push(key(node.key));}});
 if(methods.length!==2)throw Error('Brightness device methods changed');
 for(const name of ['Fm','n4'])record(s,13953,'brightness-helper-'+name,exported(s,13953,name));
 const parser=s.binding(13953,'T');record(s,13953,'brightness-parser',parser);
 if(!s.snippet(13953,parser).includes('Math.ceil(e.data[2]/255*100)')||!s.snippet(13953,exported(s,13953,'n4')).includes('Math.floor(r/100*255)'))throw Error('Brightness quantization changed');
 const get=command(s,'_9'),set=command(s,'ne');
 if(JSON.stringify(get)!=='[3,15,132]'||JSON.stringify(set)!=='[3,15,4]')throw Error('Brightness commands changed');
 const task=exported(s,96571,'Vb7');record(s,96571,'taskRunnerSetBrightnessToDevice',task);
 const taskText=s.snippet(96571,task);
 if(!taskText.includes('profileId:1')||!taskText.includes('setByLightingEngine')||!taskText.includes('getBrightness'))throw Error('Brightness task branching changed');
 for(const name of ['jK','C9','yC','SA'])record(s,89788,'special-device-predicate-'+name,exported(s,89788,name));
 // Use separate readers: sibling chunks may reuse module numbers.
 const primary=new CurrentMiddlewareSource(product),file=primary.files.find(file=>file.includes('rzDevice25Linker.'));
 if(!file)throw Error('Missing primary Linker');primary.parse(file);
 let transactionCount=0;
 for(const[id,module]of primary.modules)walk(module.fn,node=>{
  if(node.type==='MethodDefinition'&&['_getTransactionId','sendCommand'].includes(key(node.key))){record(primary,id,'primary-Linker-'+key(node.key),node);if(key(node.key)==='_getTransactionId'){
   const body=primary.snippet(id,node);
   if(!body.includes('31===this.transactionId')||!body.includes('224|this.transactionId++'))throw Error('Primary Linker transaction namespace changed');transactionCount++;
  }}
 });
 if(transactionCount!==1)throw Error('Primary transaction implementation changed');
 if(product===241){const multi=new CurrentMiddlewareSource(product),file=multi.files.find(file=>file.includes('rzDevice25LinkerMultiDevices.'));multi.parse(file);
  let count=0;for(const[id,module]of multi.modules)walk(module.fn,node=>{if(node.type==='ClassDeclaration'&&multi.snippet(id,node).includes('this.name="rzDevice25LinkerMultiDevices"')){record(multi,id,'primary-MultiDevices-inheritance',node);count++;}});
  if(count!==1)throw Error('MultiDevices inheritance changed');
  for(const receipt of multi.acquisition)if(!s.acquisition.some(r=>r.path===receipt.path))s.acquisition.push(receipt);
 }
 for(const receipt of primary.acquisition)if(!s.acquisition.some(r=>r.path===receipt.path))s.acquisition.push(receipt);
 products.push({product_id:product,profile_id:1,region_id:15,get_command:get,set_command:set,
  transaction_prefix:224,transaction_modulus:31,pre_read_before_write:true,setter_skipped_if_same_percent:true,
  percentage_to_byte:'floor(percent / 100 * 255)',byte_to_percentage:'ceil(raw / 255 * 100)',acquisition:s.acquisition});
}
const implementation_receipts=['crates/razer-device/src/receiver_brightness.rs','crates/razer-service/src/runtime/windows/receiver_brightness.rs'].map(file=>({path:file,sha256:hash(fs.readFileSync(path.join(root,file)))}));
const evidence={schema_version:1,generator_sha256:hash(read('tools/audit-receiver-brightness-current.cjs')),
 offset_unit:'UTF-16 code units; end exclusive; SHA-256 covers actual UTF-8 source bytes',products,receipts,implementation_receipts,
 semantics:['Normal 164/241 TaskRunner uses profile 1, active receiver brightness region 15 and the primary Linker E0 transaction namespace. Pairing lane transaction namespaces must not be reused.',
  'TaskRunner performs a real getBrightness before normal writes and skips setBrightness if the source-rounded percentage already equals the request.',
  'Both source getter and setter parse three response bytes: profile, region and raw brightness. Conversion uses ceil on reads and floor on writes.',
  'Source command/transaction mismatches and statuses 0/3/4 retry OUT, busy status 1 retries IN, while unsupported/unknown status and transport exceptions stop the operation. Rust does not resend an exception after a potentially delivered write.',
  'Current TaskRunner performs one pre-read and a conditional setter, without an additional post-write getter or comparison. Source completion is not a separate hardware readback.',
  'Transport receipts preserve attempted/sent state and actual setter response separately from errors; no failed send is treated as an acknowledgment or rollback.',
  'Brightness completion alone does not complete Help Reset: original effect, mapping, storage publication and memory-cache chains must be tracked independently.'],
 gaps:['Original TaskRunner host brightness publication and memory storage refresh are not implemented by this protocol helper.',
  'Other products and special lighting-engine, monitor, IoT, Philips Hue or eGPU branches are not implemented by this receiver-specific helper.',
  'Runtime acceptance has not been performed: no application, device operation, vendor DLL or vendor JavaScript was executed.']};
const target='docs/re/receiver-brightness-current-evidence.json',output=JSON.stringify(evidence,null,2)+'\n';
if(process.argv.includes('--check')){if(read(target)!==output)throw Error('Stale '+target);}else fs.writeFileSync(path.join(root,target),output);
console.log(JSON.stringify({products:products.length,receipts:receipts.length,implementation_receipts:implementation_receipts.length}));
