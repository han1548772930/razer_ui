// 1383 Razer Kraken V4 Pro OLED root: static source markers and local fingerprints.
// Parses the current device bundle as text; never executes vendor JavaScript.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const root=path.resolve(__dirname,'..'),read=p=>fs.readFileSync(path.join(root,p),'utf8');
const hash=p=>crypto.createHash('sha256').update(fs.readFileSync(path.join(root,p))).digest('hex');
const assert=(ok,message)=>{if(!ok)throw Error(message);};
const slice=(file,text,start,end)=>({path:file,offset:start,end,source:text.slice(start,end)});
const receipt=(file,text,marker,stop)=>{
  const at=text.indexOf(marker);
  assert(at>=0,'Current source no longer contains '+marker);
  return slice(file,text,at,text.indexOf(stop,at)+stop.length);
};

const js='.ref/devices/1383/static/js/6141.5d00192e.chunk.js';
const mainJs='.ref/devices/1383/static/js/main.a86f6801.js';
const chunkCss='.ref/devices/1383/static/css/6141.d8b30400.chunk.css';
const mainCss='.ref/devices/1383/static/css/main.7a0131e4.css';
const text=read(js),mainText=read(mainJs),chunk=read(chunkCss),main=read(mainCss);

/// Minified locale symbol -> key literal, from the current main bundle's export tables.
const localeKey=symbol=>{
  const long=new RegExp('[,{]'+symbol.replace(/[$]/g,'\\$')+':\\(\\)=>([\\w$]+)').exec(mainText)?.[1];
  return long?new RegExp('(?:^|[,;{])'+long.replace(/[$]/g,'\\$')+'="([A-Z][A-Z0-9_]+)"').exec(mainText)?.[1]:undefined;
};

// ── 当前源码事实 ─────────────────────────────────────────────────────────────
const root876=receipt(js,text,'xx=()=>{','var fx=s(21368)');
const widgets=receipt(js,text,'Nv=e=>{','var fx=s(21368)');
for(const marker of ['type:"warning"','width:"400px"','height:"117px"','direction:"left"','direction:"right"','min:30,minTag:30','deviceStatus:"headset"'])
  assert(root876.source.includes(marker),'OLED root no longer mounts '+marker);
for(const marker of ['DimKeyboardLighting_backdrop','DimKeyboardLighting_width-auto','OLEDLanguage_dropdown','OLEDLanguage_button','OLEDLanguage_disabled','OLEDLanguage_desc','OLEDScreensaver_oled-screensaver-option','OLEDScreensaver_selected','marginTop:"22px"','marginTop:"20px"','minTag','title:"BRIGHTNESS_HEADER"','t===i.id','value:i.value'])
  assert(widgets.source.includes(marker),'OLED widget group lost '+marker);

// Localised titles/tips for the five widgets and the wired-only warning.
const labels=Object.fromEntries([['QAd','OLED_NOT_SUPPORTED_WIRED_WARNING_TITLE'],['l2t','OLED_NOT_SUPPORTED_WIRED_WARNING_DESC'],['OFo','OLED_BRIGHTNESS_TITLE'],['yPZ','OLED_BRIGHTNESS_TIPS'],['XQ1','OLED_LANGUAGE_TITLE'],['tKs','OLED_LANGUAGE_TIPS'],['rvN','OLED_TIME_TO_HOME_SCREEN_TITLE'],['ZBh','OLED_TIME_TO_HOME_SCREEN_TIPS'],['utx','OLED_DIM_DISPLAY_TITLE'],['Iiy','OLED_DIM_DISPLAY_TIPS'],['R$0','OLED_SCREEN_SAVER_TITLE'],['$9Z','OLED_SCREEN_SAVER_TIPS']]
  .map(([symbol,key])=>{assert(localeKey(symbol)===key,symbol+' no longer resolves to '+key);return [key,{symbol,key}];}));

// `Kv` names its previews by symbol (`src:Fv`); resolve each to its media definition.
const gifs=[...new Set([...widgets.source.matchAll(/\bsrc:([\w$]+)/g)].map(m=>{
  const symbol=m[1].replace(/[$]/g,'\\$');
  return new RegExp('(?:^|[,;{])'+symbol+'=s\\.p\\+"(static\\/media\\/[^"]+)"').exec(text)?.[1];
}).filter(Boolean))];
assert(gifs.length===3&&gifs.every((gif,index)=>gif.includes('-random-sim-')&&Number(/(\d+)-random-sim/.exec(gif)?.[1])===index+1),'Screensaver previews are not the three current GIFs: '+JSON.stringify(gifs));

const cssRules={
  pollingButton:/\.customize-polling-rate-button\{([^}]*)\}/.exec(chunk)?.[1],
  widthAuto:/\.DimKeyboardLighting_width-auto__C0C3F\{([^}]*)\}/.exec(chunk)?.[1],
  languageDropdown:/\.OLEDLanguage_dropdown__xb7Ge\{([^}]*)\}/.exec(chunk)?.[1],
  languageButton:/\.OLEDLanguage_button__21pXf\{([^}]*)\}/.exec(chunk)?.[1],
  languageDisabled:/\.OLEDLanguage_disabled__a2Szl\{([^}]*)\}/.exec(chunk)?.[1],
  screensaverOptions:/\.OLEDScreensaver_oled-screensaver-options__cBAKE\{([^}]*)\}/.exec(chunk)?.[1],
  screensaverOption:/\.OLEDScreensaver_oled-screensaver-option__BONUj\{([^}]*)\}/.exec(chunk)?.[1],
  screensaverSelected:/\.OLEDScreensaver_oled-screensaver-option__BONUj\.OLEDScreensaver_selected__R\\\+IaL\{([^}]*)\}/.exec(chunk)?.[1],
  screensaverHover:/\.OLEDScreensaver_oled-screensaver-option__BONUj:hover\{([^}]*)\}/.exec(chunk)?.[1],
  track:/\.slider-container \.track\{([^}]*)\}/.exec(main)?.[1],
  tip:/\.slider-tip\{([^}]*)\}/.exec(main)?.[1],
  foot:/\.slider-container \.foot\{([^}]*)\}/.exec(main)?.[1],
};
assert(cssRules.pollingButton?.includes('#222')&&cssRules.pollingButton.includes('#5d5d5d')&&cssRules.pollingButton.includes('27px'),'Polling button CSS changed');
assert(cssRules.widthAuto?.includes('48px'),'Polling button width override changed');
assert(cssRules.languageButton?.includes('#44d62c')&&cssRules.languageButton.includes('#000'),'Language apply colours changed');
assert(cssRules.languageDisabled?.includes('opacity:.3')&&cssRules.languageDisabled.includes('pointer-events:none'),'Language disabled state changed');
assert(cssRules.screensaverOptions?.includes('grid-template-columns:1fr 1fr')&&cssRules.screensaverOptions.includes('margin-top:20px'),'Screensaver grid changed');
assert(cssRules.screensaverOption?.includes('260px')&&cssRules.screensaverOption.includes('68px'),'Screensaver tile geometry changed');
assert(cssRules.screensaverSelected?.includes('2px solid #44d62c')&&cssRules.screensaverHover?.includes('#166809'),'Screensaver selection states changed');
assert(cssRules.track?.includes('bottom:25px')&&cssRules.track.includes('height:6px')&&cssRules.tip?.includes('bottom:42px'),'Shared slider geometry changed');

// ── 本地实现 ─────────────────────────────────────────────────────────────────
const rust=read('src/features/audio_oled.rs'),parent=read('src/features/audio_products.rs'),sliderLayer=read('src/ui/source_slider.rs');
const native={
  'src/features/audio_oled.rs':hash('src/features/audio_oled.rs'),
  'src/features/audio_products.rs':hash('src/features/audio_products.rs'),
  'src/features/audio_products_data.json':hash('src/features/audio_products_data.json'),
  'tools/prepare-audio-products.cjs':hash('tools/prepare-audio-products.cjs'),
  'src/ui/source_slider.rs':hash('src/ui/source_slider.rs'),
  'src/ui/theme.rs':hash('src/ui/theme.rs'),
};
for(const marker of ['KRAKEN_OLED_LEFT_COLUMN','kraken_oled_brightness_slider','surface::css(48.)','surface::css(260.)','surface::css(68.)','0x166809','0x111111','apply_label','image_options'])
  assert(rust.includes(marker)||parent.includes(marker),'Local OLED implementation lost '+marker);
// `uo.A` 的滑条几何现在由共享的 `SourceSlider`（同一份源 CSS）承担。
for(const marker of ['if tip.is_some() { 64. } else { 36. }','surface::css(25.)','surface::css(6.)','surface::css(42.)','surface::css(8.)'])
  assert(sliderLayer.includes(marker),'Shared source slider lost '+marker);
assert(rust.includes('SourceSlider::new(state, progress).tip(Some(format!("{value:.0}")))'),'OLED brightness slider no longer mounts the shared source slider with its value tip');
assert(rust.includes('surface::page_columns()')&&rust.includes('surface::page_column('),'Local OLED page no longer uses the two source columns');
assert(parent.includes('self.spec.product_id == 1383 && key == "TAB_OLED"'),'Local OLED root is no longer dispatched');
assert(parent.includes('this.staged.insert'),'Language staging is no longer implemented');

const data=JSON.parse(read('src/features/audio_products_data.json'));
const product=data.find(p=>p.product_id===1383),page=product?.pages.find(p=>p.key==='TAB_OLED');
assert(page,'1383 TAB_OLED descriptor missing');
const byTitle=Object.fromEntries(page.sections.map(s=>[s.title,s]));
assert(Object.keys(byTitle).join(',')==='OLED_BRIGHTNESS_TITLE,OLED_LANGUAGE_TITLE,OLED_TIME_TO_HOME_SCREEN_TITLE,OLED_DIM_DISPLAY_TITLE,OLED_SCREEN_SAVER_TITLE','1383 OLED section order drifted');
assert(byTitle.OLED_BRIGHTNESS_TITLE.tips==='OLED_BRIGHTNESS_TIPS'&&byTitle.OLED_SCREEN_SAVER_TITLE.tips==='OLED_SCREEN_SAVER_TIPS','OLED widget tips drifted');
const brightness=byTitle.OLED_BRIGHTNESS_TITLE.controls[0];
assert(brightness.min===30&&brightness.max===100&&brightness.path==='/device/oledBrightness','1383 brightness bounds drifted');
assert(byTitle.OLED_LANGUAGE_TITLE.controls[0].apply_label==='APPLY','Language row lost its APPLY commit');
assert(byTitle.OLED_DIM_DISPLAY_TITLE.controls[0].enabled_by==='/device/oledDimDisplay/enabled','Dim row lost its enable binding');
assert(product.draft.device.oledScreensaver===0,'Screensaver default drifted');
const screensaver=byTitle.OLED_SCREEN_SAVER_TITLE.controls[0];
assert(screensaver.kind==='image_options'&&screensaver.options.length===4,'Screensaver tiles drifted');
for(const [index,option] of screensaver.options.entries()){
  assert(option.value===index,'Screensaver tile ids must match the reducer value');
  if(index>0)assert(option.image==='synapse/oled-screensaver-'+index+'.webp','Screensaver tile asset drifted: '+option.image);
}
const manifest=JSON.parse(read('assets/synapse/manifest.json'));
const prepared=gifs.map(gif=>manifest.entries.find(entry=>entry.source.endsWith('/'+gif)));
assert(prepared.every(Boolean)&&prepared.length===3,'Screensaver GIFs are not the prepared local assets');
const screensaverAssets=JSON.parse(read('docs/re/audio-oled-screensaver-assets.json'));
assert(hash(screensaverAssets.manifest.path)===screensaverAssets.manifest.sha256,'Current screensaver manifest changed');
assert(screensaverAssets.screensavers.length===3,'Missing current screensaver comparison');
for(const [index,receipt] of screensaverAssets.screensavers.entries()) {
  assert(receipt.tile===index+1&&receipt.source===`.ref/devices/1383/${gifs[index]}`,'Wrong product screensaver origin');
  assert(receipt.shared_source===prepared[index].source&&receipt.output===prepared[index].output,'Screensaver conversion mapping changed');
  assert(hash(receipt.source)===receipt.source_sha256&&hash(receipt.shared_source)===receipt.shared_source_sha256,'Screensaver GIF bytes changed');
  assert(fs.readFileSync(path.join(root,receipt.source)).equals(fs.readFileSync(path.join(root,receipt.shared_source))),'Current source GIF differs from prepared conversion source');
  assert(hash(receipt.output)===receipt.output_sha256,'Prepared screensaver output changed');
}

const value={
  method:'Current bundle parsed as text with literal markers, locale export resolution, CSS rule receipts and local fingerprints; no vendor code executed',
  sources:[{path:js,sha256:hash(js)},{path:mainJs,sha256:hash(mainJs)},{path:chunkCss,sha256:hash(chunkCss)},{path:mainCss,sha256:hash(mainCss)}],
  root:root876,
  widgets,
  labels,
  screensaver_previews:gifs.map((gif,index)=>({tile:index+1,source:gif,reference:prepared[index].source,output:prepared[index].output})),
  screensaver_assets:screensaverAssets,
  css:cssRules,
  native,
  contract:{
    layout:'`xx` mounts a warning banner when the headset is not on a dongle, then `body-widgets` with `col-left`(brightness, language) and `col-right`(time to home screen, dim display, screen timeout options). The local page splits the descriptor sections with the same column map.',
    brightness:'`Nv` takes min:30/minTag:30 with max 100 and step 1; the shared `uo.A` slider draws a 6px track 25px from the bottom, an 8px-inset fill, a 42px value tip and 30/100 foot marks.',
    language:'`Bv` only stages the dropdown value; `APPLY` commits, and the button carries `.OLEDLanguage_disabled` (opacity .3) while staged equals committed.',
    delay_dim:'`Ov`/`Uv` are `.polling-btn-set` rows of 48x27 `.customize-polling-rate-button.keyboard-btn-size` cells; dim adds the `#111` 300x30 `opacity:.5` backdrop while the reducer reports it disabled.',
    screensaver:'`Kv` renders a two-column grid of 260x68 tiles: tile 0 is the text tile, tiles 1-3 are the current GIFs. `SET_OLED_SCREENSAVER` stores `value+1`, so tile ids equal stored values.',
    preview_asset:'The three current 1383 manifest-declared GIFs equal the prepared conversion source GIFs byte for byte. Their WebP outputs are decoded conversions validated by the resource manifest.',
    slider_thumb:'The mounted `.slider` source thumb is a 16px circle with radius 8px. The 14x20 path SVG belongs to `.slider-more`, a different source element. No 691 implementation evidence is used.',
    home_screen:'The current 1383 reducer provides source-owned initial artwork and preview data. Seven cards and all six editable mode entries have local editors; see audio-oled-home-source.json, audio-oled-home-native.md and audio-oled-editors-2026-10-06.md.',
    remaining:'BLE/dongle service conditions, device language download, vendor animation encoding/upload and device transport remain incomplete. Local crop and profile Apply do not acknowledge a device transaction; no runtime pixels were verified',
  },
  native_verification:'Static fingerprints and descriptor assertions only; the application, builds and tests were not run',
};
const target=path.join(root,'docs/re/audio-oled-1383-current-evidence.json'),output=JSON.stringify(value,null,2)+'\n';
if(process.argv.includes('--check')){if(fs.readFileSync(target,'utf8')!==output)throw Error('Stale 1383 OLED receipt');}
else fs.writeFileSync(target,output);
console.log('Current 1383 OLED: five widgets, two columns, 48x27 button sets, 260x68 screensaver tiles and three prepared previews verified.');
