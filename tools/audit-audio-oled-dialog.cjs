// Current 1383 shell and Requires Synapse portal: data-only AST/CSS audit.
const fs=require('fs'), path=require('path');
const {Source,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'), dir='.ref/devices/1383';
const read=f=>fs.readFileSync(path.join(root,f),'utf8');
const manifest=JSON.parse(read(`${dir}/asset-manifest.json`));
const files=[...new Set(Object.values(manifest.files))];
const source=Object.create(Source.prototype);
source.directory=dir;
source.files=files.filter(f=>f.includes('/static/js/')&&f.endsWith('.js')).map(f=>`${dir}/${f.slice(f.indexOf('static/'))}`);
source.modules=new Map(); source.texts=new Map(); source.parsed=new Set();
const receipts=[[51278,'Tn'],[51278,'Mv'],[51278,'oh'],[58837,'d'],[58837,'p'],[58837,'h']].map(([id,symbol])=>({module:id,symbol,...source.receipt(id,source.binding(id,symbol))}));
receipts.push({module:58837,symbol:'tooltip_module',...source.receipt(58837,source.module(58837).fn)});
const requireSource=(symbol,fragments)=>{
 const r=receipts.find(r=>r.symbol===symbol);
 for(const fragment of fragments)if(!r.source.includes(fragment))throw Error(`${symbol} contract changed: ${fragment}`);
};
requireSource('Mv',['["animation","image","emote"].includes(t)?"800px":"850px"','overflow:"visible"','"emote"===t?{overflowY:"hidden"}:{overflowY:"scroll"}']);
requireSource('p',['x:n.left-320+n.width','y:n.top']);
requireSource('d',['"bottom-right"']);
requireSource('tooltip_module',['h.defaultProps={position:d,className:""}']);
requireSource('h',['e.right>window.innerWidth-8','t.style.left+window.innerWidth-8-e.right','e.left<8','t.style.left+8-e.left','window.addEventListener("resize",this.handleWindowResize)']);
const css=files.filter(f=>f.includes('/static/css/')&&f.endsWith('.css')).flatMap(f=>{
 const file=`${dir}/${f.slice(f.indexOf('static/'))}`,text=read(file);
 return parseCSS(text).filter(r=>r.selector==='div'||/^(\.choose-a-mat(?:$|[ ,.])|\.popup-widget \.backdrop|\.backdrop(?:$|[ ,.])|\.tooltip-razer|\.DisplayWidget_tooltip|\.CustomizeModal_|\.CustomizeAnimation_tooltip|\.CustomizeImage_tooltip)/.test(r.selector))
  .map(r=>({path:file,sha256:hash(text),...r}));
});
for(const [selector,property,value] of [
 ['div','box-sizing','border-box'],
 ['.choose-a-mat','min-width','800px'],['.choose-a-mat','transition','top .3s'],
 ['.backdrop.show .choose-a-mat','top','100px'],
 ['.popup-widget .backdrop .choose-a-mat','margin-top','110px'],
 ['.choose-a-mat .head','height','36px'],['.choose-a-mat .head','box-shadow','0 1px 0 0 #5d5d5d'],
 ['.popup-widget .backdrop .choose-a-mat .head','padding','20px 0 10px'],
 ['.tooltip-razer>.main','width','300px'],['.tooltip-razer>.main','transition','opacity .1s linear'],
 ['.DisplayWidget_tooltip__j4KNA','margin-left','285px'],['.DisplayWidget_tooltip__j4KNA','margin-top','-3px'],
 ['.CustomizeAnimation_tooltip__WEWaQ','margin-left','270px'],['.CustomizeImage_tooltip__haCIB','margin-left','270px'],
])if(!css.some(r=>r.selector===selector&&r.properties.some(p=>p.property===property&&p.value===value)))throw Error(`Missing CSS contract ${selector}: ${property}:${value}`);
const local=['crates/razer-pages/src/features/audio_oled_dialog.rs','crates/razer-pages/src/features/audio_oled_tooltip.rs','crates/razer-pages/src/features/audio_oled_media.rs'].map(file=>({path:file,sha256:hash(read(file))}));
const report={product_id:1383,method:'Static Acorn declarations and CSS only; no vendor execution. Offsets are UTF-16 code units.',manifest:{path:`${dir}/asset-manifest.json`,sha256:hash(read(`${dir}/asset-manifest.json`))},receipts,css,local,
 review:{dialog:'Retained focus capture/return, source immediate unmount, 100ms backdrop/300ms top/200ms close background. Width branch remains per editor. Native Base Dialog owns modal semantics.',tooltip:'300px main width measured for collision; p plus DisplayWidget offsets(285,-3) or Artwork offsets(270,0); defaultProps bottom-right; right then left 8px guard; no vertical flip; viewport portal layer1060; 100ms mount fade. Native Tooltip owns accessible role.'}};
const flags=process.argv.slice(2);if(flags.some(f=>f!=='--check'))throw Error('Unknown argument');
const file='docs/re/audio-oled-dialog-current-evidence.json',out=JSON.stringify(report,null,2)+'\n';
if(flags.includes('--check')){if(read(file)!==out)throw Error(`Stale ${file}`);}else fs.writeFileSync(path.join(root,file),out);
console.log('1383 OLED shared dialog and Requires Synapse portal: current AST/CSS contracts validated.');
