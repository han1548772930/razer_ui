// Current Studio modules are parsed as inert AST data, never imported/executed.
const fs=require('fs'),path=require('path');
const {Source,walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const {source:parent}=require('./audit-chroma-studio.cjs');
const root=path.resolve(__dirname,'..'),check=process.argv.includes('--check');
const read=f=>fs.readFileSync(path.join(root,f),'utf8');
const manifestPath=parent.directory+'/asset-manifest.json',manifest=JSON.parse(read(manifestPath));
const declared=new Set(Object.values(manifest.files).map(f=>parent.directory+'/'+f.replace(/^\.\//,'')));
const parentReceipt={module:4264,symbol:'se',...parent.receipt(4264,parent.binding(4264,'se'))};
if(!parentReceipt.source.includes('disabled:!t&&["select","move"].includes(e)'))throw Error('Changed device/tool condition');
const definitions=[
 {name:'reactive',module:7518,root:'p',children:['g','A'],duration:'MB',color:'color',random:true,playback:false},
 {name:'ripple',module:5305,root:'b',children:['j','N'],gradient:'default',numeric:[['speed','Mn','s8',null],['width','jw','Ph','h1']],playback:true},
 {name:'starlight',module:6548,root:'j',children:['C','N'],gradient:'default',duration:'f5',random:true,numeric:[['density','dV','JG',null]],playback:false},
];
const rows=[],receipts=[parentReceipt];
const derived=JSON.parse(read('crates/razer-pages/src/features/chroma_studio_data.json'));
const numeric=[];
for(const def of definitions){
 // Separate Source instances prevent another lazy root from silently supplying
 // a different copy of a duplicated shared module.
 const s=new Source('synapse/chroma-studio');s.module(def.module);
 const full=s.receipt(def.module,s.module(def.module).fn);
 if(!parentReceipt.source.includes('bind(s,'+def.module+')'))throw Error('Missing current lazy mount '+def.name);
 if(!full.source.includes('className:"custom-'+def.name+'"'))throw Error('Wrong root class');
 const rootBody=s.snippet(def.module,s.binding(def.module,def.root));
 for(const symbol of def.children){
  if(!rootBody.includes('.jsx)('+symbol+','))throw Error('Missing child '+symbol);
  receipts.push({effect:def.name,module:def.module,symbol,...s.receipt(def.module,s.binding(def.module,symbol))});
 }
 receipts.push({effect:def.name,module:def.module,symbol:'factory',...full});
 const c=k=>s.literal(6257,s.exported(6257,k));
 const row={...def,mount:['4264:se',def.module+':default',def.module+':'+def.root],source:full.path,controls:[],status:'mounted-native-local-controls; runtime and full shared-widget parity pending'};
 if(def.duration){
  const values=c(def.duration);
  if(JSON.stringify(values)!==JSON.stringify(derived.effects.find(e=>e.name===def.name).duration_values))throw Error('Stale duration values');
  row.controls.push({kind:'duration',milliseconds:values,preview_module:1991});
 }
 if(def.color)row.controls.push({kind:'color-dropdown',field:def.color,disabled_when:'randomColor',preserves_black:true,no_FU_fallback:true});
 if(def.name==='reactive'&&(!full.source.includes('({color:e.color})')||full.source.includes('.FU')))throw Error('Changed Reactive color conversion');
 if(def.gradient){
  const presets=c('dp');
  if(JSON.stringify(presets)!==JSON.stringify(derived.gradients.default.presets))throw Error('Stale default gradient');
  row.controls.push({kind:'gradient',variant:'default',presets:presets.length,max_stops:presets[0].length,min_stops:1,canvas_width:180,disabled_when:def.random?'randomColor':null});
 }
 if(def.random)row.controls.push({kind:'checkbox',field:'randomColor',module:9741});
 if(def.playback)row.controls.push({kind:'playback',module:5805});
 for(const [field,minKey,maxKey,stepKey] of def.numeric??[]){
  const limits={effect:def.name,field,min:c(minKey),max:c(maxKey),step:stepKey?c(stepKey):1};
  let verified=false;
  walk(s.module(def.module).fn,n=>{
   if(n.type!=='CallExpression'||n.callee.type!=='SequenceExpression'||!['jsx','jsxs'].includes(key(n.callee.expressions.at(-1)?.property)))return;
   const widget=n.arguments[0],props=n.arguments[1];
   if(widget?.type!=='MemberExpression'||props?.type!=='ObjectExpression')return;
   const binding=s.binding(def.module,widget.object.name);
   if(binding?.type!=='CallExpression'||binding.arguments[0]?.value!==768)return;
   const get=name=>props.properties.find(p=>key(p.key)===name)?.value;
   if(s.literal(def.module,get('min'))===limits.min&&s.literal(def.module,get('max'))===limits.max&&(get('step')?s.literal(def.module,get('step')):1)===limits.step)verified=true;
  });
  if(!verified)throw Error('Numeric range missing in actual root JSX '+field);
  numeric.push(limits);row.controls.push({kind:'number-and-range',...limits});
 }
 // Record every mounted shared widget and every helper relevant to its owner.
 const shared=[99,1592,2925,9741,1698,9170,1991,7660];
 if(def.color)shared.push(16);
 if(def.gradient)shared.push(3690,3968,9220);
 if(def.numeric)shared.push(768,2245,2132);
 if(def.playback)shared.push(5805,126);
 for(const id of [...new Set(shared)]){
  const m=s.module(id),text=s.text(m.file);
  receipts.push({effect:def.name,module:id,symbol:'factory',path:m.file,sha256:hash(text),offset:m.fn.start,end:m.fn.end,source:text.slice(m.fn.start,m.fn.end)});
 }
 for(const constant of [def.duration,...(def.numeric??[]).flatMap(n=>n.slice(1)),...(def.gradient?['dp','GB']:[])].filter(Boolean)){
  let node=s.exported(6257,constant);if(node.type==='Identifier')node=s.binding(6257,node.name);
  receipts.push({effect:def.name,module:6257,symbol:constant,...s.receipt(6257,node)});
 }
 rows.push(row);
}
const numericSemantics={
 source_input:'2245 uses type=number. With step>=1, keydown prevents ., +, -, lowercase e; paste/uppercase E can still supply a valid fractional/exponent number. onInput edits a local n but never assigns it back to target.value.',
 change:'5305/6548 convert the actual input value with Number before changing the working field; fractional/exponent values are not rounded at this stage.',
 blur:'7660:x5 clamps to min/max, then Math.round(value/step)*step and two-decimal normalization.',
 cases:[{field:'speed',input:'1e1',change:10,blur:10},{field:'speed',input:'1.5',change:1.5,blur:2},{field:'width',input:'1.5e2',change:150,blur:200},{field:'width',input:'',change:0,blur:100},{field:'density',input:'-1',change:-1,blur:1}],
 validation:'Source semantics re-read statically. These are review vectors, not executed Rust/window tests.'
};
const css=[];
for(const file of declared){
 if(!/\/static\/css\/(?:main\.|606\.|42\.|255\.|813\.)[^/]+\.css$/.test(file))continue;
 const source=read(file);
 const rules=parseCSS(source).filter(r=>/panel-subtitle|panel\.right|input-label|input-group|input-stepper|slider|dropdown-selector|duration-preview|custom-reactive|custom-starlight|checkbox|gradient-picker|thumb-gradient/.test(r.selector));
 css.push({path:file,sha256:hash(source),rules});
}
if(!css.some(c=>c.rules.some(r=>r.selector==='.duration-preview.moderate.reactive')))throw Error('Missing current Reactive preview CSS');
const assetRows=JSON.parse(read('assets/synapse/chroma-studio-assets.json')).filter(a=>/^(reactive-white|ripple-white|starlight-white|wave-(?:lg|md|sm)-gray|wave-reactive-(?:md|sm)-gray)$/.test(a.id));
for(const a of assetRows){
 if(!declared.has(a.source))throw Error('Undeclared asset '+a.source);
 if(hash(fs.readFileSync(path.join(root,a.source)))!==a.source_sha256||hash(fs.readFileSync(path.join(root,a.output)))!==a.output_sha256)throw Error('Changed current resource '+a.id);
}
const output={date:'2026-10-07',method:'Separate current lazy-root source instances, AST mount/control/range parsing and current manifest CSS/resource validation; no vendor code execution',manifest:{path:manifestPath,sha256:hash(read(manifestPath))},rows,numeric,numeric_semantics:numericSemantics,receipts,css,assets:assetRows,
 boundaries:['Properties use current-layer working parameters. No device selection/read/preview/apply is fabricated.','Select/Move with no real selected devices remains disabled; Pen/Bucket support local parameter editing.','Device mixed values, actual read publisher, shared popup edge behavior and full visual/input/animation acceptance remain pending.'],
 summary:{effect_roots:rows.length,ast_receipts:receipts.length,css_files:css.length,css_rules:css.reduce((n,c)=>n+c.rules.length,0),assets:assetRows.length,full_effect_acceptances:0}};
const destination='docs/re/studio-reactive-ripple-starlight-current-evidence.json',encoded=JSON.stringify(output,null,2)+'\n';
if(check){if(read(destination)!==encoded)throw Error('Stale '+destination);}else fs.writeFileSync(path.join(root,destination),encoded);
console.log(JSON.stringify(output.summary));
