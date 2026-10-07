// Static comparison queue for the five still-unmounted current Studio roots.
const fs=require('fs'),path=require('path');
const {Source,walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const {source:parent}=require('./audit-chroma-studio.cjs');
const root=path.resolve(__dirname,'..'),check=process.argv.includes('--check');
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
const manifestPath=parent.directory+'/asset-manifest.json',manifest=JSON.parse(read(manifestPath));
const files=[...new Set(Object.values(manifest.files).map(f=>parent.directory+'/'+f.replace(/^\.\//,'')))];
const descriptor=JSON.parse(read('src/features/chroma_studio_properties_data.json'));
const nativeFile='src/features/chroma_studio_properties.rs',native=read(nativeFile);
const parentReceipt={module:4264,symbol:'se',...parent.receipt(4264,parent.binding(4264,'se'))};
const receipts=[parentReceipt],rows=[];
for(const [name,id,rootSymbol,children,extra] of [
 ['wave',1591,'k',['S','E'],[5196,1981]],
 ['wheel',8379,'N',['C','y'],[1617,9870,1981]],
 ['tidal',8154,'C',['A','y'],[16,9741,1617,9870,1981]],
 ['audio',9120,'b',['j','C'],[9741]],
 ['chroma-generate',2474,'C',['M'],[1617,7155,8480,7226]],
]){
 const s=new Source('synapse/chroma-studio');s.module(id);
 // Some numeric constants are emitted as inert comma expressions (W[0],0).
 // Their value is the last AST operand; never evaluate the preceding code.
 const baseLiteral=s.literal.bind(s);
 s.literal=(module,node,seen)=>node?.type==='SequenceExpression'?s.literal(module,node.expressions.at(-1),seen):baseLiteral(module,node,seen);
 const current=s.receipt(id,s.module(id).fn),body=s.snippet(id,s.binding(id,rootSymbol));
 if(!parentReceipt.source.includes('bind(s,'+id+')')||!body.includes('className:"custom-'+name+'"'))throw Error('Changed lazy root '+name);
 if(native.includes('Some("'+name+'") =>'))throw Error('Native root now mounted; update this missing-UI review '+name);
 receipts.push({effect:name,module:id,symbol:'factory',...current});
 for(const symbol of children){
  if(!body.includes('.jsx)('+symbol+',')&&!body.includes('.jsxs)('+symbol+','))throw Error('No child mount '+name+':'+symbol);
  receipts.push({effect:name,module:id,symbol,...s.receipt(id,s.binding(id,symbol))});
 }
 const controls=descriptor.effects[name].controls.map(c=>({...c,native_status:'missing UI; descriptor is not a rendered control'}));
 for(const control of controls.filter(c=>['slider_stepper','number_input','angle'].includes(c.kind))){
  let verified=false;
  walk(s.module(id).fn,n=>{
   if(n.type!=='CallExpression'||n.callee.type!=='SequenceExpression'||!['jsx','jsxs'].includes(key(n.callee.expressions.at(-1)?.property)))return;
   const widget=n.arguments[0],props=n.arguments[1];
   if(widget?.type!=='MemberExpression'||props?.type!=='ObjectExpression')return;
   const binding=s.binding(id,widget.object.name),expected=control.kind==='slider_stepper'?768:2245;
   if(binding.type!=='CallExpression'||binding.arguments[0]?.value!==expected)return;
   const get=k=>props.properties.find(p=>key(p.key)===k)?.value;
   if(!get('min')||!get('max'))return;
   if(s.literal(id,get('min'))===control.min&&s.literal(id,get('max'))===control.max&&(get('step')?s.literal(id,get('step')):1)===control.step)verified=true;
  });
  if(!verified)throw Error('Range not present in current JSX '+name+'.'+control.field);
 }
 for(const module of [...new Set([9286,2904,7660,2925,...extra,...(name==='chroma-generate'?[]:[768,2245,2132]),...(['wave','wheel','audio'].includes(name)?[3690,99,1592]:[]),...(['wave','wheel','tidal'].includes(name)?[5805]:[])])]){
  const m=s.module(module),text=s.text(m.file);
  receipts.push({effect:name,module,symbol:'factory',path:m.file,sha256:hash(text),offset:m.fn.start,end:m.fn.end,source:text.slice(m.fn.start,m.fn.end)});
 }
 rows.push({effect:name,module:id,source:current.path,mount:['4264:se',id+':default',id+':'+rootSymbol],controls,native_status:'unmounted property root; all listed controls pending',full_effect_accepted:false});
}
// The center button alone is not the Wheel workflow. Include the canvas
// producer, zoom/coordinate helpers and separate params2 reducer/write leg.
const canvas=parent.binding(4264,'vt');
for(const symbol of ['editorOnMouseUp','getCursorOnCanvas','renderCenterPoint']){
 let found;walk(canvas,n=>{if(n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&key(n.left.property)===symbol)found=n.right;});
 if(!found)throw Error('Missing center point producer '+symbol);
 receipts.push({module:4264,symbol,...parent.receipt(4264,found)});
}
for(const [module,symbol]of [[4264,'pt'],[1638,'I'],[1638,'j'],[1981,'c'],[1981,'d'],[1981,'u'],[2904,'u'],[9286,'N'],[9286,'U']])receipts.push({module,symbol,...parent.receipt(module,parent.binding(module,symbol))});
const css=[],resources=new Map();
for(const file of files.filter(f=>/\/static\/css\/(?:main|EditorCanvas|285|729|510|412|474)\.[^/]+\.css$/.test(f))){
 const source=read(file),rules=parseCSS(source).filter(r=>/angle|center-point|btn-group-toggle|btn-clockwise|btn-counterclockwise|btn-outward|btn-inward|input-group-2|custom-wave|custom-wheel|custom-tidal|custom-audio|custom-chroma-generate|region-preview|chroma-generate-history|input-group-stepper|slider-stepper|panel\.right|panel-subtitle/.test(r.selector));
 css.push({path:file,sha256:hash(source),rules});
 for(const rule of rules)for(const match of rule.declarations.matchAll(/url\((?:["'])?([^)'" ]+)/g)){
  const resource=path.posix.normalize(path.posix.join(path.posix.dirname(file),match[1]));
  if(!files.includes(resource))throw Error('CSS refers to undeclared resource '+resource);
  const exists=fs.existsSync(path.join(root,resource));
  resources.set(resource,{path:resource,status:exists?'current source bytes available':'not downloaded; preparation pending',sha256:exists?hash(fs.readFileSync(path.join(root,resource))):null});
 }
}
const output={date:'2026-10-07',method:'Current manifest files parsed as AST/CSS; actual lazy mounts and JSX numeric limits checked. Native missing UI is recorded explicitly. No vendor code execution.',manifest:{path:manifestPath,sha256:hash(read(manifestPath))},native:{path:nativeFile,sha256:hash(native)},rows,receipts,css,resources:[...resources.values()],
 summary:{source_roots:rows.length,native_missing_roots:rows.length,ast_receipts:receipts.length,css_files:css.length,css_rules:css.reduce((n,c)=>n+c.rules.length,0),referenced_resources:resources.size,source_resources_missing:[...resources.values()].filter(r=>r.sha256===null).length,full_effect_acceptances:0}};
const destination='docs/re/studio-pending-properties-current-review.json',text=JSON.stringify(output,null,2)+'\n';
if(check){if(read(destination)!==text)throw Error('Stale '+destination);}else fs.writeFileSync(path.join(root,destination),text);
console.log(JSON.stringify(output.summary));
