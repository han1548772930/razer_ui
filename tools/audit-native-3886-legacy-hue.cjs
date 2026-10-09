// Static audit of product 3886's legacy monolithic Hue/native chain.
// Parses current source as data only; never evaluates middleware or loads a DLL.
const fs=require('fs'),path=require('path'),crypto=require('crypto'),acorn=require('./node_modules/acorn');
const root=path.resolve(__dirname,'..');
const rel='.ref/middleware/3886/main.df6f64c941b9efded61e.js';
const abs=path.join(root,rel),bytes=fs.readFileSync(abs),source=bytes.toString('utf8');
const sha=b=>crypto.createHash('sha256').update(b).digest('hex');
if(sha(bytes)!=='95bd47acbc00240cca06bc0f95e57bbfc5d2fa3427926a1e44969a489d3c3e5b')throw Error('Unexpected 3886 source');
const ast=acorn.parse(source,{ecmaVersion:'latest'}),nodes=[];
function walk(n,parent=null){if(!n?.type)return;nodes.push({n,parent});for(const v of Object.values(n)){if(Array.isArray(v))for(const c of v)walk(c,n);else if(v?.type)walk(v,n);}}
walk(ast);
const receipt=n=>n&&Number.isInteger(n.start)?({path:rel,sha256:sha(bytes),offset:n.start,end:n.end,source:source.slice(n.start,n.end)}):n;
const findOne=p=>{const a=nodes.filter(x=>p(x.n,x.parent));if(a.length!==1)throw Error('Expected one node, got '+a.length);return a[0].n;};
const findAny=p=>nodes.filter(x=>p(x.n,x.parent)).map(x=>x.n);
const classDX=findOne(n=>n.type==='ClassDeclaration'&&n.id?.name==='DX');
const classP2=findOne(n=>n.type==='ClassDeclaration'&&n.id?.name==='P2');
const classMethods=n=>n.body.body.filter(x=>x.type==='MethodDefinition').map(x=>({name:x.key.name||x.key.value,...receipt(x)}));
const assignmentReceipts=(cls,names)=>findAny((n,p)=>n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&n.left.object.type==='ThisExpression'&&names.includes(n.left.property.name||n.left.property.value)&&n.start>=cls.start&&n.end<=cls.end).map(receipt);
const dxAssignments=assignmentReceipts(classDX,['init','destroy','scanDevice','pair','unpair','abort','findBridge','getConnectedBridgeInfo','getEntertainmentGroupList','getSelectedEntertainmentGroup','selectEntertainmentGroup','getLightList','setBrightness','setCustomChromaFrame','acquireControl','releaseControl','getDllVersion','getControllerFilePath','getLightDevices','turnOffAllLights','callDLLApi','nodeFFICallback']);
const callApi=dxAssignments.find(x=>x.source.startsWith('this.callDLLApi='));
const init=dxAssignments.find(x=>x.source.startsWith('this.init='));
const destroy=dxAssignments.find(x=>x.source.startsWith('this.destroy='));
const mZ=findOne(n=>n.type==='VariableDeclarator'&&n.id?.name==='mZ');
const L2=findOne(n=>n.type==='VariableDeclarator'&&n.id?.name==='L2');
const T6=findOne(n=>n.type==='FunctionDeclaration'&&n.id?.name==='T6');
const C6=findOne(n=>n.type==='FunctionDeclaration'&&n.id?.name==='C6');
const p2Connect=assignmentReceipts(classP2,['connect'])[0];
const p2Methods=classMethods(classP2);
const hueInstantiation=findOne(n=>n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&n.left.object.type==='Identifier'&&n.left.object.name==='Is'&&n.left.property.name==='rzDevice'&&source.slice(n.start,n.end).includes('this.philipsHueMgr=new CX'));
const registerHue=findOne(n=>n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&n.left.object.type==='ThisExpression'&&n.left.property.name==='registerHueEvent');
const p2Destroy=findOne(n=>n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&n.left.object.type==='ThisExpression'&&n.left.property.name==='destroy'&&n.start>=classP2.start&&n.end<=classP2.end);
const c6HueCall=findOne(n=>n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&n.callee.object.type==='CallExpression'&&source.slice(n.start,n.end).includes('P2.getInstance().connect'));
const t6Calls=findAny(n=>n.type==='CallExpression'&&n.callee.type==='Identifier'&&n.callee.name==='T6'&&n.start>=C6.start&&n.end<=C6.end);
const mZParts={
 declaration:receipt(mZ),
 installedResourcesCalls:findAny(n=>n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&n.callee.property.name==='getItem'&&n.arguments[0]?.value==='installedResources'&&n.start>=mZ.start&&n.end<=mZ.end).map(receipt),
 matchPredicates:findAny(n=>n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&n.callee.property.name==='includes'&&n.start>=mZ.start&&n.end<=mZ.end).map(receipt),
 pathExpressions:findAny(n=>n.type==='TemplateLiteral'&&n.start>=mZ.start&&n.end<=mZ.end).map(receipt),
 loaderInit:findAny(n=>n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&n.callee.object.type==='MemberExpression'&&n.callee.object.object.name==='Is'&&n.callee.object.property.name==='rzDevice'&&n.callee.property.name==='init'&&n.start>=mZ.start&&n.end<=mZ.end).map(receipt),
 featureList:findAny(n=>n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&n.callee.property.name==='getFeatureParam'&&n.arguments[0]?.value==='hasDll'&&n.start>=mZ.start&&n.end<=mZ.end).map(receipt),
 callbackActions:findAny(n=>n.type==='Literal'&&typeof n.value==='string'&&['startCamera','setLevelMeterEnable'].includes(n.value)&&n.start>=mZ.start&&n.end<=mZ.end).map(receipt),
};
const apiActions=[];
for(const r of [init,...dxAssignments])if(r)for(const m of r.source.matchAll(/action:\"([^\"]+)\"/g))apiActions.push({action:m[1],owner:r.source.startsWith('this.init=')?'init':r.source.startsWith('this.')?r.source.slice(5,r.source.indexOf('=')):'unknown',receipt:r});
const initApiObject=init.source.match(/apiObj:\{[\s\S]*?\}\}\}/)?.[0]||'';
const abiEntries=[];
for(const m of initApiObject.matchAll(/([A-Za-z0-9_]+):\[\"([^\"]+)\",\[([^\]]*)\]\]/g))abiEntries.push({name:m[1],return_type:m[2],argument_types:m[3].split(',').filter(Boolean).map(x=>x.replaceAll('"','')),source:m[0]});
const observedEvents=['DETECTEVENT_HUE_DONE_COMPLETED','DETECTEVENT_HUE_DISCONNECTED','DETECTEVENT_HUE_CONNECTED','DETECTEVENT_HUE_LIGHTS_ON_LINE','DETECTEVENT_HUE_LIGHTS_OFF_LINE','DETECTEVENT_HUE_UPDATE_INFO','DETECTEVENT_HUE_GROUP_SELECTED_DONE','DETECTEVENT_HUE_BUSY_STREAMING'];
const eventReceipts=observedEvents.map(v=>({event:v,...receipt(findAny(n=>n.type==='Literal'&&n.value===v)[0])}));
const consumers=findAny(n=>n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&n.callee.object.type==='MemberExpression'&&n.callee.object.object.type==='ThisExpression'&&n.callee.object.property.name==='philipsHueMgr'&&n.start>=classP2.start&&n.end<=classP2.end).map(receipt);
const result={schema_version:1,date:'2026-10-09',generator_sha256:sha(fs.readFileSync(__filename)),scope:'Product 3886 current monolithic middleware Philips Hue manager; conditional caller/resource/ABI chain',method:'Acorn AST and byte/hash receipts only; no vendor JavaScript, application or DLL execution',source:receipt(ast),class:{name:'DX',...receipt(classDX),methods:classMethods(classDX)},manager:{name:'P2',...receipt(classP2),methods:p2Methods,connect_assignment:p2Connect,construct_and_runtime_assignment:receipt(hueInstantiation),register_event_assignment:receipt(registerHue),destroy_assignment:receipt(p2Destroy),connect_call_in_setup:receipt(c6HueCall)},loader:{mZ:mZParts,L2:receipt(L2),T6:receipt(T6),C6:receipt(C6),T6_calls_in_C6:t6Calls.map(receipt)},init:{assignment:receipt(init),destroy:receipt(destroy),call_dll_api:receipt(callApi),api_actions:apiActions,abi_entries:abiEntries},events:eventReceipts,consumer_call_receipts:consumers,summary:{dll_abi_exports:abiEntries.length,manager_methods:p2Methods.length,manager_consumers:consumers.length,observed_event_names:eventReceipts.length},boundaries:['P2.connect constructs the DX manager before T6; it does not itself call DX.init.','C6 only reaches registerHueEvent after T6 returns true. T6 invokes mZ only when injected hasDll is truthy; the current monolith does not contain a literal hasDll resource list.','mZ selects installedResources Common/Synapse entries by exact name and usedBy containing product id 3886, builds userDataDir/Apps/filePath, and passes that runtime-derived path to Is.rzDevice.init.','The fallback PhilipsHueNative.dll in DX.init is used only when its caller argument is empty; this source chain does not prove that fallback branch or a loaded DLL identity.','GetDLLVersion/SetNodeFFIEvent/Create follow ConfigureFFI only after a truthy bridge result; status and hardware success remain unobserved.','P2.destroy calls DX.destroy; source does not prove teardown timing or successful hardware state.']};
const target='docs/re/native-3886-legacy-hue-current-evidence.json',serialized=JSON.stringify(result,null,2)+'\n';
if(process.argv.includes('--check')){if(fs.readFileSync(path.join(root,target),'utf8')!==serialized)throw Error('Stale '+target)}else fs.writeFileSync(path.join(root,target),serialized);
console.log(JSON.stringify(result.summary));
