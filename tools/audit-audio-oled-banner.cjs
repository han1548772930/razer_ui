// Read-only static provenance/branch audit; never imports reference JavaScript.
const fs=require('fs'),path=require('path'),{spawnSync}=require('child_process');
const {hash}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..'),read=file=>fs.readFileSync(path.join(root,file),'utf8');
const flags=process.argv.slice(2);if(flags.some(f=>f!=='--check'))throw Error('Unknown argument');
const parsed=spawnSync(process.execPath,[path.join(__dirname,'prepare-audio-oled-banner.cjs'),'--check'],{cwd:root,encoding:'utf8'});
if(parsed.status!==0)throw Error(parsed.stderr||parsed.stdout);
const evidence=JSON.parse(read('docs/re/audio-oled-banner-current-evidence.json'));
const data=JSON.parse(read('src/features/audio_oled_banner_data.json'));
const assets=JSON.parse(read('assets/synapse/audio-oled-banner-assets.json'));
const sources=new Map();
function verify(receipt){
  if(!receipt.path.startsWith('.ref/devices/1383/'))throw Error('Noncurrent Banner provenance');
  if(!sources.has(receipt.path))sources.set(receipt.path,read(receipt.path));
  const text=sources.get(receipt.path);
  if(hash(text)!==receipt.sha256)throw Error(`Stale source hash ${receipt.path}`);
  if(receipt.source!==undefined&&text.slice(receipt.offset,receipt.end)!==receipt.source)throw Error(`Stale AST/CSS range ${receipt.path}`);
}
verify(evidence.manifest);
for(const receipt of [...evidence.receipts,...evidence.keyframes,...evidence.fontFaces])verify(receipt);
const iconIds=['restart','pause','increase','decrease','bold','italic','underline','left','right','up','down'];
if(iconIds.some(id=>!data.icons[id])||data.images.length!==8||data.fonts.length!==64||data.sizes.length!==14)throw Error('Incomplete source Banner choices');
const embedded=read('assets/synapse/audio-oled-banner-embedded.rs');
if(assets.length!==19)throw Error('Incomplete Banner resource set');
for(const asset of assets){
  const bytes=fs.readFileSync(path.join(root,asset.output));
  if(hash(bytes)!==asset.output_sha256||hash(fs.readFileSync(path.join(root,asset.source)))!==asset.source_sha256)throw Error(`Stale asset ${asset.id}`);
  if(!embedded.includes(`("${asset.output.slice(7)}", include_bytes!("${path.basename(asset.output)}"))`))throw Error(`Missing embedded asset ${asset.id}`);
}
const css=(part)=>evidence.css.filter(r=>r.selector.includes(part)).map(r=>r.declarations).join(';');
for(const [part,fragment]of [['text-config-block','gap:0 20px'],['banner-image-wrapper','height:20px'],['horizontal-text-wrapper','white-space:pre'],['vertical-text-wrapper','white-space:pre'],['textarea','width:385px'],['textarea','height:98px'],['config-button__','width:45px']]){
  if(!css(part).includes(fragment))throw Error(`Changed Banner CSS ${part}:${fragment}`);
}
const ua=JSON.parse(read(evidence.textareaCascade.ua_receipt));
if(hash(read(evidence.textareaCascade.ua_receipt))!==evidence.textareaCascade.ua_sha256
    || hash(read(ua.css.path))!==ua.css.sha256)throw Error('Stale UA cascade provenance');
if(data.textarea.outer_width!==391||data.textarea.outer_height!==104||data.textarea.padding!==2
    || evidence.textareaCascade.boxSizing!=='content-box')throw Error('Changed textarea cascade');
for(const file of ['locales/en.json','locales/zh-CN.json']){
  const locale=JSON.parse(read(file));for(const key of Object.values(data.labels))if(typeof locale[key]!=='string')throw Error(`Missing Banner label ${file}:${key}`);
}
const directionNames={left:'scroll-horizontal-left-to-right',right:'scroll-horizontal-right-to-left',up:'scroll-vertical-top-to-bottom',down:'scroll-vertical-bottom-to-top'};
const directions=Object.entries(directionNames).map(([value,name])=>{
  const keyframe=evidence.keyframes.find(k=>k.source.startsWith(`@keyframes CustomizeBanner_${name}__`));
  if(!keyframe)throw Error(`Missing source keyframes ${value}`);
  return {value,source:keyframe};
});
const output={product_id:1383,method:'Static AST/CSS/resource/locale validation only; no application or tests executed.',
  fonts:data.fonts.map(n=>n.name),sizes:data.sizes.map(n=>n.name),images:data.images.map(i=>i.id),icons:iconIds,
  limits:data.limits,textarea:data.textarea,ua_receipt:evidence.textareaCascade.ua_receipt,
  motion:{...data.motion,horizontal_duration_dependencies:['text.value','text.scroll'],cache:'local_duration_ms; zero means not yet measured; Home and editor Hg have independent mount measurements'},
  directions,files:[...sources].map(([path,text])=>({path,sha256:hash(text)})),
  native:['src/features/audio_oled_banner.rs','src/features/audio_oled_banner_theme.rs'].map(path=>({path,sha256:hash(read(path))}))};
const destination='docs/re/audio-oled-banner-static-audit.json',serialized=JSON.stringify(output,null,2)+'\n';
if(flags.includes('--check')){if(read(destination)!==serialized)throw Error('Stale Banner static audit');}else fs.writeFileSync(path.join(root,destination),serialized);
console.log('1383 Banner: source branches, 64 fonts, 14 sizes, 19 resources, four keyframes and native provenance validated.');
