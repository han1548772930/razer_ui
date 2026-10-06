// Current product 70 DPI editor: syntax and CSS receipts only, never execute JS.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {walk,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),directory='.ref/devices/70';
const sourcePath=directory+'/static/js/main.8f24b6a1.js';
const source=fs.readFileSync(path.join(root,sourcePath),'utf8');
const sha256=hash(source);
if(sha256!=='be0816be63fe674ea9fc106cbd55f5aabd509e1ff36aaf66e6c002d9ecd1f4b5')throw Error('Product 70 source changed; re-audit its mounted DPI chain');
const wanted=new Set(['yT','XT','jT','EI','cI','uI','zI','TI','II','zE']);
const receipts=[];
walk(acorn.parse(source,{ecmaVersion:'latest'}),node=>{
 if(node.type==='Property'&&node.key?.value===4230&&node.start===185084){
  receipts.push({symbol:'module:4230',path:sourcePath,sha256,offset:node.start,end:node.end,source:source.slice(node.start,node.end)});
 }
 if(node.start<4150000||!['ClassDeclaration','VariableDeclarator'].includes(node.type)||!wanted.has(node.id?.name))return;
 receipts.push({symbol:node.id.name,path:sourcePath,sha256,offset:node.start,end:node.end,source:source.slice(node.start,node.end)});
});
for(const name of wanted)if(receipts.filter(r=>r.symbol===name).length!==1)throw Error('Ambiguous current DPI symbol '+name);
if(receipts.filter(r=>r.symbol==='module:4230').length!==1)throw Error('Missing current numeric editor');
const manifestPath=directory+'/asset-manifest.json',manifestBytes=fs.readFileSync(path.join(root,manifestPath));
const manifest=JSON.parse(manifestBytes);
const files=[...new Set(Object.values(manifest.files))].filter(file=>file.includes('static/')).map(file=>directory+'/'+file.slice(file.indexOf('static/')));
if(!files.includes(sourcePath))throw Error('Audited entry no longer declared by current manifest');
const css=[];
for(const file of files.filter(file=>file.endsWith('.css'))){
 const text=fs.readFileSync(path.join(root,file),'utf8');
 const rules=parseCSS(text).filter(r=>/stage|sensitivity|dpi|draggable|drag-image|stepper|spinner/.test(r.selector));
 if(rules.length)css.push({path:file,sha256:hash(text),rules});
}
const assetNames=[['stepper_up.svg','stepper-up.svg'],['stepper_down.svg','stepper-down.svg'],['icon_sensitivity_xy.svg','sensitivity-xy.svg'],['icon_sensitivity_xy_active.svg','sensitivity-xy-active.svg'],['icon_sensitivity_xy_disabled.svg','sensitivity-xy-disabled.svg'],['icon_draggable_large.svg','dpi-draggable.svg'],...['red','green','blue','cyan','yellow'].map((name,index)=>[name+'.svg',`stage-${index+1}.svg`])];
const base=JSON.parse(fs.readFileSync(path.join(root,directory,'index.html.http.json'),'utf8')).source_url.replace(/index\.html$/,'');
const assets=assetNames.map(([name,output])=>{
 const relative=manifest.files['static/media/'+name];
 if(!relative)throw Error('Missing current DPI resource '+name);
 const input=directory+'/'+relative.replace(/^\.\//,''),target='assets/synapse/'+output;
 const bytes=fs.readFileSync(path.join(root,input));
 if(!bytes.equals(fs.readFileSync(path.join(root,target))))throw Error('Shared DPI asset differs from current 70 bytes: '+target);
 return {source:input,source_url:new URL(relative,base).href,output:target,sha256:hash(bytes)};
});
const result={product_id:70,method:'Acorn AST and static CSS extraction only; no reference JavaScript execution',manifest:{path:manifestPath,sha256:hash(manifestBytes)},receipts,css,assets,
 semantics:{mount:'zI mounts uI/cI without useTwoWayTab',disabled_stages:'cI passes only dpiStages[activeStage-1]; current row remains editable',rows:'EI mounts XT via jT for each row; ordinal/visibility and selectedStage must remain distinct',reorder:'EI onDrop moves originalOrder[draggedFrom] to draggedTo, remaps selectedStage when inside the moved interval, then cI.updateStages repairs a hidden selected row',axis_edit:'cI activates edited visible rows; linked X also writes Y; disabling independent Y restores Y=X'},
 native_status:'Current native 70 renderer mounts multiple editable rows, visibility and drag/reorder with selected-stage remapping. XY reset follows cI/II/TI/zE including hidden-stage selection only when X differs from Y. Hover-only actions and directional insertion borders are mounted. Native input/slider/switch skin, exact drag preview and remaining source edge cases are not complete; no runtime or pixel verification.'};
const file=path.join(root,'docs/re/mouse-70-dpi-current-evidence.json'),text=JSON.stringify(result,null,2)+'\n';
if(process.argv.includes('--check')){if(fs.readFileSync(file,'utf8')!==text)throw Error('Stale DPI receipts');}else fs.writeFileSync(file,text);
console.log(`Product 70 DPI: ${receipts.length} current AST receipts, ${css.length} CSS files; native editor remains incomplete.`);
