// Build native audio descriptors from parsed current source. No eval/import of source.
const fs=require('fs'),path=require('path'),acorn=require('acorn'),crypto=require('crypto');
const root=path.resolve(__dirname,'..'),read=p=>fs.readFileSync(path.join(root,p),'utf8');
const evidence=[...JSON.parse(read('docs/re/audio-product-evidence.json')).products,
 ...JSON.parse(read('docs/re/audio-additional-evidence.json')).products];
const originals=[...JSON.parse(read('docs/re/audio-product-configs.json')).products,
 ...JSON.parse(read('docs/re/source-product-configs.json')).products];
const key=n=>n?.name??n?.value;
function literal(n) {
 if(!n)return undefined;
 if(n.type==='Literal')return n.value;
 if(n.type==='UnaryExpression'){const v=literal(n.argument);if(v!==undefined){if(n.operator==='-')return -v;if(n.operator==='!')return !v;}}
 if(n.type==='ArrayExpression')return n.elements.map(literal);
 if(n.type==='ObjectExpression'){const o={};for(const p of n.properties)if(p.type==='Property'&&!p.computed){const v=literal(p.value);if(v!==undefined)o[key(p.key)]=v;}return o;}
 return undefined;
}
function walk(n,f){if(!n?.type)return;f(n);for(const v of Object.values(n)){if(Array.isArray(v)){for(const c of v)walk(c,f);}else if(v?.type)walk(v,f);}}
const files=new Map();
function defaults(component) {
 const name=/^class ([\w$]+)/.exec(component.source)?.[1];if(!name)return {};
 if(!files.has(component.path))files.set(component.path,read(component.path));
 const source=files.get(component.path), token=name+'.defaultProps=', index=source.indexOf(token,component.offset);
 if(index<0||index>component.end+1000)return {};
 try{return literal(acorn.parseExpressionAt(source,index+token.length,{ecmaVersion:'latest'}))??{};}catch{return {};}
}
const fieldInfo={
 volume:['VOLUME_HEADER'],speakerVolume:['VOLUME_HEADER'],bassBoost:['BASS_BOOST_HEADER','BASS_BOOST_DESC'],
 soundNormalization:['SOUND_NORMALIZATION_HEADER','SOUND_NORMALIZATION_DESC'],voiceClarity:['VOICE_CLARITY_HEADER','VOICE_CLARITY_DESC_1'],
 micVolume:['MIC_VOLUME','MIC_VOLUME_DESC'],micSensitivity:['VOICE_GATE','VOICE_GATE_DESC'],micSideTone:['MIC_MONITORING_SIDETONE','SIDETONE_DESC'],
 micVolumeNormalization:['VOLUME_NORMALIZATION'],micVoiceClarity:['VOICE_CLARITY_HEADER'],ambientNoiseReduction:['AMBIENT_NOISE_REEDUCTION'],
 micAmbientNoiseReduction:['AMBIENT_NOISE_REEDUCTION'],micNoiseCancellationEnabled:['MIC_NOISE_CANCELLATION'],micBoost:['MIC_BOOST'],isMicBoostEnabled:['MIC_BOOST'],
 thxSpatialAudio:['THX_SPATIAL_AUDIO'],hapticIntensity:['HAPTIC_INTENSITY'],powerSaving:['AUDIO_POWER_SAVING_HEADER','AUDIO_POWER_SAVING_DESC'],
 brightness:['BRIGHTNESS_HEADER'],switchOffLighting:['SWITCH_OFF_LIGHTING_HEADER'],
 isHighPassFilterEnabled:['HIGH_PASS_FILTER'],isAnalogGainLimiterEnabled:['ANALOG_GAIN_LIMITER'],microphoneLimiter:['DIGITAL_GAIN_LIMITER_TITLE'],
 isAdvanceGainSettingsEnabled:['ADVANCE_GAIN_SETTINGS'],isMicrophoneAdjustmentIndicator:['MICROPHONE_VOLUME_ADJUSTMENT_INDICATOR'],
 isHeadphoneAdjustmentIndicator:['HEADPHONE_VOLUME_ADJUSTMENT_INDICATOR'],peakingIndicator:['PEAKING_INDICATOR'],muteEffect:['MUTE_EFFECT'],
 notDisturb:['DO_NOT_DISTURB'],noiseCancellation:['ACTIVE_NOISE_CANCELLATION'],audioPrompts:['AUDIO_PROMPTS'],
 gameChatBalance:['GAME_CHAT_BALANCE'],stereoWidening:['STEREO_WIDENING'],centerFocus:['CENTER_FOCUS'],
 autoPause:['AUTO_PAUSE'],lowLatencyMode:['LOW_LATENCY_MODE'],isHighSpeedEnabled:['ENHANCEMENTS_ULTRA_LOW_LATENCY_TITLE'],
};
const audioFields=['volume','speakerVolume','thxSpatialAudio','bassBoost','soundNormalization','voiceClarity','gameChatBalance','stereoWidening','centerFocus'];
const micFields=['micVolume','micSensitivity','micSideTone','micVolumeNormalization','micVoiceClarity','ambientNoiseReduction','micAmbientNoiseReduction','micNoiseCancellationEnabled','micBoost','isMicBoostEnabled','isHighPassFilterEnabled','isAnalogGainLimiterEnabled','microphoneLimiter','isAdvanceGainSettingsEnabled','audioPrompts'];
function allowed(page,field) {
 if(['TAB_LIGHTING','LIGHTING'].includes(page))return ['brightness','switchOffLighting','isMicrophoneAdjustmentIndicator','isHeadphoneAdjustmentIndicator','peakingIndicator','muteEffect'].includes(field);
 if(page==='TAB_POWER')return field==='powerSaving';
 if(['MIC','TAB_MIC'].includes(page))return micFields.includes(field)||field==='volume';
 if(['TAB_SOUND','TAB_EQ','SURROUND'].includes(page))return audioFields.includes(field);
 if(page==='TAB_ENHANCEMENT')return ['bassBoost','soundNormalization','voiceClarity','hapticIntensity','notDisturb','noiseCancellation','audioPrompts','autoPause','lowLatencyMode','isHighSpeedEnabled'].includes(field);
 if(page==='TAB_HAPTICS')return field==='hapticIntensity';
 if(['TAB_DEMO','DEMO'].includes(page))return field==='thxSpatialAudio';
 return false;
}
function number(v){return typeof v==='number'&&Number.isFinite(v);}
const products=[],coverage=[];
for(const p of evidence){
 const config=p.config, original=originals.find(o=>o.product_id===p.product_id);
 const draft={profile:structuredClone(config.DEFAULTPROFILE??original.config.exports.DEFAULTPROFILE??{}),device:{}};
 const states=p.states.filter(s=>s.fields.includes('actionsFromUI'));
 for(const u of p.unresolved){try{const v=literal(acorn.parseExpressionAt(u.source,0,{ecmaVersion:'latest'}));if(v&&Array.isArray(v.actionsFromUI))states.push({value:v,fields:Object.keys(v)});}catch{}}
 for(const field of Object.keys(fieldInfo))if(draft.profile[field]===undefined){const s=states.find(s=>s.value[field]!==undefined);if(s)draft.device[field]=structuredClone(s.value[field]);}
 const locate=field=>draft.profile[field]!==undefined?'/profile/'+field:draft.device[field]!==undefined?'/device/'+field:null;
 const pages=[],equalizers=[],gaps=[],supplementary_source=[];
 const decoded={};
 for(const u of p.unresolved){try{
  const ast=acorn.parseExpressionAt(u.source,0,{ecmaVersion:'latest'});
  if(u.name.endsWith('BandPreset'))walk(ast,n=>{if(n.type==='CallExpression'&&n.callee.type==='MemberExpression'&&key(n.callee.property)==='entries'){const v=literal(n.arguments[0]);if(v)decoded[u.name]=v;}});
  else if(['defaultEQData','EQ_CHART_OPT'].includes(u.name))decoded[u.name]=literal(ast);
 }catch{}}
 const configs={...config,...decoded};
 function reducerInitial(name){
  // Redux's explicit undefined-state return binds the reducer to its literal
  // declaration in this exact bundle; symbols are never shared across PIDs.
  const re=new RegExp(name+':function\\([^)]*\\)\\{if\\("undefined"===typeof [\\w$]+\\)return ([\\w$]+);');
  const match=re.exec(read(p.source));return match?states.find(s=>s.symbol===match[1])?.value:null;
 }
 // Some products keep frequency arrays private in the same CONFIG scope.
 const configSource=read(original.config.path), configNode=acorn.parseExpressionAt(configSource,original.config.offset,{ecmaVersion:'latest'});
 const privateArrays=new Map();walk(configNode,n=>{if(n.type==='VariableDeclarator'&&n.id.type==='Identifier'&&n.init?.type==='ArrayExpression'){const v=literal(n.init);if(v?.every(number))privateArrays.set(n.id.name,v);}});
 for(const kind of ['audio','mic'])if(!configs[kind+'BandFrequency']){
  const u=p.unresolved.find(u=>u.name===kind+'BandPreset');const symbol=/frequency:([\w$]+)\[/.exec(u?.source??'')?.[1];
  if(privateArrays.has(symbol))configs[kind+'BandFrequency']=privateArrays.get(symbol);
 }

 function rangeFor(field,page){
  const matched=page.components.filter(c=>new RegExp('\\b'+field+'\\b').test(c.source));
  const jsx=matched.flatMap(c=>c.jsx.map(j=>({j,c})));
  const direct=jsx.filter(({j})=>Object.values(j.expressions).some(e=>typeof e==='string'&&new RegExp('\\b'+field+'\\.value\\b').test(e)));
  const definition={};
  for(const c of matched)Object.assign(definition,defaults(c));
  const configName=field==='micSensitivity'?'voiceGateConfig':field==='micSideTone'?'volumeSideToneConfig':field+'Config';
  let range={...(definition[configName]??{}),...(configs[configName]??{}),...(field==='micSensitivity'?configs.VOICE_GATE_CONFIG??{}:{})};
  for(const {j} of jsx)if(j.props[configName]&&typeof j.props[configName]==='object')Object.assign(range,j.props[configName]);
  if(field==='powerSaving')Object.assign(range,Object.fromEntries(['min','max','step'].filter(k=>number(definition[k])).map(k=>[k,definition[k]])));
  for(const {j} of direct)for(const prop of ['min','max','step']){
   if(number(j.props[prop]))range[prop]=j.props[prop];
   else if(!number(range[prop])){const fallback=/:\s*(-?\d+(?:\.\d+)?)$/.exec(j.expressions[prop]??'');if(fallback)range[prop]=Number(fallback[1]);}
  }
  if(field==='micSideTone'&&!number(range.max)&&matched.some(c=>/volumeSideToneConfig/.test(c.source)))range.max=100;
  if(!number(range.min)&&['volume','speakerVolume','micVolume','brightness','gameChatBalance'].includes(field)&&jsx.some(({j})=>j.props.min===0||j.props.minValue===0))range.min=0;
  if(!number(range.max)&&['volume','speakerVolume','micVolume','brightness','gameChatBalance'].includes(field)&&jsx.some(({j})=>j.props.max===100||j.props.maxValue===100))range.max=100;
  for(const {j} of jsx)if(Array.isArray(j.props.data)){const item=j.props.data.find(d=>d.key===field);if(item&&number(item.minValue)&&number(item.maxValue)){range.min=item.minValue;range.max=item.maxValue;range.step=item.step??1;}}
  if(!number(range.min)&&['bassBoost','soundNormalization','voiceClarity'].includes(field)&&matched.some(c=>/currentSetting/.test(c.source)))range.min=10;
  if(!number(range.max)&&['bassBoost','soundNormalization','voiceClarity'].includes(field)&&matched.some(c=>/currentSetting/.test(c.source)))range.max=100;
  if(!number(range.min)||!number(range.max)||range.max<=range.min)return null;
  return {min:range.min,max:range.max,step:number(range.step)&&range.step>0?range.step:1};
 }
 for(const pg of p.pages){
  const sections=[];const source=pg.components.map(c=>c.source).join('\n');
  for(const [field,[title,description]] of Object.entries(fieldInfo)){
   if(!allowed(pg.key,field)||!new RegExp('\\b'+field+'\\b').test(source))continue;
   const base=locate(field);if(!base)continue;
   const value=(base.startsWith('/profile')?draft.profile:draft.device)[field], controls=[];
   if(typeof value==='boolean')controls.push({path:base,label:title,kind:'toggle'});
   else if(value&&typeof value==='object'&&!Array.isArray(value)){
    const enabled=typeof value.isEnabled==='boolean'?base+'/isEnabled':null;
    if(enabled)controls.push({path:enabled,label:title,kind:'toggle'});
    if(field==='switchOffLighting'){
     if(typeof value.isDisplayOn==='boolean')controls.push({path:base+'/isDisplayOn',label:'SWITCH_OFF_LIGHTING_DISPLAY_OFF',kind:'toggle'});
     if(typeof value.isIdleEnabled==='boolean')controls.push({path:base+'/isIdleEnabled',label:'SWITCH_OFF_LIGHTING_IDLE',kind:'toggle'});
     // Idle timing requires the actual mounted input bounds, not a family default.
     const slider=pg.components.flatMap(c=>c.jsx).find(j=>j.expressions.value?.includes('idleMinutes')&&number(j.props.min)&&number(j.props.max));
     if(slider&&number(value.idleMinutes))controls.push({path:base+'/idleMinutes',label:'MINUTES',kind:'slider',min:slider.props.min,max:slider.props.max,step:slider.props.step??1,enabled_by:base+'/isIdleEnabled'});
    }else if(number(value.value)){
     const power=configs.POWER_SAVING_VALUE_FROM_PRODUCT_INFO;
     if(field==='powerSaving'&&Array.isArray(power)&&power.length&&power.every(v=>number(v.value))){controls.push({path:base+'/value',label:title,kind:'select',options:power.map(v=>({label:String(v.value)+' min',value:v.value})),enabled_by:enabled});}
     else{const range=rangeFor(field,pg);if(range){value.value=Math.max(range.min,Math.min(range.max,value.value));controls.push({path:base+'/value',label:title,kind:'slider',...range,enabled_by:enabled,unit:field==='powerSaving'?' min':''});}else gaps.push({page:pg.key,field,reason:'Mounted control range requires further binding resolution'});}
    }
   }
   if(controls.length)sections.push({title,...(description?{description}:{}),controls});
  }
  // Quick-effect IDs and options come from this product CONFIG, never a family list.
  if(['TAB_LIGHTING','LIGHTING'].includes(pg.key)&&Array.isArray(configs.QUICK_EFFECTS)&&draft.profile.quickEffects&&!Array.isArray(draft.profile.quickEffects)){
   const effects=configs.QUICK_EFFECTS.filter(e=>typeof e.name==='string'&&number(e.id));
   const field=number(draft.profile.quickEffects.selectedEffectId)?'selectedEffectId':number(draft.profile.quickEffects.effect)?'effect':null;
   if(field&&effects.length)sections.push({title:'EFFECTS',controls:[{path:'/profile/quickEffects/'+field,label:'QUICK_EFFECTS',kind:'select',options:effects.map(e=>({label:e.name,value:e.id}))}]});
  }
  for(const kind of ['audio','mic']){
   const isMic=kind==='mic';
   if(isMic?!['MIC','TAB_MIC'].includes(pg.key):!['TAB_SOUND','TAB_EQ'].includes(pg.key))continue;
   if(!new RegExp(isMic?'micEqualizer|micBandPreset|micEqReducer':'audioEqualizer|audioBandPreset|eqProfileReducer|selectedPreset').test(source))continue;
   const frequency=configs[isMic?'micBandFrequency':'audioBandFrequency'];if(!Array.isArray(frequency)||!frequency.length)continue;
   let presetData=configs[isMic?'micBandPreset':'audioBandPreset']??(isMic?configs.micBandData:null),selected=(draft.profile[isMic?'micEqualizer':'audioEqualizer']??draft.profile.equalizer??{}).mode??'default';
   let presets=[],editCustom=true;
   const modern=!isMic?(draft.profile.audioEqualizer?.presets??(source.includes('selectedPresetId')?configs.defaultEQData:null)):null;
   if(modern){
    const arrays=Array.isArray(modern.presets)?[modern.presets]:[modern.standard,modern.esports].filter(Array.isArray);
    presets=arrays.flat().filter(p=>typeof p.name==='string'&&Array.isArray(p.current)&&p.current.length===frequency.length).map(p=>({key:p.name,label:p.name,bands:p.current}));
    selected=modern.selectedPreset?.standard??(presets[modern.selectedPresetId??0]?.key)??selected;editCustom=false;
   }
   if(!presets.length&&presetData)presets=Object.entries(presetData).filter(([_,v])=>Array.isArray(v)&&v.length===frequency.length&&v.every(number)).map(([key,bands])=>({key,label:key,bands}));
   const tabs=configs[isMic?'Mic_EQ_Tabs':'Audio_EQ_Tabs'];
   if(Array.isArray(tabs)){for(const p of presets)p.label=tabs.find(t=>t.id===p.key)?.name??p.label;if(tabs.some(t=>t.id==='custom')&&!presets.some(p=>p.key==='custom'))presets.push({key:'custom',label:'CUSTOM',bands:frequency.map(()=>0)});}
   if(!presets.length){gaps.push({page:pg.key,field:kind+'Equalizer',reason:'Current preset data requires further literal resolution'});continue;}
   let bounds=isMic?null:configs.bandDataConfig;
   const eqOptions=pg.components.flatMap(c=>c.jsx).find(j=>j.props.options&&number(j.props.options.min)&&number(j.props.options.max));
   if(eqOptions)bounds={minValue:eqOptions.props.options.min,maxValue:eqOptions.props.options.max};
   if(!bounds&&isMic)bounds=configs.bandDataConfig;
   if(!bounds){const ticks=configs.EQ_CHART_OPT?.scales?.yAxes?.[0]?.ticks;if(ticks)bounds={minValue:ticks.min,maxValue:ticks.max};}
   if(!bounds){
    const range=pg.components.flatMap(c=>c.jsx).find(j=>number(j.props.minValue)&&j.props.minValue<0&&number(j.props.maxValue));
    if(range)bounds=range.props;
   }
   if(!number(bounds?.minValue)||!number(bounds?.maxValue)){gaps.push({page:pg.key,field:kind+'Equalizer',reason:'Current EQ axis range requires further source resolution'});continue;}
   if(!presets.some(p=>p.key===selected))selected=presets[0].key;
   if(!equalizers.some(e=>e.key===kind))equalizers.push({key:kind,frequencies:frequency.map(f=>f>=1000?`${f/1000}k`:String(f)),presets,selected,min:bounds.minValue,max:bounds.maxValue,step:1,edit_custom:editCustom});
   sections.push({title:isMic?'MIC_EQUALIZER':'AUDIO_EQUALIZER',controls:[],equalizer:kind});
  }
  // The stream bus is a reducer on some microphones and profile-owned on others.
  if(pg.key==='STREAM_MIXER_HEADER'&&!draft.profile.streamMixerSettings&&draft.profile.isStreamMixerEnabled!==undefined){
   draft.device.streamMixerSettings=Object.fromEntries(['isStreamMixerEnabled','isStreamMixMonitor','streamMixVolume','playbackMixVolume'].map(k=>[k,structuredClone(draft.profile[k])]));
   // These products explicitly declare their physical headphone/mic channels
   // nonexistent. Do not fabricate those rows from shared DEFAULTPROFILE data.
  }
  if(pg.key==='STREAM_MIXER_HEADER'&&!draft.profile.streamMixerSettings&&!draft.device.streamMixerSettings){
   const initial=states.find(s=>s.value.isStreamMixerEnabled!==undefined&&s.value.streamMixVolume)?.value;
   if(initial)draft.device.streamMixerSettings=Object.fromEntries(['isStreamMixerEnabled','isStreamMixMonitor','streamMixVolume','playbackMixVolume'].map(k=>[k,structuredClone(initial[k])]));
  }
  // Stream and playback buses retain independent gain/mute values and link state.
  if(pg.key==='STREAM_MIXER_HEADER'&&(draft.profile.streamMixerSettings||draft.device.streamMixerSettings)){
   const s=draft.profile.streamMixerSettings??draft.device.streamMixerSettings,base=draft.profile.streamMixerSettings?'/profile/streamMixerSettings':'/device/streamMixerSettings';
   sections.push({title:'STREAM_MIXER_HEADER',controls:[{path:base+'/isStreamMixerEnabled',label:'STREAM_MIXER_HEADER',kind:'toggle'},{path:base+'/isStreamMixMonitor',label:'AUDIO_MONITORING',kind:'toggle',enabled_by:base+'/isStreamMixerEnabled'}]});
   for(const [bus,title] of [['streamMixVolume','STREAM_MIX_HEADER'],['playbackMixVolume','PLAYBACK_MIX']])if(s[bus])sections.push({title,controls:[{path:`${base}/${bus}/isEnabled`,label:title,kind:'toggle',enabled_by:base+'/isStreamMixerEnabled'},{path:`${base}/${bus}/value`,label:'VOLUME_HEADER',kind:'slider',min:0,max:100,step:1,enabled_by:`${base}/${bus}/isEnabled`}]});
   for(const [index,channel] of (s.otherCategories??[]).entries()){
    const row=`${base}/otherCategories/${index}`,controls=[{path:row+'/isLinked',label:'LINK',kind:'toggle',enabled_by:base+'/isStreamMixerEnabled'}];
    for(const [bus,label] of [['microphone','STREAM_MIX_HEADER'],['headphone','PLAYBACK_MIX']])if(channel[bus])controls.push({path:`${row}/${bus}/isEnabled`,label,kind:'toggle',enabled_by:base+'/isStreamMixerEnabled'},{path:`${row}/${bus}/value`,label,kind:'slider',min:0,max:100,step:1,enabled_by:`${row}/${bus}/isEnabled`});
    sections.push({title:channel.name,controls});
   }
  }
  // Flat Redux output-volume controls must not disappear merely because they
  // bind `value` instead of a nested `volume.value` object.
  if(pg.key==='TAB_SOUND'&&!sections.some(s=>s.title==='VOLUME_HEADER')){
   const component=pg.components.find(c=>c.jsx.some(j=>j.props.title==='VOLUME_HEADER'));
   const slider=component?.jsx.find(j=>j.props.min===0&&j.props.max===100);
   const initial=states.find(s=>s.fields.includes('value')&&s.fields.includes('isEnabled')&&number(s.value.value))?.value;
   if(slider&&initial){draft.device.outputVolume={value:initial.value,isEnabled:initial.isEnabled};sections.unshift({title:'VOLUME_HEADER',controls:[{path:'/device/outputVolume/isEnabled',label:'VOLUME_HEADER',kind:'toggle'},{path:'/device/outputVolume/value',label:'VOLUME',kind:'slider',min:0,max:100,step:slider.props.step??1,enabled_by:'/device/outputVolume/isEnabled'}]});}
  }
  // Modern enhancement selectors are mutually exclusive. Keep that invariant
  // when their native switches request a different enhancement.
  for(const c of pg.components)for(const j of c.jsx)if(Array.isArray(j.props.data)&&j.props.data.every(d=>d.key&&d.fnCall)){
   const peers=j.props.data.map(d=>locate(d.key)).filter(Boolean).map(p=>p+'/isEnabled');
   for(const section of sections)for(const control of section.controls)if(peers.includes(control.path))control.exclusive_with=peers.filter(p=>p!==control.path);
  }
  // The physical front dial and multifunction switch are separate settings.
  if(pg.key==='MIC'){
   for(const field of ['dialControl','isMultiFunctionEnabled']){
    const matched=pg.components.filter(c=>new RegExp('\\b'+field+'\\b').test(c.source));if(!matched.length)continue;
    const initial=draft.profile[field]??states.find(s=>s.value[field]!==undefined)?.value[field];if(initial===undefined)continue;
    const base=draft.profile[field]!==undefined?'/profile/'+field:'/device/'+field;
    if(base.startsWith('/device/'))draft.device[field]=initial;
    if(typeof initial==='boolean')sections.push({title:'MUTE_BUTTON_TITLE',controls:[{path:base,label:'MULTI_FUNCTION',kind:'toggle'}]});
    else{const tabs=matched.flatMap(c=>c.jsx).find(j=>Array.isArray(j.props.tabs)&&j.props.tabs.every(t=>t.name&&t.value!==undefined))?.props.tabs;
     if(tabs)sections.push({title:'FRONT_DIAL_TITLE',controls:[{path:base,label:'FRONT_DIAL_TITLE',kind:'select',options:tabs.map(t=>({label:t.name,value:t.value}))}]});}
   }
  }
  // DSP settings retain the product's literal defaults and per-parameter ranges.
  const dspLabels={noiseGate:'NOISE_GATE',compressor:'COMPRESSOR',noise_gate:'NOISE_GATE',mic_compressor:'COMPRESSOR',mic_reverb:'REVERB',ai_noise_suppression:'AI_NOISE_SUPPRESSION',mic_limiter:'LIMITER'};
  const parameterLabels={thresholdDB:'THRESHOLD',threshold:'THRESHOLD',db:'THRESHOLD',atTime:'ATTACK_TIME',attackTime:'ATTACK_TIME',rlTime:'RELEASE_TIME',releaseTime:'RELEASE_TIME',holdTime:'HOLD_TIME',decayDB:'DECAY',ratio:'RATIO',intensity:'INTENSITY',value:'LEVEL',targetGain:'TARGET_GAIN',makeUpGain:'MAKE_UP_GAIN',softKneeWidth:'SOFT_KNEE_WIDTH',basic:'LEVEL'};
  if(['TAB_MIC','TAB_EFFECTS','EFFECTS'].includes(pg.key))for(const [field,title] of Object.entries(dspLabels)){
   const initial=states.find(s=>s.value[field]&&typeof s.value[field]==='object')?.value[field];
   const mounted=pg.components.filter(c=>new RegExp('\\b'+field+'\\b').test(c.source));if(!initial||!mounted.length)continue;
   draft.device[field]=structuredClone(initial);const base='/device/'+field,controls=[];
   if(typeof initial.isEnabled==='boolean')controls.push({path:base+'/isEnabled',label:title,kind:'toggle'});
   for(const [parameter,value] of Object.entries(initial)){
    if(parameter==='isEnabled'||parameter==='useMode'||parameter==='roomMode')continue;
    let range=typeof value==='object'&&value&&number(value.min)&&number(value.max)?value:null;
    if(!range){const j=mounted.flatMap(c=>c.jsx).find(j=>number(j.props.min)&&number(j.props.max)&&Object.entries(j.expressions).some(([key,e])=>['onChange','changeValue','setParentState'].includes(key)&&e.includes('"'+parameter+'"')));if(j)range=j.props;}
    const current=number(value)?value:value?.value;if(!range||!number(current))continue;
    const target=base+'/'+parameter+(number(value)?'':'/value');
    if(number(value))draft.device[field][parameter]=Math.max(range.min,Math.min(range.max,current));
    controls.push({path:target,label:parameterLabels[parameter]??parameter,kind:'slider',min:range.min,max:range.max,step:range.step??1,enabled_by:base+'/isEnabled'});
   }
   if(initial.roomMode!==undefined&&Array.isArray(configs.ROOM_SIZES))controls.push({path:base+'/roomMode',label:'ROOM_SIZE',kind:'select',options:configs.ROOM_SIZES.map(r=>({label:r.name.toUpperCase(),value:r.roomMode})),enabled_by:base+'/isEnabled'});
   if(controls.length)sections.push({title,controls});
  }
  // Parametric EQ presets have variable frequencies and Q; these are not a
  // fixed graphic EQ and must keep their own per-preset band objects.
  if(pg.key==='TAB_EQ'&&Array.isArray(configs.defaultEQData?.data)){
   draft.device.parametric=structuredClone(configs.defaultEQData);
   const base='/device/parametric';
   sections.push({title:'PARAMETRIC_EQUALIZER',controls:[{path:base+'/selectedPreset',label:'PRESET',kind:'select',options:configs.defaultEQData.data.map(p=>({label:'EQ_PRESET_'+p.preset.toUpperCase().replace('FPSCALLOUT','FPS_CALLOUT'),value:p.preset}))}]});
   for(const [index,preset] of configs.defaultEQData.data.entries()){
    const visible_when={path:base+'/selectedPreset',value:preset.preset},presetPath=`${base}/data/${index}`;
    const controls=[];
    if(preset.highPassFilter){controls.push({path:presetPath+'/highPassFilter/isEnabled',label:'HIGH_PASS_FILTER',kind:'toggle'});
     if(Array.isArray(configs.HIGH_PASS_FILTER_FREQUENCIES))controls.push({path:presetPath+'/highPassFilter/value',label:'EQ_FREQUENCY',kind:'select',options:configs.HIGH_PASS_FILTER_FREQUENCIES.map(f=>({label:f.frequency+' Hz',value:f.value})),enabled_by:presetPath+'/highPassFilter/isEnabled'});}
    sections.push({title:'HIGH_PASS_FILTER',visible_when,controls});
    for(const [bandIndex,band] of preset.bands.entries()){
     if(band.status===0)continue;
     const bandPath=`${presetPath}/bands/${bandIndex}`;
     const controls=[];
     for(const [parameter,label] of [['frequency','EQ_FREQUENCY'],['gain','EQ_GAIN'],['q','Q_FACTOR']]){
      const j=pg.components.flatMap(c=>c.jsx).find(j=>number(j.props.minValue)&&number(j.props.maxValue)&&j.expressions.value?.includes('.'+parameter));
      if(j&&number(band[parameter]))controls.push({path:bandPath+'/'+parameter,label,kind:'slider',min:j.props.minValue,max:j.props.maxValue,step:j.props.stepValue??1,unit:parameter==='frequency'?' Hz':parameter==='gain'?' dB':''});
     }
     sections.push({title:'EQ_BAND',suffix:` ${band.id+1}`,visible_when,controls});
    }
   }
  }
  if(pg.key==='TAB_CALIBRATION'&&draft.profile.calibration?.speakers){
   const bounds=p.arrays.find(a=>a.value.length&&a.value.every(v=>v&&typeof v.id==='string'&&number(v.minAngle)&&number(v.maxAngle)))?.value;
   if(bounds){
    sections.push({title:'SPATIAL_AUDIO_CALIBRATION',controls:[{path:'/profile/calibration',label:'RESET',kind:'reset'}]});
    for(const [index,speaker] of draft.profile.calibration.speakers.entries()){
     const bound=bounds.find(b=>b.id===speaker.id);if(!bound)continue;
     sections.push({title:bound.name??'CHANNEL',suffix:bound.name?'':' '+(index+1),controls:[{path:`/profile/calibration/speakers/${index}/angle`,label:'SPATIAL_AUDIO_CALIBRATION',kind:'slider',min:bound.minAngle,max:bound.maxAngle,step:1,unit:'°'}]});
    }
   }
  }
  if(pg.key==='TAB_CUSTOMIZE'&&source.includes('mixerBindReducer')){
   const initial=reducerInitial('mixerBindReducer')?.mixerBind;
   const inputs=p.arrays.find(a=>a.value.length===7&&a.value[0]?.id==='mic'&&a.value.some(v=>v.id==='optical'))?.value;
   const mutes=p.arrays.find(a=>a.value.length===5&&a.value[0]?.id==='muteAll'&&a.value.some(v=>v.id==='voiceChat'))?.value;
   if(initial&&inputs&&mutes){draft.device.mixerBind=structuredClone(initial);const base='/device/mixerBind';
    for(const channel of Object.keys(initial.inputSource))sections.push({title:'CHANNEL',suffix:' '+channel.slice(-1),controls:[{path:`${base}/inputSource/${channel}`,label:'INPUT_SOURCE',kind:'select',options:inputs.map(i=>({label:i.name,value:i.id}))},{path:`${base}/muteButton/${channel}`,label:'MUTE_BUTTON_TITLE',kind:'select',options:mutes.map(i=>({label:i.name,value:i.id}))}]});
    const longPress=p.arrays.find(a=>a.value.some(v=>v?.id==='voiceChanger')&&a.value.some(v=>v?.id==='bleep'))?.value;
    if(longPress)sections.push({title:'MIC_MUTE_BUTTON',controls:[{path:base+'/longPressMicMuteButton',label:'LONG_PRESS',kind:'select',options:longPress.map(i=>({label:i.name,value:i.id}))}]});
   }
  }
  if(pg.key==='TAB_MIXER'){
   for(const [reducer,title] of [['outputMixerReducer','OUTPUT_MIXER'],['playbackMixReducer','PLAYBACK_MIX'],['streamMixReducer','STREAM_MIX_HEADER'],['lineOutReducer','LINE_OUT'],['voiceChatReducer','VOICE_CHAT']]){
    if(!source.includes(reducer))continue;const initial=reducerInitial(reducer);if(!initial)continue;
    draft.device[reducer]=structuredClone(initial);for(const f of ['actionsFromUI','actionsFromLocalStorage','errorActions','frequency'])delete draft.device[reducer][f];
    const controls=[],base='/device/'+reducer;
    for(const [field,v] of Object.entries(initial)){
     if(!v||typeof v!=='object'||Array.isArray(v))continue;
     const label=field==='volume'?'VOLUME_HEADER':field==='lineIn'?'LINE_IN':field==='xlr'?'XLR':field.toUpperCase();
     if(typeof v.isEnabled==='boolean')controls.push({path:`${base}/${field}/isEnabled`,label,kind:'toggle'});
     if(number(v.value)&&number(v.min)&&number(v.max))controls.push({path:`${base}/${field}/value`,label,kind:'slider',min:v.min,max:v.max,step:v.step??1,unit:' dB',enabled_by:typeof v.isEnabled==='boolean'?`${base}/${field}/isEnabled`:null});
     if(typeof v.isMuted==='boolean')controls.push({path:`${base}/${field}/isMuted`,label:'MUTE',kind:'toggle',enabled_by:`${base}/${field}/isEnabled`});
    }
    if(controls.length)sections.push({title,controls});
   }
  }
  if(pg.key==='EFFECTS')for(const [field,title] of [['vocalFading','VOCAL_FADING'],['keyShifter','KEY_SHIFTER'],['voiceChanger','VOICE_CHANGER']]){
   const reducer=field+'Reducer';if(!source.includes(reducer))continue;const initial=reducerInitial(reducer)?.[field];if(!initial)continue;
   draft.device[field]=structuredClone(initial);const base='/device/'+field,controls=[{path:base+'/isEnabled',label:title,kind:'toggle'}];
   const matched=pg.components.filter(c=>c.source.includes(reducer));
   const slider=matched.flatMap(c=>c.jsx).find(j=>number(j.props.min)&&number(j.props.max));
   if(slider)controls.push({path:base+'/value',label:title,kind:'slider',min:slider.props.min,max:slider.props.max,step:slider.props.step??1,enabled_by:base+'/isEnabled'});
   if(field==='voiceChanger'){const choices=p.arrays.find(a=>a.value.some(v=>v?.name==='CARTOON')&&a.value.some(v=>v?.name==='MONSTER'))?.value;if(choices)controls.push({path:base+'/value',label:title,kind:'select',options:choices.map(c=>({label:c.name,value:c.id})),enabled_by:base+'/isEnabled'});}
   sections.push({title,controls});
  }
  if(pg.key==='TAB_LIGHTING'&&Array.isArray(draft.profile.brightness)&&source.includes('region')){
   const range=pg.components.flatMap(c=>c.jsx).find(j=>j.props.min===0&&j.props.max===100);
   if(range)for(const [index,region] of draft.profile.brightness.entries()){
    sections.push({title:'BRIGHTNESS_HEADER',suffix:' · '+region.regionId,controls:[{path:`/profile/brightness/${index}/isEnabled`,label:'BRIGHTNESS_HEADER',kind:'toggle'},{path:`/profile/brightness/${index}/value`,label:'BRIGHTNESS_HEADER',kind:'slider',min:0,max:100,step:range.props.step??1,enabled_by:`/profile/brightness/${index}/isEnabled`}]});
   }
  }
  if(['TAB_HAPTICS','TAB_CUSTOMIZE'].includes(pg.key)){
   const initial=states.find(s=>s.fields.includes('sensaSource')&&s.fields.includes('regions'))?.value;
   const component=pg.components.find(c=>c.jsx.some(j=>j.props.title==='HAPTIC_INTENSITY'));
   if(initial&&component){
    const min=Number(/valueMin:[\w$]+=(\d+)/.exec(component.source)?.[1]),max=Number(/valueMax:[\w$]+=(\d+)/.exec(component.source)?.[1]);
    draft.device.sensa=Object.fromEntries(['sensaSource','applyToAllRegions','hapticIntensity','regions'].map(k=>[k,structuredClone(initial[k])]));
    const base='/device/sensa';const controls=[{path:base+'/hapticIntensity/isEnabled',label:'HAPTIC_INTENSITY',kind:'toggle'},{path:base+'/hapticIntensity/enableAudioToHaptics',label:'AUDIO_TO_HAPTICS',kind:'toggle',enabled_by:base+'/hapticIntensity/isEnabled'}];
    if(number(min)&&number(max))controls.splice(1,0,{path:base+'/hapticIntensity/value',label:'HAPTIC_INTENSITY',kind:'slider',min,max,step:1,enabled_by:base+'/hapticIntensity/isEnabled'});
    sections.push({title:'HAPTIC_INTENSITY',controls});
    sections.push({title:'INDIVIDUAL_INTENSITY',controls:[{path:base+'/applyToAllRegions',label:'APPLY_TO_OTHER_ZONES',kind:'toggle'},{path:base+'/regions',label:'RESET_ALL',kind:'reset'}]});
    const range=pg.components.flatMap(c=>c.jsx).find(j=>j.props.min===0&&j.props.max===100&&j.expressions.value?.includes('.percentage'));
    if(range)for(const [index,region] of initial.regions.entries())sections.push({title:'INDIVIDUAL_INTENSITY',suffix:' · '+region.id,controls:[{path:`${base}/regions/${index}/isEnabled`,label:'HAPTICS_ENABLED',kind:'toggle'},{path:`${base}/regions/${index}/percentage`,label:'INDIVIDUAL_INTENSITY',kind:'slider',min:0,max:100,step:1,unit:'%',enabled_by:`${base}/regions/${index}/isEnabled`}]});
   }
  }
  if(pg.key==='TAB_HOME'&&source.includes('loupedeckState'))sections.push({title:'LOUPEDECK',description:'LOUPEDECK_PROFILE_INSTRUCTION',controls:[{path:'/profile/name',label:'LAUNCH_LOUPEDECK',kind:'unavailable'}]});
  if(pg.key==='TAB_OLED'){
   for(const [field,title,description] of [['oledBrightness','OLED_BRIGHTNESS_TITLE','OLED_BRIGHTNESS_DESC'],['timeToHomeScreen','OLED_TIME_TO_HOME_SCREEN_TITLE','OLED_TIME_TO_HOME_SCREEN_DESC'],['oledDimDisplay','OLED_DIM_DISPLAY_TITLE','OLED_DIM_DISPLAY_DESC'],['oledLanguage','OLED_LANGUAGE_TITLE','OLED_LANGUAGE_TIPS']]){
    const component=pg.components.find(c=>c.source.includes(field+'Reducer')||c.source.includes('oledTimeToHomeScreenReducer')&&field==='timeToHomeScreen');
    const initial=states.find(s=>s.value[field]!==undefined)?.value[field];
    if(!component||initial===undefined)continue;
    draft.device[field]=structuredClone(initial);const base='/device/'+field,controls=[];
    if(field==='oledBrightness'){
     const min=Number(/min:[\w$]+=(\d+)/.exec(component.source)?.[1]),max=Number(/max:[\w$]+=(\d+)/.exec(component.source)?.[1]);
     if(number(min)&&number(max))controls.push({path:base,label:title,kind:'slider',min,max,step:1});
    }else if(field==='oledDimDisplay'&&Array.isArray(configs.OLED_DIM_DISPLAY_VALUES)){
     controls.push({path:base+'/value',label:title,kind:'select',enabled_by:base+'/enabled',options:configs.OLED_DIM_DISPLAY_VALUES.map(value=>({label:String(value),value}))});
    }else{
     const values=configs[field==='oledLanguage'?'OLED_LANGUAGE_VALUES':'OLED_TIME_TO_HOME_SCREEN_VALUES'];
     if(Array.isArray(values))controls.push({path:base,label:field==='oledLanguage'?'OLED_LANGUAGE_SELECT_LABEL':title,kind:'select',options:values.map(v=>({label:v.label??v.name,value:v.value}))});
    }
    if(controls.length)sections.push({title,description,controls});
   }
   gaps.push('OLED home-screen artwork/editors, screensaver previews and device update flow remain incomplete');
  }
  if(pg.key==='TAB_ENHANCEMENT'&&source.includes('noiseCancellationReducer')&&!sections.some(s=>s.title==='ACTIVE_NOISE_CANCELLATION')){
   const text=read(p.source),match=/noiseCancellationReducer:function\(\)\{let [\w$]+=arguments\.length>0&&void 0!==arguments\[0\]\?arguments\[0\]:([\w$]+)/.exec(text);
   const modes=p.arrays.find(a=>a.value.some(v=>v?.name==='ANC')&&a.value.some(v=>v?.name==='AMBIENT1'));
   if(match&&modes){
    const symbol=match[1].replace(/[$]/g,'\\$'),declarations=[...text.matchAll(new RegExp('\\b'+symbol+'=(\\{)','g'))];
    for(const declaration of declarations){
     let node;try{node=acorn.parseExpressionAt(text,declaration.index+declaration[0].length-1,{ecmaVersion:'latest'});}catch{continue;}
     const initial=literal(node),status=node.properties?.find(v=>key(v.key)==='status')?.value;
     if(typeof initial?.isEnabled!=='boolean'||!number(initial.level)||status?.type!=='Identifier')continue;
     const arrayNode=acorn.parseExpressionAt(modes.source,0,{ecmaVersion:'latest'}),index=arrayNode.elements.findIndex(v=>v.properties?.some(p=>key(p.key)==='value'&&p.value.name===status.name));
     if(index<0)continue;initial.status=modes.value[index].value;draft.device.noiseCancellation=initial;
     sections.push({title:'ACTIVE_NOISE_CANCELLATION',description:'ACTIVE_NOISE_CANCELLATION_TIP_EARBUDS',controls:[{path:'/device/noiseCancellation/isEnabled',label:'ACTIVE_NOISE_CANCELLATION',kind:'toggle'},{path:'/device/noiseCancellation/status',label:'ACTIVE_NOISE_CANCELLATION',kind:'select',enabled_by:'/device/noiseCancellation/isEnabled',options:modes.value.map(v=>({label:v.name,value:v.value}))}]});
     break;
    }
   }
  }
  if(pg.key==='TAB_LIGHTING'&&source.includes('https://www.razer.com/streamer-companion-app'))sections.push({title:'TAKE_CONTROL_EMOTES',description:'TAKE_CONTROL_EMOTES_DESC',controls:[{path:'/profile/name',label:'LEARN_MORE',kind:'link',url:'https://www.razer.com/streamer-companion-app'}]});
  if(p.product_id===1382&&pg.key==='TAB_CUSTOMIZE'&&source.includes('buttonList')){
   // This product builds the assignment selector lazily. Parse only literal
   // rows from its current bundle; never import or evaluate the bundle.
   const text=read(p.source),ast=acorn.parse(text,{ecmaVersion:'latest'});let buttons,multimedia;
   walk(ast,n=>{if(n.type!=='ArrayExpression')return;const value=literal(n);if(!Array.isArray(value))return;
    const isButtons=value.length===8&&value.every(v=>v?.inputType==='ControlPodInput'&&Array.isArray(v.functionList));
    const isMultimedia=value.length===10&&value.some(v=>v?.id==='MuteVolume')&&value.some(v=>v?.id==='MicVolumeUp');
    if(isButtons||isMultimedia){supplementary_source.push({path:p.source,offset:n.start,end:n.end,source:text.slice(n.start,n.end)});if(isButtons)buttons=value;else multimedia=value;}
   });
   if(buttons&&multimedia){
    draft.device.podEnabled={};draft.profile.podMappings={};
    const labels={ControlPodClockwise:['CLOCKWISE',''],ControlPodAnticlockwise:['COUNTER_CLOCKWISE',''],ControlPodClick:['PRESS',' × 1'],ControlPodDoubleClick:['PRESS',' × 2'],ControlPodTripleClick:['PRESS',' × 3'],ControlPodPressHold:['LONG_PRESS',''],ControlPodSourceClick:['SOURCE_BUTTON',' × 1'],ControlPodSourceDoubleClick:['SOURCE_BUTTON',' × 2']};
    for(const button of buttons){
     const original=configs.BUTTON_LIST.find(v=>v.inputID===button.inputID);if(!original)continue;
     const id=button.inputID,base='/profile/podMappings/'+id,[title,suffix]=labels[id];
     draft.device.podEnabled[id]=button.isEnabled;
     draft.profile.podMappings[id]={outputType:original.outputType,multimediaGroup:original.multimediaGroup??{multimediaAssignment:multimedia[0].id},audioGroup:original.audioGroup??{audioAssignment:'CycleDownSoundDevice',audioMode:'SwitchPlaybackDevice'}};
     const enabled='/device/podEnabled/'+id;
     const options=[{label:'MULTIMEDIA',value:'multimediaGroup'},{label:'SWITCH_PLAYBACK_DEVICE',value:'audioGroup'},{label:'DISABLE',value:'disableGroup'}].filter(v=>v.value!=='multimediaGroup'||button.functionList.includes('MULTIMEDIA'));
     sections.push({title,suffix,controls:[{path:base+'/outputType',label:'ASSIGN',kind:'select',enabled_by:enabled,options},{path:base,label:'RESET',kind:'reset',enabled_by:enabled}]});
     sections.push({title,suffix,visible_when:{path:base+'/outputType',value:'multimediaGroup'},controls:[{path:base+'/multimediaGroup/multimediaAssignment',label:'MULTIMEDIA',kind:'select',enabled_by:enabled,options:multimedia.map(v=>({label:v.content,value:v.id}))}]});
    }
    sections.push({title:'AUDIO_MODE',description:'AUDIO_MODE_CONTENT',controls:[{path:'/profile/name',label:'AUDIO_MODE_BUTTON',kind:'unavailable'},{path:'/profile/name',label:'FAQ',kind:'link',url:'https://mysupport.razer.com/app/answers/detail/a_id/13581'}]});
    gaps.push('Control Pod currently supports source multimedia, default playback cycling and disable drafts only; keyboard, mouse, macro, profile, lighting, launch, text and dial assignment editors, product artwork, tutorials and pairing remain incomplete');
   }
  }
  pages.push({key:pg.key,sections});
 }
 const aliases={SWITCH_OFF_LIGHTING_DISPLAY_OFF:'DISPLAY_TURNED_OFF',SWITCH_OFF_LIGHTING_IDLE:'IDLE_FOR_MIN',MINUTES:'IDLE_FOR_MIN',PEAKING_INDICATOR:'PEAKING_INDICATOR_TITLE',MUTE_EFFECT:'MUTE_EFFECT_TITLE',DECAY:'EFFECTS_DECAY',INTENSITY:'REVERB_INTENSITY',SMALL:'REVERB_ROOM_SMALL',LARGE:'REVERB_ROOM_LARGE',PLATE:'REVERB_ROOM_PLATE',TARGET_GAIN:'GAIN',MAKE_UP_GAIN:'GAIN',LEVEL:'GAIN'};
 const normalizations=[];
 for(const page of pages)for(const section of page.sections){section.title=aliases[section.title]??section.title;for(const control of section.controls){
  control.label=aliases[control.label]??control.label;for(const o of control.options??[])o.label=aliases[o.label]??o.label;
  const parts=control.path.split('/').slice(1);let owner=draft;for(const part of parts.slice(0,-1))owner=owner[part];const key=parts.at(-1),original=owner[key];
  if(control.kind==='slider'&&number(original)){owner[key]=Math.max(control.min,Math.min(control.max,original));}
  if(control.kind==='select'&&!control.options.some(o=>o.value===original)&&control.options.length)owner[key]=control.options[0].value;
  if(owner[key]!==original)normalizations.push({path:control.path,source_default:original,initial_draft:owner[key],reason:'Source reducer sentinel lies outside its mounted control range/options; no device response is available'});
 }}
 products.push({product_id:p.product_id,name:p.name,draft,pages,equalizers});
 coverage.push({product_id:p.product_id,status:'partial_native',pages:pages.map(pg=>({key:pg.key,sections:pg.sections.length,controls:pg.sections.reduce((n,s)=>n+s.controls.length,0),equalizers:pg.sections.filter(s=>s.equalizer).length})),default_normalizations:normalizations,supplementary_source,gaps});
 files.clear();
}
fs.writeFileSync(path.join(root,'src/features/audio_products_data.json'),JSON.stringify(products,null,2)+'\n');
fs.writeFileSync(path.join(root,'docs/re/audio-product-native-coverage.json'),JSON.stringify({schema_version:1,generator_sha256:crypto.createHash('sha256').update(fs.readFileSync(__filename)).digest('hex'),products:coverage},null,2)+'\n');
console.log(`Prepared ${products.length} source-specific audio workspaces; ${coverage.reduce((n,p)=>n+p.pages.reduce((n,p)=>n+p.controls,0),0)} controls.`);
