// Current audio product-page subtrees and source-specific local control gates.
// Parse vendor bytes only; no require/import/eval of reference JavaScript.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {walk,hash}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..'),read=p=>fs.readFileSync(path.join(root,p),'utf8');
const apply=process.argv.includes('--apply'),check=process.argv.includes('--check');
const specs=JSON.parse(read('src/features/audio_products_data.json'));
const sources=[...JSON.parse(read('docs/re/audio-product-evidence.json')).products,...JSON.parse(read('docs/re/audio-additional-evidence.json')).products];
const receipts=[],gateFixes=[],products=[];
const at=(object,pointer)=>pointer.split('/').slice(1).reduce((value,key)=>value?.[key],object);
for(const spec of specs){
 const source=sources.find(p=>p.product_id===spec.product_id);
 if(!source)throw Error('Missing source '+spec.product_id);
 const needed=new Map();
 for(const page of source.pages)for(const c of page.components){
  if(!needed.has(c.path))needed.set(c.path,new Map());
  needed.get(c.path).set(`${c.offset}:${c.end}`,{...c,ast_type:null});
 }
 const fileReceipts=[];
 for(const[file,nodes]of needed){
  if(!file.startsWith(`.ref/devices/${spec.product_id}/`))throw Error('Cross-product source '+file);
  const text=read(file),digest=hash(text),declared=source.source_files.find(f=>f.path===file);
  if(!declared||declared.sha256!==digest)throw Error('Changed source '+file);
  for(const c of nodes.values())if(text.slice(c.offset,c.end)!==c.source)throw Error('Changed component '+file+':'+c.offset);
  walk(acorn.parse(text,{ecmaVersion:'latest'}),node=>{const c=nodes.get(`${node.start}:${node.end}`);if(c)c.ast_type=node.type;});
  for(const c of nodes.values())if(!c.ast_type)throw Error('Not an exact AST node '+file+':'+c.offset);
  fileReceipts.push({path:file,sha256:digest,component_count:nodes.size});
 }
 const pages=[];
 for(const native of spec.pages){
  const original=source.pages.find(p=>p.key===native.key);
  if(!original)throw Error('Missing page '+spec.product_id+'/'+native.key);
  const components=original.components.map(c=>{
   const node=needed.get(c.path).get(`${c.offset}:${c.end}`);
   return {path:c.path,offset:c.offset,end:c.end,ast_type:node.ast_type};
  });
  const roots=components.filter(c=>c.path===original.path&&c.offset>=original.offset);
  const controls=native.sections.flatMap(s=>s.controls);
  const subtree={product_id:spec.product_id,page:native.key,navigation:{path:original.path,offset:original.offset},
   roots,components,source_ast_status:'verified_current_bytes_and_exact_nodes',
   ui_review_status:'partial_subtree_review',controls:controls.map(c=>({path:c.path,kind:c.kind})),
   equalizers:native.sections.flatMap(s=>s.equalizer?[s.equalizer]:[]),
   unresolved:['Runtime branch conditions, complete interactions and visuals require further per-component review; exact AST presence alone is not completed UI.']};
  if(!roots.length)subtree.unresolved.push('No direct JSX entry receipt at/after navigation offset; root binding requires independent follow-up.');
  for(const c of original.components)for(const j of c.jsx){
   let suffix;
   if(j.props.id==='checkDisplay'&&j.expressions.disabled==='!this.props.brightnessOn')suffix='isDisplayOn';
   else if(j.props.id==='checkIdle'&&j.expressions.disabled==='!this.props.brightnessOn')suffix='isIdleEnabled';
   else if(j.expressions.value==='this.props.switchOffLighting.idleMinutes'&&j.expressions.active==='this.props.switchOffLighting.isIdleEnabled&&this.props.brightnessOn')suffix='idleMinutes';
   if(!suffix)continue;
   const nativeControl=controls.find(control=>control.path.endsWith('/switchOffLighting/'+suffix));
   if(!nativeControl)continue;
   const brightness=nativeControl.path.replace(/\/switchOffLighting\/[^/]+$/,'/brightness/isEnabled');
   if(typeof at(spec.draft,brightness)!=='boolean')throw Error('Missing brightness model '+spec.product_id);
   const present=nativeControl.enabled_by===brightness||(nativeControl.enabled_all??[]).includes(brightness);
   const binding={product_id:spec.product_id,page:native.key,path:nativeControl.path,enabled_by:brightness,
    source:{path:c.path,sha256:fileReceipts.find(f=>f.path===c.path).sha256,offset:c.offset,end:c.end},
    jsx:{offset:j.offset,props:j.props,expressions:j.expressions}};
   receipts.push(binding);
   if(!present){gateFixes.push(binding);if(apply){if(suffix==='idleMinutes')nativeControl.enabled_all=[...new Set([...(nativeControl.enabled_all??[]),brightness])];else nativeControl.enabled_by=brightness;}}
  }
  if(controls.some(c=>c.path.includes('/switchOffLighting/')))subtree.reviewed_branches=['switchOffLighting source brightness gates'];
  pages.push(subtree);
 }
 products.push({product_id:spec.product_id,name:spec.name,source_files:fileReceipts,pages});
 console.log(`Audio ${spec.product_id}: ${pages.length} pages, ${fileReceipts.length} current source files.`);
}
if(apply){
 fs.writeFileSync(path.join(root,'src/features/audio_products_data.json'),JSON.stringify(specs,null,2)+'\n');
 // Only the independently checked gates changed in the base generator. Existing
 // per-feature overlay data and coverage gaps are retained, not regenerated.
 const coverage=JSON.parse(read('docs/re/audio-product-native-coverage.json'));
 coverage.generator_sha256=hash(read('tools/prepare-audio-products.cjs'));
 fs.writeFileSync(path.join(root,'docs/re/audio-product-native-coverage.json'),JSON.stringify(coverage,null,2)+'\n');
}
if(check&&gateFixes.length)throw Error(`Missing ${gateFixes.length} source brightness gates`);
const native=['src/features/audio_products.rs','src/features/audio_products_data.json','src/features/stream_mixer.rs','src/features/control_pod_audio.rs'];
const result={method:'Current per-product SHA and exact full-file Acorn nodes; page subtrees and explicitly checked lighting gates; not complete UI acceptance',
 generator_sha256:hash(read('tools/audit-audio-review-current.cjs')),products,
 source_lighting_gates:receipts,native:native.map(path=>({path,sha256:hash(read(path))})),
 summary:{products:products.length,pages:products.reduce((n,p)=>n+p.pages.length,0),gates:receipts.length,gated_products:new Set(receipts.map(r=>r.product_id)).size}};
const serialized=JSON.stringify(result,null,2)+'\n',output='docs/re/audio-review-current-evidence.json';
if(check){if(read(output)!==serialized)throw Error('Stale '+output);}else fs.writeFileSync(path.join(root,output),serialized);
console.log(`Audio review: ${result.summary.products} products, ${result.summary.pages} pages, ${receipts.length} gates; ${gateFixes.length} missing gates ${apply?'applied':'observed'}.`);
