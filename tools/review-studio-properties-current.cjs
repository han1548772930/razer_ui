// Current property contract receipts. Parse vendor factories/CSS; never execute them.
const fs=require('fs'),path=require('path');
const {Source,walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),s=new Source('synapse/chroma-studio');
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
const requireValue=(value,message)=>{if(!value)throw Error(message);return value;};
const receipts=[];
for(const [module,symbol] of [[9120,'C'],[8379,'y'],[8154,'y'],[1591,'E']])
 receipts.push({module,symbol,...s.receipt(module,s.binding(module,symbol))});
for(const module of [768,2245,1617,5196,9741,7660,9286])
 receipts.push({module,symbol:'factory',...s.receipt(module,s.module(module).fn)});
// Keep the source of the remaining angle, center-point and media workflows
// without copying an obsolete claim that their property roots are unmounted.
const remainingReceipts=[];
for(const module of [1591,2474,1981,2904,7155,8480,7226])
 remainingReceipts.push({module,symbol:'factory',...s.receipt(module,s.module(module).fn)});
for(const [module,symbol] of [[4264,'se'],[4264,'pt'],[1638,'I'],[1638,'j'],[9870,'d']])
 remainingReceipts.push({module,symbol,...s.receipt(module,s.binding(module,symbol))});
for(const symbol of ['editorOnMouseUp','getCursorOnCanvas','renderCenterPoint']){
 let found;walk(s.binding(4264,'vt'),node=>{
  if(node.type==='AssignmentExpression'&&node.left.type==='MemberExpression'&&key(node.left.property)===symbol)found=node.right;
 });
 requireValue(found,'Missing center point source '+symbol);
 remainingReceipts.push({module:4264,symbol,...s.receipt(4264,found)});
}
const property=(node,name)=>node?.properties?.find(p=>p.type==='Property'&&key(p.key)===name)?.value;
const objects=module=>{const list=[];walk(s.module(module).fn,n=>{if(n.type==='ObjectExpression')list.push(n);});return list;};
for(const module of [8379,8154]){
 const group=requireValue(objects(module).find(n=>property(n,'className')?.value==='btn-group-toggle'),'Missing direction group');
 const handler=property(group,'onClick');
 requireValue(handler&&/counterclockwise:!/.test(s.snippet(module,handler)),'Group no longer toggles direction');
 const children=property(group,'children');
 requireValue(children?.type==='ArrayExpression'&&children.elements.length===2,'Direction must have two source children');
 for(const child of children.elements)requireValue(!property(child.arguments[1],'onClick'),'Direction child gained independent action');
}
const audio=receipts.find(r=>r.module===9120).source;
requireValue(audio.includes('disabled:t')&&audio.includes('checked:t')&&audio.includes('autoBoost:t.checked'),'Changed Audio auto control');
const wave=receipts.find(r=>r.module===1591).source;
requireValue(wave.includes('pause:1e3*Number(t.value)')&&wave.includes('t/1e3'),'Changed Pause unit conversion');
requireValue(wave.includes('className:"switch-container"')&&wave.includes('split:t.checked'),'Changed Split switch');
const composition=requireValue(objects(768).find(n=>property(n,'className')?.value==='input-group-stepper'),'Missing numeric group');
const columns=property(composition,'children');
requireValue(columns?.type==='ArrayExpression'&&columns.elements.length===2
 &&property(columns.elements[1].arguments[1],'children')?.type==='Identifier','Changed numeric right column');
const manifestPath=s.directory+'/asset-manifest.json',manifest=JSON.parse(read(manifestPath));
const css=[];
for(const f of [...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css'))){
 const file=s.directory+'/'+f.slice(2),text=read(file);
 const rules=parseCSS(text).filter(r=>/btn-group-toggle|input-group-stepper|custom-audio.*labels|switch-razer|input-group-2|angle|center-point|custom-chroma-generate|region-preview|chroma-generate-history/.test(r.selector));
 if(rules.length)css.push({path:file,sha256:hash(text),rules});
}
const svg=JSON.parse(read('assets/synapse/chroma-studio-assets.json'))
 .filter(r=>/^(?:clockwise|counterclockwise|outward|inward)-(?:gray|black)$/.test(r.id));
requireValue(svg.length===8,'Missing direction state icons');
for(const asset of svg){
 requireValue(hash(fs.readFileSync(path.join(root,asset.source)))===asset.source_sha256,'Changed direction source icon');
 requireValue(hash(fs.readFileSync(path.join(root,asset.output)))===asset.output_sha256,'Changed embedded direction icon');
}
const evidence={method:'Current AST/CSS and original SVG bytes; no vendor execution. Native behavior is covered by compile-only test-support.',
 manifest:{path:manifestPath,sha256:hash(read(manifestPath))},receipts,remaining_source_receipts:remainingReceipts,css,svg,
 contract:{audio_auto:'Auto is the right column of Boost input-group-stepper; it disables Boost input/range, not itself.',
  wave_pause:'Number input alongside Speed; seconds in UI, multiplied by 1000 in working params; no Pause slider.',
  wave_split:'Source switch alongside Width input, label above, 32x18 track and 14px thumb.',
  direction:'Wheel/Tidal either half or group click toggles params2.counterclockwise, including the selected half; two icon halves share one 104x30 minimum group.'}};
const destination='docs/re/studio-properties-current-evidence.json',output=JSON.stringify(evidence,null,2)+'\n';
if(process.argv.includes('--check'))requireValue(read(destination)===output,'Stale '+destination);
else fs.writeFileSync(path.join(root,destination),output);
console.log(`Studio properties: ${receipts.length} repaired-contract AST receipts, ${remainingReceipts.length} remaining-work AST receipts, ${css.length} CSS sheets, ${svg.length} direction SVGs.`);
