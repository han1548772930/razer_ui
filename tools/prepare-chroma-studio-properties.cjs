// Parse current webpack source as data. Never evaluate downloaded factories.
const fs=require('fs'),path=require('path');
const {source:s}=require('./audit-chroma-studio.cjs');
const {walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..');
const roots={ambient:1958,audio:9120,breathing:2777,'chroma-generate':2474,fire:5591,reactive:7518,ripple:5305,spectrum:8552,starlight:6548,static:3181,tidal:8154,wave:1591,wheel:8379};
const shared={color:1698,color_dropdown:16,gradient:3690,playback:5805,slider_stepper:768,number_input:2245,slider:2132,duration:1991,custom_colors:4809};
function value(id,n,seen=new Set()){
 if(!n||seen.has(n))throw Error('Unresolved literal');
 seen=new Set([...seen,n]); const v=c=>value(id,c,seen);
 if(n.type==='Literal')return n.value;
 if(n.type==='Identifier')return v(s.binding(id,n.name));
 if(n.type==='UnaryExpression'){
  if(n.operator==='void')return null;
  if(n.operator==='!')return !v(n.argument);
  if(n.operator==='-')return -v(n.argument);
 }
 if(n.type==='SequenceExpression')return v(n.expressions.at(-1));
 if(n.type==='ArrayExpression')return n.elements.map(v);
 if(n.type==='ObjectExpression')return Object.fromEntries(n.properties.map(p=>{
  if(p.type!=='Property')throw Error('Spread');
  return [p.computed?v(p.key):key(p.key),v(p.value)];
 }));
 if(n.type==='MemberExpression'){
  const b=n.object.type==='Identifier'?s.module(id).definitions.get(n.object.name):null;
  if(b?.type==='CallExpression'&&b.callee.name===s.module(id).fn.params[2]?.name&&b.arguments[0]?.type==='Literal')
   return value(b.arguments[0].value,s.exported(b.arguments[0].value,n.computed?v(n.property):key(n.property)),seen);
  return v(n.object)[n.computed?v(n.property):key(n.property)];
 }
 throw Error(`Dynamic ${n.type}`);
}
const constant=k=>value(6257,s.exported(6257,k));
function component(id,n){
 if(n?.type==='Literal')return {tag:n.value};
 if(n?.type==='MemberExpression'&&n.object.type==='Identifier'){
  const b=s.module(id).definitions.get(n.object.name);
  if(b?.type==='CallExpression'&&b.arguments[0]?.type==='Literal')return {module:b.arguments[0].value,export:key(n.property)};
 }
 return {expression:s.snippet(id,n)};
}
function inventory(id){
 const result=[];
 walk(s.module(id).fn,n=>{
  if(n.type!=='CallExpression')return;
  const callee=n.callee.type==='SequenceExpression'?n.callee.expressions.at(-1):n.callee;
  if(callee.type!=='MemberExpression'||!['jsx','jsxs'].includes(key(callee.property))||n.arguments[1]?.type!=='ObjectExpression')return;
  const props={};
  for(const p of n.arguments[1].properties){
   if(p.type!=='Property'||key(p.key)==='children')continue;
   const prop={source:s.snippet(id,p.value)};
   try{prop.literal=value(id,p.value);}catch{/* Dynamic props retain exact source. */}
   props[key(p.key)]=prop;
  }
  result.push({offset:n.start,component:component(id,n.arguments[0]),props});
 });
 return result;
}
const slider=(field,min,max,step=1,extra={})=>({kind:'slider_stepper',field,min,max,step,...extra});
const color=(field,extra={})=>({kind:'color_dropdown',field,...extra});
const random={kind:'checkbox',field:'randomColor'};
const gradient={kind:'gradient',field:'colorStops',canvas_width:180};
const duration=k=>({kind:'duration',field:'duration',values:constant(k)});
const playback={kind:'playback'};
const controls={
 ambient:[{kind:'screen',field:'screen',presets:constant('vl'),native_required:true},slider('blur',constant('A8'),constant('wr'),1,{kind:'slider',with_tip:false})],
 audio:[gradient,slider('boost',constant('c2'),constant('UV'),constant('OM'),{disabled_when:'autoBoost'}),{kind:'checkbox',field:'autoBoost'},slider('decay',constant('vQ'),constant('zH'),constant('ZL'))],
 breathing:[color('color',{disabled_when:'randomColor',empty_color:constant('FU')}),color('color2',{disabled_when:'randomColor',empty_color:constant('FU')}),random,duration('NH'),playback],
 'chroma-generate':[{kind:'media',accept:'image/*, .mp4',field:'mediaSrc'},{kind:'generate',max_configs:constant('aQ'),fields:['effectConfigList','currentConfigId','currentConfig']}],
 fire:[color('color',{meaning:'hot'}),color('color2',{meaning:'cold'})],
 reactive:[color('color',{disabled_when:'randomColor'}),random,duration('MB')],
 ripple:[gradient,slider('speed',constant('Mn'),constant('s8')),slider('width',constant('jw'),constant('Ph'),constant('h1')),playback],
 spectrum:[gradient,duration('Q9')],
 starlight:[{...gradient,disabled_when:'randomColor'},random,slider('density',constant('dV'),constant('JG')),duration('f5')],
 static:[{kind:'color',field:'color',canvas_width:228,canvas_height:136}],
 tidal:[color('color',{disabled_when:'randomColor',empty_color:constant('FU')}),color('color2',{disabled_when:'randomColor',empty_color:constant('FU')}),random,slider('speed',constant('y8'),constant('Sl')),{kind:'center_point',field:'isTidalCenterPointActive',buffer:'editor'},{kind:'direction',field:'counterclockwise',buffer:'params2'},playback],
 wave:[gradient,slider('speed',constant('jU'),constant('nz')),slider('pause',constant('s2'),constant('w7'),1,{kind:'number_input',storage_multiplier:1000}),slider('width',constant('AV'),constant('S1')),{kind:'switch',field:'split'},{kind:'angle',field:'angle',min:0,max:359,step:1},playback],
 wheel:[gradient,slider('speed',constant('fy'),constant('Lb')),{kind:'center_point',field:'isCenterPointActive',buffer:'editor'},{kind:'direction',field:'counterclockwise',buffer:'params2'},playback]
};
const data={
 effects:Object.fromEntries(Object.entries(roots).map(([name,module])=>[name,{module,controls:controls[name]}])),
 shared,
 playback:{start:constant('W4'),end:constant('$e'),cycles_min:constant('YZ'),cycles_max:constant('QG'),inactive_cycles:constant('PT')},
 gradients:{default:{presets:constant('dp'),max_stops:constant('VD'),min_stops:1},audio:{presets:constant('sR'),max_stops:constant('kI'),min_stops:1},spectrum:{presets:constant('Zk'),max_stops:constant('BI'),min_stops:2}},
 color:{canvas_width:228,canvas_height:136,dropdown_canvas_width:208,brightness_min:0,brightness_max:100,brightness_threshold:value(1698,s.binding(1698,'w')),hex_max_length:6,rgb_min:0,rgb_max:255,custom_slots:8,presets:value(1698,s.binding(1698,'x')),native_eyedropper:true},
 semantics:{params:'UA/HZ merges working effectLayer.params and removes patched keys from paramsMixed; it does not synchronize defaults, layer.params or params2.',params2:'RZ/XF merges effectLayer.params2; wheel/tidal center-point and direction use this separate buffer.',commit:'9286:U applies working parameters to device regions. Without real devices do not invent region writes.',tool_change:'9870:d preserves parameters between pen and bucket; transitions involving select/move reset or merge source defaults.',preview:'HZ defaults preview=true; previewEffect runs only with select tool. Continuous slider changes use false; afterChange uses true.',selection:'4264:se disables properties when no devices are selected and tool is select or move.',generate:'UA derives currentConfig from effectConfigList[currentConfigId] or {} only when both list and index occur in the patch.'}
};
const ids=[...new Set([...Object.values(roots),...Object.values(shared),6257,2904,9220,126,9870])];
// Refuse curated numeric ranges that do not occur on the corresponding root's JSX.
for(const [name,id] of Object.entries(roots))for(const control of controls[name]){
 if(!['slider_stepper','number_input','slider','angle'].includes(control.kind))continue;
 const module=control.kind==='angle'?2245:shared[control.kind];
 if(!inventory(id).some(i=>i.component.module===module&&i.props.min?.literal===control.min&&i.props.max?.literal===control.max&&(i.props.step?.literal??1)===control.step))throw Error(`Range not backed by JSX: ${name}.${control.field}`);
}
const receipts=ids.map(id=>({module:id,...s.receipt(id,s.module(id).fn)}));
for(const[id,names]of [[4264,['se']],[9286,['R','N','P','U']],[1638,['j']],[9870,['d']],[2925,['a']],[7155,['c']]])for(const name of names)receipts.push({module:id,symbol:name,...s.receipt(id,s.binding(id,name))});
const manifestPath=`${s.directory}/asset-manifest.json`,manifest=JSON.parse(fs.readFileSync(path.join(root,manifestPath),'utf8'));
const css=[],missingCSS=[];
for(const relative of [...new Set(Object.values(manifest.files))].filter(v=>v.endsWith('.css'))){
 const file=`${s.directory}/${relative.replace(/^\.\//,'')}`;
 if(!fs.existsSync(path.join(root,file))){missingCSS.push(file);continue;}
 const raw=fs.readFileSync(path.join(root,file),'utf8');
 const rules=parseCSS(raw).filter(r=>/panel|section|input-label|color|gradient|slider|stepper|playback|screen|ambient|generate|center-point|direction|duration-preview/.test(r.selector));
 if(rules.length)css.push({path:file,sha256:hash(raw),rules});
}
const evidence={method:'Current manifest-declared JavaScript parsed as Acorn AST; literal extraction and JSX inventory only. No vendor code execution. Null in extracted literals represents source undefined.',manifest:{path:manifestPath,sha256:hash(fs.readFileSync(path.join(root,manifestPath)))},receipts,jsx:Object.fromEntries(ids.map(id=>[id,inventory(id)])),css,missing_css:missingCSS};
for(const[file,obj]of [['src/features/chroma_studio_properties_data.json',data],['docs/re/chroma-studio-properties-source.json',evidence]]){
 const text=JSON.stringify(obj,null,2)+'\n',target=path.join(root,file);
 if(process.argv.includes('--check')){if(fs.readFileSync(target,'utf8')!==text)throw Error(`Stale ${file}`);}else fs.writeFileSync(target,text);
}
console.log(`Studio properties: ${Object.keys(roots).length} effect roots, ${receipts.length} receipts, ${css.length} CSS files; ${missingCSS.length} unavailable declared CSS files.`);
