// Interpret literal AST data only; never execute downloaded product JavaScript.
const fs=require('fs'),path=require('path'),crypto=require('crypto'),acorn=require('acorn');
const root=path.resolve(__dirname,'..');
const read=p=>fs.readFileSync(path.join(root,p),'utf8');
const hash=s=>crypto.createHash('sha256').update(s).digest('hex');
const configs=JSON.parse(read('docs/re/source-product-configs.json')).products;
const traces=JSON.parse(read('docs/re/source-product-pages.json')).products;
const evidence=JSON.parse(read('docs/re/camera-product-evidence.json'));
if(evidence.parser_sha256!==hash(fs.readFileSync(path.join(__dirname,'extract-audio-evidence.cjs'))))throw Error('Regenerate camera evidence with current parser');
const output=[],receipts=[];
const walk=(n,fn)=>{if(!n?.type||fn(n)===false)return;for(const v of Object.values(n)){if(Array.isArray(v))v.forEach(x=>walk(x,fn));else if(v?.type)walk(v,fn);}};
for(const p of configs.filter(p=>p.family==='camera')) {
 const pid=p.product_id,config=p.config.exports,info=config.DeviceInfo;
 const trace=traces.find(p=>p.product_id===pid);
 const audited=evidence.products.find(p=>p.product_id===pid);
 for(const receipt of audited.source_files)if(hash(read(receipt.path))!==receipt.sha256)throw Error(`Changed ${receipt.path}`);
 const file=p.navigation[0].source,source=read(file);
 if(hash(source)!==p.navigation[0].sha256)throw Error(`Changed ${file}`);
 let textSource=source,textFile=file,h=/(?:[,{])4693:/.exec(textSource),module;
 if(h) {
  const expr=acorn.parseExpressionAt(textSource,h.index+h[0].length,{ecmaVersion:'latest'});
  module=expr.type==='SequenceExpression'?expr.expressions[0]:expr;
 } else {
  const tree=acorn.parse(source,{ecmaVersion:'latest'});
  walk(tree,n=>{if(n.type==='Property'&&(n.key.value??n.key.name)===4693&&/Function/.test(n.value.type)){module=n.value;return false;}});
 }
 if(!module)throw Error(`No localization module ${pid}`);
 const bindings=new Map(),exports=new Map();
 walk(module,n=>{if(n.type==='VariableDeclarator'&&n.id.type==='Identifier')bindings.set(n.id.name,n.init);if(n.type==='Property'&&n.value?.type==='ArrowFunctionExpression')exports.set(n.key.name??n.key.value,n.value.body.name);});
 function label(symbol) {const v=bindings.get(exports.get(symbol));if(v?.type!=='Literal'||typeof v.value!=='string')throw Error(`No label ${pid}:${symbol}`);return v.value;}
 const result={product_id:pid,profile:structuredClone(config.DEFAULTPROFILE),pages:[],support:info.supportPage};
 const page=key=>{let pg=result.pages.find(p=>p.key===key);if(!pg){pg={key,sections:[]};result.pages.push(pg);}return pg;};
 const section=(pg,title)=>{const s={title,controls:[]};page(pg).sections.push(s);return s;};
 function control(s,kind,key,text,path,extra={}) {const c={kind,key:`${pid}:${key}`,label:text,path,...extra};s.controls.push(c);return c;}
 const slider=(s,key,text,path,min,max,step=1,extra={})=>control(s,'slider',key,text,path,{min,max,step,...extra});
 const toggle=(s,key,text,path,extra={})=>control(s,key==='auto-white-balance'?'toggle':'switch',key,text,path,extra);
 const select=(s,key,text,path,options)=>control(s,'options',key,text,path,{options:options.map(([label,value])=>({label,value}))});
 const legacy=pid===3587||pid===3589||pid===3590;
 const imagePage=legacy?'TAB_CUSTOMIZE':'IMAGE';
 const image=section(imagePage,label('W1Z'));
 const profileImages=result.profile.image;
 const mode=result.profile.imageMode!==undefined?'imageMode':'imageSettingMode';
 // Preset order and names are exact DEFAULTPROFILE array entries.
 select(image,'image-preset',label(legacy?'W1Z':'JGr'),`/${mode}`,profileImages.map((p,i)=>[p.name.toUpperCase(),i]));
 for(const [field,symbol] of [['brightness','a1g'],['contrast','DkT'],['saturation','Bq1']]) {
  const nodes=trace.pages.find(p=>p.key===imagePage).components.flatMap(c=>c.jsx).filter(j=>j.props.min==='0'&&j.props.max==='255'&&j.props.step==='1');
  if(nodes.length<3)throw Error(`Missing image range proof ${pid}`);
  slider(image,field,label(symbol),`@image/${field}`,0,255,1);
 }
 toggle(image,'auto-white-balance',label('Q8g'),'@image/whiteBalance/isAuto');
 const wb=legacy?info.whiteBalanceValue:info.whiteBalance;
 slider(image,'white-balance',label('Z8r'),'@image/whiteBalance/value',wb.min,wb.max,legacy?wb.step:10,{disabled_when:'@image/whiteBalance/isAuto'});
 if(legacy) {
  const nav=p.navigation.flatMap(n=>n.items).find(i=>i.name.value==='TAB_CUSTOMIZE');
  if(nav.component.includes('supportsFocus:!0')) {
   const focus=section('TAB_CUSTOMIZE',label('dgn'));
   toggle(focus,'autofocus',label('dgn'),'/camera/cameraMode',{options:[{label:'MANUAL',value:'manual'},{label:'AUTO',value:'auto'}]});
   // YS oe: min:0,max:DeviceInfo.maxCameraFocus||255,step:1.
   if(!source.includes('maxCameraFocus||255'))throw Error(`Missing focus range ${pid}`);
   slider(focus,'focus',label('dgn'),'/camera/cameraValue',0,info.maxCameraFocus,1,{visible_when:{path:'/camera/cameraMode',value:'manual'}});
  }
  if(nav.component.includes('supportsHDR:!0')) {
   const hdr=section('TAB_CUSTOMIZE',label('vfE'));
   toggle(hdr,'hdr',label('vfE'),'/camera/hdrActive');
   select(hdr,'fov',label('tTN'),'/camera/fieldOfViewActive',[[label('kUF'),2],[label('gnd'),1],[label('eV0'),0]]);
  }
 } else {
  const cameraPage=trace.pages.find(p=>p.key==='CAMERA');
  const raw=cameraPage.components.map(c=>c.source).join('\n');
  const focus=section('CAMERA',label('dgn'));
  toggle(focus,'autofocus',label('dgn'),'/camera/autoFocus/isEnabled');
  // Only the Ultra's literal 1..450 slider is independently resolved here.
  if(pid===3592&&raw.includes('min:1,max:450,step:1')) slider(focus,'focus',label('dgn'),'/camera/autoFocus/manualFocus',1,450,1,{visible_when:{path:'/camera/autoFocus/isEnabled',value:false}});
  const processing=trace.pages.find(p=>p.key==='PROCESSING');
  const body=processing.components.map(c=>c.source).join('\n');
  // These are page-mounted switches; DEFAULTPROFILE alone is insufficient.
  for(const [field,symbol] of [['ldc','_Kh'],['lowLightCompensation','RAi']]) {
   if(body.includes(`name:`)&&body.includes(`.${symbol}`)) toggle(section('PROCESSING',label(symbol)),field,label(symbol),`/camera/${field}`);
  }
  const noise=section('PROCESSING',label('VMM'));
  for(const [field,symbol] of [['state2D','XXG'],['state3D','rI2']]) {
   if(body.includes(`.${symbol}`))toggle(noise,field,label(symbol),`/camera/dynamicNoiseReduction/${field}`);
  }
  if(!noise.controls.length)page('PROCESSING').sections.pop();
  const mirror=section('IMAGE',label('S7W'));
  toggle(mirror,'mirror',label('S7W'),'/camera/mirrorVideo');
  // Watermark is explicitly disabled by the other three product roots.
  const nav=p.navigation.flatMap(n=>n.items).find(i=>i.name.value==='IMAGE');
  if(!nav.component.includes('supportWatermark:!1'))toggle(section('IMAGE',label('Tjs')),'watermark',label('Tjs'),'/camera/watermark/isEnabled');
 }
 if(legacy) {
  const pg=result.pages.find(p=>p.key==='TAB_CUSTOMIZE');
  const image=pg.sections.shift(); pg.sections.push(image);
 } else {
  const mounted=key=>audited.pages.find(p=>p.key===key)?.components??[];
  const sourceFor=(key,field)=>mounted(key).find(c=>c.source.includes(`advancedWebcamReducer.${field}`)&&c.source.includes('children:'));
  const receiptControl=(s,kind,key,title,path,proof,extra={})=>{
   if(!proof)throw Error(`Missing camera proof ${pid}:${key}`);
   return control(s,kind,key,title,path,{...extra,source:{path:proof.path,offset:proof.offset,end:proof.end}});
  };
  const optionsFor=(component,predicate)=>audited.arrays.find(a=>a.value.every(v=>v&&typeof v==='object'&&Object.hasOwn(v,'value'))&&predicate(a.value)&&component.source.includes(`${a.symbol}.map`))?.value;
  const addOptions=(s,key,title,path,proof,options,extra={})=>{
   if(!options?.length)return;
   return receiptControl(s,'options',key,title,path,proof,{options:options.map(o=>({label:String(o.name),value:o.value})),...extra});
  };
  const focus=page('CAMERA').sections.find(s=>s.title===label('dgn'));
  // Rebuild manual focus using the mounted component's explicit overrides.
  focus.controls=focus.controls.filter(c=>!c.key.endsWith(':focus'));
  const focusProof=sourceFor('CAMERA','autoFocus');
  const mount=mounted('CAMERA').flatMap(c=>c.jsx).find(j=>Object.hasOwn(j.props,'supportMode')||Object.hasOwn(j.props,'manualFocusMin'));
  const min=mount?.props.manualFocusMin??1,max=mount?.props.manualFocusMax??450;
  if(pid!==3592&&!source.includes('manualFocusMin:1,manualFocusMax:450'))throw Error(`Missing focus defaults ${pid}`);
  receiptControl(focus,'slider','focus',label('EzE'),'/camera/autoFocus/manualFocus',focusProof,{min,max,step:1,visible_when:{path:'/camera/autoFocus/isEnabled',value:false}});
  const whenAuto={visible_when:{path:'/camera/autoFocus/isEnabled',value:true}};
  if(pid===3592||mount?.props.supportMode===true)addOptions(focus,'focus-mode',label('gah'),'/camera/autoFocus/mode',focusProof,optionsFor(focusProof,a=>a.some(x=>x.name==='STANDARD')&&a.some(x=>x.name==='FACE')),whenAuto);
  if(pid===3592||mount?.props.supporttracking===true)addOptions(focus,'focus-tracking',label('eWS'),'/camera/autoFocus/tracking',focusProof,optionsFor(focusProof,a=>a.some(x=>x.name==='PASSIVE')),whenAuto);
  if(pid===3592||mount?.props.supportLighting===true)addOptions(focus,'focus-lighting',label('Tf7'),'/camera/autoFocus/lightingType',focusProof,optionsFor(focusProof,a=>a.some(x=>x.name==='STYLIZED')));
  const exposureProof=sourceFor('CAMERA','autoExposure');
  const exposure=section('CAMERA',label('aDz'));
  receiptControl(exposure,'switch','auto-exposure',label('aDz'),'/camera/autoExposure/isEnabled',exposureProof);
  const whenManual={visible_when:{path:'/camera/autoExposure/isEnabled',value:false}};
  if(info.exposure && exposureProof.source.includes('DeviceInfo.exposure.min')) {
   receiptControl(exposure,'slider','manual-exposure',label('aDz'),'/camera/autoExposure/exposureValue',exposureProof,{min:info.exposure.min,max:info.exposure.max,step:1,...whenManual});
  } else {
   addOptions(exposure,'iso','ISO','/camera/autoExposure/isoSensitivity',exposureProof,optionsFor(exposureProof,a=>a.some(x=>x.name===6400)),whenManual);
   const shutter=audited.arrays.find(a=>a.value.some(v=>v?.name==='1/2000')&&exposureProof.source.includes(`originData:${a.symbol}`));
   if(shutter)receiptControl(exposure,'select','shutter',label('SJ$'),'/camera/autoExposure/shutterSpeed',exposureProof,{options:shutter.value.filter(v=>v.value>=info.shutterSpeed.min&&v.value<=info.shutterSpeed.max).map(v=>({label:v.name,value:v.value})),...whenManual});
   const meter=optionsFor(exposureProof,a=>a.some(x=>x.name==='AVERAGE'));
   addOptions(exposure,'metering',label('cSt'),'/camera/autoExposure/metering',exposureProof,meter,{visible_when:{path:'/camera/autoExposure/isEnabled',value:true}});
   receiptControl(exposure,'slider','compensation',label('F9S'),'/camera/autoExposure/compensation',exposureProof,{min:-3,max:3,step:.1,visible_when:{path:'/camera/autoExposure/isEnabled',value:true},minimum_when:{path:'/camera/autoExposure/metering',value:5,min:-1}});
   receiptControl(exposure,'reset','reset-exposure','RESET','/camera/autoExposure',exposureProof,{reset_value:result.profile.camera.autoExposure});
  }
  const processingProof=sourceFor('PROCESSING','mjpegQuality');
  if(processingProof) {
   const quality=section('PROCESSING',label(pid===3592?'L7_':'G4E'));
   receiptControl(quality,'switch','auto-quality',label('ezb'),'/camera/mjpegQuality/autoQuality',processingProof);
   if(!source.includes('PERFORMANCE:5,QUALITY:7'))throw Error(`Missing quality enum ${pid}`);
   addOptions(quality,'quality-mode',label(pid===3592?'L7_':'G4E'),'/camera/mjpegQuality/mode',processingProof,[{name:label('_TM'),value:5},{name:label('kuU'),value:7}],{visible_when:{path:'/camera/mjpegQuality/autoQuality',value:true}});
   receiptControl(quality,'slider','manual-quality',label(pid===3592?'L7_':'G4E'),'/camera/mjpegQuality/manualQuality',processingProof,{min:10,max:90,step:1,visible_when:{path:'/camera/mjpegQuality/autoQuality',value:false}});
   const hdr=section('PROCESSING','HDR');
   receiptControl(hdr,'switch','hdr','HDR','/camera/hdr/isEnabled',processingProof);
   if(!source.includes('DARK_MODE:0,BRIGHT_MODE:1'))throw Error(`Missing HDR enum ${pid}`);
   addOptions(hdr,'hdr-mode','HDR','/camera/hdr/mode',processingProof,[{name:label('$3b'),value:0},{name:label('pG'),value:1}],{disabled_unless:'/camera/hdr/isEnabled'});
  }
  const imageProof=sourceFor('IMAGE','antiFlicker');
  const flicker=optionsFor(imageProof,a=>a.some(x=>x.name==='50 hz'));
  addOptions(section('IMAGE',label('qJr')),'anti-flicker',label('qJr'),'/camera/antiFlicker',imageProof,flicker);
  const imageControls=sourceFor('IMAGE','imagePreset');
  for(const [field,symbol] of [['sharpness','UFE'],['gain','cSw']]) {
   if(result.profile.image.every(p=>Object.hasOwn(p.dataSet,field)))receiptControl(image,'slider',field,label(symbol),`@image/${field}`,imageControls,{min:0,max:255,step:1});
  }
  if(pid===3595) {
   const micProof=mounted('MIC').find(c=>c.source.includes('adaptiveNoiseReduction'));
   if(!source.includes('adaptiveNoiseReduction:!1,crystalClearVoice:!1'))throw Error('Missing independent Mic defaults');
   // Independent device state, outside DEFAULTPROFILE.camera, retained locally.
   result.profile.deviceMic={adaptiveNoiseReduction:false,crystalClearVoice:false};
   result.device_fields=['deviceMic'];
   for(const [field,symbol,description] of [['adaptiveNoiseReduction','Er1','CWO'],['crystalClearVoice','kuq','an_']]) {
    const s=section('MIC',label(symbol));s.description=label(description);
    receiptControl(s,'switch',field,label(symbol),`/deviceMic/${field}`,micProof);
   }
  }
 }
 output.push(result);
 receipts.push({product_id:pid,source:file,sha256:hash(source),localization_source:textFile,localization_sha256:hash(textSource),config_source:p.config.path,config_offset:p.config.offset,pages:result.pages.map(pg=>({key:pg.key,control_keys:pg.sections.flatMap(s=>s.controls.map(c=>c.key)),source_navigation_offset:p.navigation.flatMap(n=>n.items).find(i=>i.name.value===pg.key)?.offset})),limitations:['Live video, camera enumeration, view presets/framing, source overlays and device commands remain incomplete. Mic controls retain independent local device state; no firmware/reboot or hardware success is synthesized.']});
}
fs.writeFileSync(path.join(root,'src/features/source_controls_data.json'),JSON.stringify(output,null,2)+'\n');
fs.writeFileSync(path.join(root,'docs/re/camera-controls-source.json'),JSON.stringify({schema_version:1,scanner_sha256:hash(fs.readFileSync(__filename)),products:receipts},null,2)+'\n');
console.log(`Generated ${output.length} source-specific camera control descriptors.`);
