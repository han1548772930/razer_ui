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
  // The four current roots mount one 400px `.camera-container` column; the
  // three earlier roots keep the shared Customize layout.
  if(!legacy)result.layout='camera';
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
  // 原版的图像参数行同样由共享设置行渲染：`hasStepper:!0` 且步长写在行上
  // （亮度/对比度/饱和度 `stepValue:1`，白平衡 `stepValue:10`）。逐行取证，
  // 缺一就报错，避免把没有步进器的行也画上步进器。行字面量在挂载组件里，
  // 取不到时退回整包文本（`advancedWebcamReducer.<field>` 未必出现在行附近）。
  const rowLiteral=symbol=>{
   // 白平衡行的 `hasStepper` 在 500 字符之后，因此窗口放宽到 900；用负向前瞻
   // 确保没有跨进下一行（下一行一定以 `name:` 开头），避免把邻居的步进器算进来。
   const row=new RegExp(`name:[\\w$]+\\.${symbol},(?:(?!name:)[\\s\\S]){0,900}?hasStepper:!0`).exec(source);
   return row&&row[0];
  };
  for(const [field,symbol] of [['brightness','a1g'],['contrast','DkT'],['saturation','Bq1'],['white-balance','Z8r']]) {
   const rowControl=image.controls.find(c=>c.key===`${pid}:${field}`);
   if(!rowControl)throw Error(`Missing image row ${pid}:${field}`);
   const row=rowLiteral(symbol);
   if(!row||!row.includes('hasStepper:!0'))throw Error(`Missing row stepper ${pid}:${symbol}`);
   if(!/stepValue:([\d.]+)/.test(row))throw Error(`Missing row stepper step ${pid}:${symbol}`);
   Object.assign(rowControl,{has_stepper:true,allow_decimal:false,round_up_decimals:false});
  }
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
  // Framing block: every current CAMERA page mounts zoom, the pan/tilt pad,
  // the five view presets and the preset shortcut row ahead of auto-focus.
  const camMounted=mounted('CAMERA');
  const framingProof=camMounted.find(c=>c.source.includes('"preset-container"')&&c.source.includes('viewPresets'));
  const padProof=camMounted.find(c=>c.source.includes('pan-and-tilt-container'));
  const listenProof=camMounted.find(c=>c.source.includes('keyboard_listen'));
  if(!framingProof||!padProof||!listenProof)throw Error(`Missing framing mounts ${pid}`);
  const symbol=(re,text,what)=>{const m=re.exec(text);if(!m)throw Error(`Missing ${what} ${pid}`);return m[1];};
  const zoomNames=/name:(?:[A-Za-z_$][\w$]*)\.([\w$]+),tooltipContent:(?:[A-Za-z_$][\w$]*)\.([\w$]+)/.exec(framingProof.source);
  if(!zoomNames)throw Error(`Missing zoom name ${pid}`);
  const zoomName=zoomNames[1],zoomTooltip=zoomNames[2];
  // The same label is reached through the translator call or the text element,
  // depending on the product root, so both mounted shapes are accepted.
  const elementText=(className,text)=>{
   const re=new RegExp(`className:"${className}",children:\\(0,[\\w$.]+\\.(?:JN\\)\\(?|jsx\\)\\([\\w$]+\\.A,\\{text:)[A-Za-z_$][\\w$]*\\.([\\w$]+)\\}?\\)`);
   const m=re.exec(text);
   if(!m)throw Error(`Missing ${className} ${pid}`);
   return m[1];
  };
  const modeDescription=elementText('mode-description',framingProof.source);
  const presetName=elementText('preset-name',framingProof.source);
  const shortcutName=elementText('shortcut-key-name',framingProof.source);
  const padName=elementText('pan-and-tilt-name',padProof.source);
  const centerTooltip=symbol(/tooltip:\(0,[\w$.]+\.JN\)\(?[A-Za-z_$][\w$]*\.([\w$]+)[^)]*\)/,padProof.source,'pan and tilt centre tooltip');
  const shortcutIdle=symbol(/style:\{color:"#707070",textTransform:"none"\},children:\(0,[\w$.]+\.jsx\)\((?:[\w$]+)\.A,\{text:[A-Za-z_$][\w$]*\.([\w$]+)\}\)/,listenProof.source,'view preset shortcut placeholder');
  const mountProof=camMounted.find(c=>c.source.includes('camera-container')&&c.source.includes('min_Zoom_Value'));
  const zoomProps=mountProof&&/min_Zoom_Value:([\d.]+),max_Zoom_Value:([\d.]+),zoom_Step_Value:([\d.]+)/.exec(mountProof.source);
  // The Ultra's stepper carries its own literal bounds instead of mount props.
  const stepper=/stepValue:([\d.]+),minStepper:([\d.]+),maxStepper:([\d.]+)/.exec(framingProof.source);
  const zoom=zoomProps?{min:+zoomProps[1],max:+zoomProps[2],step:+zoomProps[3]}:stepper?{min:+stepper[2],max:+stepper[3],step:+stepper[1]}:null;
  if(!zoom)throw Error(`Missing zoom range ${pid}`);
  const panProp=mountProof&&/maxPanTilt:([\d.]+)/.exec(mountProof.source);
  // The pad's own default is 10 when no mount prop overrides it.
  if(!panProp&&!padProof.source.includes('?10:a'))throw Error(`Missing pan/tilt bound ${pid}`);
  const maxPanTilt=panProp?+panProp[1]:10;
  const presets=result.profile.camera.viewPresets.data;
  if(presets.length<2)throw Error(`Missing view presets ${pid}`);
  const box=/\{width:"".concat\((\w+),"px"\),height:"".concat\((\w+),"px"\),borderWidth:/.exec(padProof.source);
  if(!box)throw Error(`Missing pan/tilt box ${pid}`);
  // A minified name can be reused in another scope; require one distinct value.
  const boxLiteral=name=>{
   const values=new Set([...source.matchAll(new RegExp(`[,;({]\\s*${name}\\s*=\\s*(\\d+(?:\\.\\d+)?)`,'g'))].map(m=>m[1]));
   if(values.size!==1)throw Error(`Ambiguous pan/tilt box ${pid}:${name}`);
   return +[...values][0];
  };
  const boxWidth=boxLiteral(box[1]),boxHeight=boxLiteral(box[2]);
  // 复合禁用条件：`ldc && (4K 30FPS | 1440p 30FPS)`。同一个条件函数同时接到
  // 变焦步进器（disabledStepper）、变焦滑块（active）、平移/倾斜面板（isDisabled）
  // 与预设块（含快捷键）的 `.disabled` 外壳上。
  const gateProof=/!([\w$]+)\|\|"([^"]+)"!==[\w$]+\.name&&"([^"]+)"!==[\w$]+\.name\|\|\([\w$]+=!0\)/.exec(framingProof.source);
  if(!gateProof)throw Error(`Missing framing gate ${pid}`);
  // The selector list also holds viewPresets/resolution, so match the LDC field
  // itself; `shallowEqual` is only passed for object selectors.
  const ldcSelector=new RegExp(`([\\w$]+)=\\(0,[\\w$.]+\\.useSelector\\)\\([\\w$]+=>[\\w$]+\\.advancedWebcamReducer\\.ldc[,)]`).exec(framingProof.source);
  if(!ldcSelector||ldcSelector[1]!==gateProof[1])throw Error(`Missing framing gate state ${pid}`);
  const gatedResolutions=[gateProof[2],gateProof[3]].map(name=>{
   const pattern=new RegExp(`\\{name:"${name.replace(/[.*+?^${}()|[\]\\]/g,'\\$&')}",width:(\\d+),height:(\\d+),fps:(\\d+)\\}`, 'g');
   // A minified bundle keeps one list per family; require a single distinct record.
   const values=new Set([...source.matchAll(pattern)].map(m=>`${m[1]},${m[2]},${m[3]}`));
   if(values.size!==1)throw Error(`Ambiguous gated resolution ${pid}:${name}`);
   const [width,height,fps]=[...values][0].split(',').map(Number);
   return {width,height,fps};
  });
  const gate=gatedResolutions.map(value=>[{path:'/camera/ldc',value:true},{path:'/camera/resolution',value}]);
  // 变焦行由共享设置行渲染：`HM.A` + `hasStepper:!0`，同时带
  // `allowDecimal:!0,roundUpDecimals:!0`（缩放显示一位小数）与 `disabledStepper`
  // （同一道 LDC 复合条件）。步进器本身的步长/上下限用 `stepValue/minStepper/maxStepper`。
  const zoomStepper=/disabledStepper:[\w$]+\(\),name:[\w$]+\.[\w$]+,tooltipContent:[\w$]+\.[\w$]+,stepValue:([\w$.]+),minStepper:([\w$.]+),maxStepper:([\w$.]+),stepperValue:[\w$]+\.zoom,handleStepperValue:[\w$]+,hasStepper:!0/
   .exec(framingProof.source);
  if(!zoomStepper)throw Error(`Missing zoom stepper row ${pid}`);
  const zoomStepperFlags={has_stepper:true,
   allow_decimal:framingProof.source.includes('roundUpDecimals:!0')&&framingProof.source.includes('allowDecimal:!0'),
   round_up_decimals:framingProof.source.includes('roundUpDecimals:!0'),
   stepper_receipt:{path:framingProof.path,offset:framingProof.offset+zoomStepper.index,end:framingProof.offset+zoomStepper.index+zoomStepper[0].length}};
  const framing={title:label(zoomName),controls:[]};
  slider(framing,'zoom',label(zoomName),'@view/zoom',zoom.min,zoom.max,zoom.step,{tooltip:label(zoomTooltip),source:{path:framingProof.path,offset:framingProof.offset,end:framingProof.end},disabled_when_any:gate,...zoomStepperFlags});
  receiptControl(framing,'pan_tilt','pan-tilt',label(padName),'@view/pan',padProof,{tilt_path:'@view/tilt',max_pan_tilt:maxPanTilt,box_width:boxWidth,box_height:boxHeight,tooltip:label(centerTooltip),disabled_when_any:gate});
  receiptControl(framing,'preset','view-preset',label(presetName),'/camera/viewPresets/viewMode',framingProof,{options:presets.map(p=>({label:String(p.mode),value:p.mode})),description:label(modeDescription),disabled_when_any:gate});
  receiptControl(framing,'keys','shortcut-key',label(shortcutName),'@view/shortcutKey',listenProof,{placeholder:label(shortcutIdle),disabled_when_any:gate});
  page('CAMERA').sections.unshift(framing);
  // Watermark placement pad, mounted inside the same setting row as the switch.
  const imageBlock=mounted('IMAGE').find(c=>c.source.includes('direction-container'));
  if(imageBlock) {
   const arraySymbol=/className:"direction-container[^"]*"[^,]*?,children:([\w$]+)\.map\(/.exec(imageBlock.source);
   if(!arraySymbol)throw Error(`Missing direction array ${pid}`);
   const list=new RegExp(`[,;]\\s*${arraySymbol[1]}\\s*=\\s*\\[([^\\]]*)\\]`).exec(source);
   if(!list)throw Error(`Missing direction list ${pid}`);
   // Watermark placements are exported string constants rather than i18n
   // symbols. Resolve `SYM:()=>local` through the module export map, then read
   // the local's static string inside that same module function.
   const quote=name=>name.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
   // Literal-only folding; downloaded JavaScript is never executed.
   const staticString=node=>{
    if(!node)return null;
    if(node.type==='Literal')return typeof node.value==='string'?node.value:null;
    if(node.type==='ParenthesizedExpression')return staticString(node.expression);
    if(node.type==='SequenceExpression')return staticString(node.expressions[node.expressions.length-1]);
    if(node.type==='ConditionalExpression'&&node.test?.type==='Literal')return staticString(node.test.value?node.consequent:node.alternate);
    return null;
   };
   const tree=acorn.parse(source,{ecmaVersion:'latest'});
   const exportedString=symbol=>{
    const map=new RegExp(`(?:[,{])\\s*${quote(symbol)}\\s*:\\s*\\(\\)\\s*=>\\s*([\\w$]+)`).exec(source);
    if(!map)throw Error(`No export ${pid}:${symbol}`);
    const local=map[1];
    let scope=null;
    walk(tree,n=>{
     if(typeof n.start!=='number'||!/Function/.test(n.type))return;
     if(n.start<=map.index&&map.index<n.end&&(!scope||n.end-n.start<scope.end-scope.start))scope=n;
    });
    if(!scope)throw Error(`No module ${pid}:${symbol}`);
    const values=new Set();
    walk(scope,n=>{
     const value=n.type==='VariableDeclarator'&&n.id?.type==='Identifier'&&n.id.name===local?staticString(n.init)
      :n.type==='AssignmentExpression'&&n.left?.type==='Identifier'&&n.left.name===local?staticString(n.right):null;
     if(typeof value==='string')values.add(value);
    });
    if(values.size!==1)throw Error(`Ambiguous literal ${pid}:${symbol}`);
    return [...values][0];
   };
   const symbols=[...list[1].matchAll(/\{name:[A-Za-z_$][\w$]*\.([\w$]+)\}/g)].map(m=>m[1]);
   const positions=symbols.map(exportedString);
   if(new Set(positions).size!==positions.length)throw Error(`Duplicate watermark positions ${pid}`);
   if(!positions.every(p=>/^(left|center|right)-(top|bottom)$/.test(p)))throw Error(`Unexpected watermark positions ${pid}: ${positions}`);
   if(positions.length<2)throw Error(`Missing watermark positions ${pid}`);
   // Three roots mount the watermark component but pass `supportWatermark:!1`,
   // so the pad is only generated where the switch itself is mounted.
   const watermark=page('IMAGE').sections.find(s=>s.title===label('Tjs'));
   if(watermark) {
    const at=imageBlock.source.indexOf('mode-description');
    const description=/text:[A-Za-z_$][\w$]*\.([\w$]+)/.exec(imageBlock.source.slice(at,at+240));
    if(!description)throw Error(`Missing watermark description ${pid}`);
    receiptControl(watermark,'direction','watermark-position',label('Tjs'),'/camera/watermark/position',imageBlock,{options:positions.map(p=>({label:p,value:p})),description:label(description[1]),disabled_unless:'/camera/watermark/isEnabled'});
   }
  }
  // Video resolution row, mounted in the preview block above the framing row.
  // Only 3594/3595/3596 mount that block; 3592 has no resolution selector.
  const previewProof=mounted('CAMERA').find(c=>c.source.includes('resolutionList'));
  if(previewProof) {
   // The mounted preview block falls back to one specific list literal
   // (`X||list`, or a module property for 3596), so resolve that binding
   // instead of guessing between the bundle's several lookalike arrays.
   const quoteText=name=>name.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
   const arrayFor=name=>{
    // Minified names are reused, so every `NAME=[` candidate is inspected and
    // the first array that actually holds resolution records is used.
    const pattern=new RegExp(`(?:^|[,;]|const |let |var )\\s*${quoteText(name)}\\s*=\\s*\\[`,'g');
    for(const start of source.matchAll(pattern)){
     const open=start.index+start[0].length-1;
     let depth=0,end=open;
     for(let i=open;i<source.length;i++){
      const ch=source[i];
      if(ch==='"'||ch==="'"){const q=ch;i++;while(i<source.length&&source[i]!==q){if(source[i]==='\\')i++;i++;}continue;}
      if(ch==='[')depth++;
      else if(ch===']'){depth--;if(depth===0){end=i;break;}}
     }
     const entries=[...source.slice(open,end+1)
      .matchAll(/\{name:"([^"]+)",width:(\d+),height:(\d+),fps:(\d+)\}/g)]
      .map(x=>({label:x[1],value:{width:+x[2],height:+x[3],fps:+x[4]}}));
     if(entries.length>=2)return entries;
    }
    return null;
   };
   const fallback=/available videoResolutions[\s\S]{0,140}?\|\|\s*([A-Za-z_$][\w$]*(?:\.[A-Za-z_$][\w$]*)?)/.exec(previewProof.source);
   if(!fallback)throw Error(`Missing resolution fallback ${pid}`);
   const binding=fallback[1].includes('.')
    ?new RegExp(`(?:[,{])\\s*${quoteText(fallback[1].split('.')[1])}\\s*:\\s*\\(\\)\\s*=>\\s*([\\w$]+)`).exec(source)?.[1]
    :fallback[1];
   if(!binding)throw Error(`Missing resolution binding ${pid}`);
   const resolutions=arrayFor(binding);
   if(!resolutions)throw Error(`Missing resolution list ${pid}`);
   const current=result.profile.camera.resolution;
   if(!resolutions.some(r=>r.value.width===current.width&&r.value.height===current.height&&r.value.fps===current.fps))throw Error(`Unknown default resolution ${pid}`);
   const resolutionTitle=/name:[A-Za-z_$][\w$]*\.([\w$]+),warningTooltip/.exec(previewProof.source);
   if(!resolutionTitle)throw Error(`Missing resolution row ${pid}`);
   const resolutionSection={title:label(resolutionTitle[1]),controls:[]};
   receiptControl(resolutionSection,'select','resolution',label(resolutionTitle[1]),'/camera/resolution',previewProof,{options:resolutions});
   page('CAMERA').sections.unshift(resolutionSection);
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
 const limitations=legacy
  ? ['Live video, camera enumeration, view presets/framing, source overlays and device commands remain incomplete. Mic controls retain independent local device state; no firmware/reboot or hardware success is synthesized.']
  : ['The mounted framing row (zoom, pan/tilt pad, five view presets and the preset shortcut chord) is retained, plus the resolution row and the watermark placement pad wherever the source mounts them. Live video, camera enumeration, watermark appearance and processing notes, AI auto framing, the LDC/resolution disable branch, dragging the white framing box and device commands remain incomplete. Mic controls retain independent local device state; no firmware/reboot or hardware success is synthesized.'];
 // 锐度/增益两行在图像页里由 `supportSharpness`/`supportGain` 决定是否渲染，
 // 而相机标签页的挂载点：3592 完全不传 props（两者默认 false），3594/3595/3596
 // 显式传 `!1`。因此当前稳定版这四个产品都不渲染这两行——这是核对结果，不是缺口。
 // 3587/3589/3590 走共享 Customize 布局，没有这套相机标签页，跳过该核对。
 const camTabMount=legacy?null:/id:3,name:[\w$]+\.W1Z,component:\(0,[\w$]+\.jsx\)\([\w$]+,\{([^}]*)\}\)/.exec(source);
 if(!legacy&&!camTabMount)throw Error(`Missing camera tab mount ${pid}`);
 if(camTabMount&&/supportSharpness:!0|supportGain:!0/.test(camTabMount[1]))throw Error(`Sharpness/gain enabled ${pid}`);
 const sharpnessGain=camTabMount?{rendered:false,props:camTabMount[1]||'(no props)',source:file,offset:source.indexOf(camTabMount[0]),end:source.indexOf(camTabMount[0])+camTabMount[0].length}:{rendered:null,note:'legacy shared Customize layout',source:file};
 output.push(result);
 receipts.push({sharpness_gain_rows:sharpnessGain,product_id:pid,source:file,sha256:hash(source),localization_source:textFile,localization_sha256:hash(textSource),config_source:p.config.path,config_offset:p.config.offset,pages:result.pages.map(pg=>({key:pg.key,control_keys:pg.sections.flatMap(s=>s.controls.map(c=>c.key)),source_navigation_offset:p.navigation.flatMap(n=>n.items).find(i=>i.name.value===pg.key)?.offset})),limitations});
}
fs.writeFileSync(path.join(root,'src/features/source_controls_data.json'),JSON.stringify(output,null,2)+'\n');
fs.writeFileSync(path.join(root,'docs/re/camera-controls-source.json'),JSON.stringify({schema_version:1,scanner_sha256:hash(fs.readFileSync(__filename)),products:receipts},null,2)+'\n');
console.log(`Generated ${output.length} source-specific camera control descriptors.`);
