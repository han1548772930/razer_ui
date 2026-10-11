// Read-only current product text parsing; vendor JavaScript is never evaluated.
const fs=require('fs'),crypto=require('crypto');
const products=JSON.parse(fs.readFileSync('crates/razer-pages/src/features/keyboard_products_data.json','utf8'));
let receipts=[];
for(const pid of process.argv.slice(2).map(Number)){
 const directory=`local-ui-reverse/source/official/apps.razer.com/synapse/products/${pid}/ui/static`;
 const jsName=fs.readdirSync(directory+'/js').find(name=>name.startsWith('main.'));
 const cssName=fs.readdirSync(directory+'/css').find(name=>name.startsWith('main.'));
 const js=fs.readFileSync(directory+'/js/'+jsName,'utf8'),css=fs.readFileSync(directory+'/css/'+cssName,'utf8');
 const specimen=products.find(product=>product.product_id===pid);
 const markers=['quick-bind-container','gamepad-tester-title','this.toggleButtonPanel=','className:"config-wrapper dot-bg"','className:"factory-default"','this.renderEffects=','this.renderEffect=','analogV1','AnalogGenVersion','function Hl()','ON_SET_MOD_TAP','ON_SET_SNAP_TAP','pressedKeys:t','resetActuation','Japanese','hoverGray-remappedBlack','function x_(','class x_ ','keyPressed'];
 const nodes=markers.map(marker=>{const offset=js.indexOf(marker);return{marker,byte_offset:offset<0?null:Buffer.byteLength(js.slice(0,offset)),text:offset<0?null:js.slice(Math.max(0,offset-1800),offset+2400)}});
 const conditions=[...js.matchAll(/(?:productId|pid|ProductId)\s*(?:===|==|!==|!=)\s*679|679\s*(?:===|==|!==|!=)\s*(?:[\w$.]*productId|[\w$.]*pid)/g)].map(match=>({byte_offset:Buffer.byteLength(js.slice(0,match.index)),text:js.slice(match.index-100,match.index+200)}));
 const rules=['.config-wrapper{height:385px','.config-wrapper .config-block{display:flex','.svg-key.active:not(.notRemapped)','.hyper .svg-key.active','.keyboard-svg:not(.hyper) .hoverGray-remappedBlack.isAssignment','.padding-for-wrist-rest-device','.hyper .svg-key.isAssignment','.ModTap_wrapper__','.ModTap_modTab_tip__'];
 const cssRules=rules.map(marker=>{const offset=css.indexOf(marker),end=css.indexOf('}',offset)+1;return{marker,byte_offset:offset<0?null:Buffer.byteLength(css.slice(0,offset)),text:offset<0?null:css.slice(offset,end)}});
 const assignmentCss=[...css.matchAll(/[^{}]*(?:isAssignment|padding-for-wrist-rest-device)[^{}]*\{[^}]*\}/g)].map(match=>({byte_offset:Buffer.byteLength(css.slice(0,match.index)),text:match[0]}));
 const keyColorCallers=[...js.matchAll(/keyHoverAndRemappedColor:/g)].map(match=>({byte_offset:Buffer.byteLength(js.slice(0,match.index)),text:js.slice(Math.max(0,match.index-120),match.index+180)}));
 const resources=fs.readdirSync(directory+'/media').filter(name=>/^(?:controller_icon_(?:wasd|qe)|controller-fill-(?:grey|green)-icon|icon_sidepanel(?:_a)?|icon_expand|xbox-trigger-(?:left|right)|chroma_sync_v3_static)\./.test(name)).map(name=>({source:directory+'/media/'+name,sha256:crypto.createHash('sha256').update(fs.readFileSync(directory+'/media/'+name)).digest('hex')}));
 const layoutCss=[...css.matchAll(/[^{}]*(?:body-widgets|widget-col)[^{}]*\{[^}]*\}/g)].map(match=>({byte_offset:Buffer.byteLength(css.slice(0,match.index)),text:match[0]}));
 const pageColumnCallers=[...js.matchAll(/direction:"(?:left|right)",children:/g)].map(match=>({byte_offset:Buffer.byteLength(js.slice(0,match.index)),text:js.slice(Math.max(0,match.index-160),match.index+600)}));
 receipts.push({pid,js:directory+'/js/'+jsName,js_sha256:crypto.createHash('sha256').update(js).digest('hex'),css:directory+'/css/'+cssName,css_sha256:crypto.createHash('sha256').update(css).digest('hex'),analog_gen:specimen?.config.DeviceInfo.AnalogGenVersion,explicit_679_conditions:conditions,nodes,cssRules,assignmentCss,keyColorCallers,resources,layoutCss,pageColumnCallers});
}
fs.mkdirSync('.work/keyboard-analog-callers',{recursive:true});fs.writeFileSync('.work/keyboard-analog-callers/receipts.json',JSON.stringify(receipts,null,2));
console.log(JSON.stringify({native_layouts:products.filter(product=>product.source_layout==='analog_gamepad').map(({product_id,source_layout,source_mod_tap,source_keyboard_top_padding_percent})=>({product_id,source_layout,source_mod_tap,source_keyboard_top_padding_percent}))}));
for(const receipt of receipts)console.log(JSON.stringify({pid:receipt.pid,js:receipt.js,js_sha256:receipt.js_sha256,css_sha256:receipt.css_sha256,analog_gen:receipt.analog_gen,explicit_679_conditions:receipt.explicit_679_conditions,nodes:receipt.nodes.map(({text,...node})=>node)}));
