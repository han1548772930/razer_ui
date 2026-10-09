// Static data extraction only: never import or evaluate downloaded JavaScript.
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const root = path.resolve(__dirname, '..');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const hash = s => crypto.createHash('sha256').update(s).digest('hex');
const product = JSON.parse(read('crates/razer-pages/src/features/keyboard_products_data.json')).find(p => p.product_id === 691);
const page = JSON.parse(read('docs/re/keyboard-product-pages.json')).products
  .find(p => p.product_id === 691).pages.find(p => p.key === 'OLED');
const source = read(page.path);
if (hash(source) !== page.sha256) throw Error('OLED source changed');
const mainFile = product.source_files.find(f => /\/main\./.test(f.path));
const main = read(mainFile.path);
if (hash(main) !== mainFile.sha256) throw Error('OLED reducer source changed');
function proof(symbol, fragments) {
  const c = page.components.find(c => c.symbol === symbol);
  if (!c || fragments.some(s => !c.source.includes(s)) || source.slice(c.offset, c.end) !== c.source)
    throw Error('OLED component changed: ' + symbol);
  return {path: page.path, offset: c.offset, end: c.end};
}
function seed(fragment) {
  const offset = main.indexOf(fragment);
  if (offset < 0) throw Error('OLED initial state changed');
  return {path: mainFile.path, offset, fragment};
}
// Explicit literal checks keep changes in vendor defaults from silently passing.
const initial = [seed('bt={oledBrightness:50,'), seed('kt={timeToHomeScreen:5,'),
  seed('Yt={oledDimDisplay:{enabled:!0,value:1},'), seed('Ft={oledLanguage:0,oledLanguageChanged:0,')];
const brightness = proof('hi', ['void 0===t?20:t','void 0===s?100:s','step:1','SET_OLED_BRIGHTNESS']);
const home = proof('xi', ['OLED_TIME_TO_HOME_SCREEN_VALUES','SET_OLED_TIME_TO_HOME_SCREEN']);
const dim = proof('Gi', ['OLED_DIM_DISPLAY_VALUES','enabled:n,value:t','!n&&','SET_OLED_TIME_TO_DIM']);
const language = proof('wi', ['t<127?t:255&~t','d===t&&d===b','SET_OLED_LANGUAGE',
  'e.deviceReducer.isBle','"progress"===e.oledLoadingReducer.oledLoading.type']);
const screensaver = proof('ea', ['id:0,type:"text"','id:1,type:"image"','id:2,type:"image"',
  'id:3,type:"image"','SET_OLED_SCREENSAVER']);
const layout = proof('na', ['direction:"left",children:[(0,J.jsx)(hi,{}),(0,J.jsx)(wi,{})]',
  'direction:"right",children:[(0,J.jsx)(xi,{}),(0,J.jsx)(Gi,{}),(0,J.jsx)(ea,{})]']);
const homeDisplay = proof('di', ['homeScreenDisplay),x=p.enabled,b=p.selected',
  'enabled:!x,selected:b','!x&&(0,J.jsx)("div",{className:ci})',
  'id:"media",type:5,title:v.rwQ,selected:b,onEdit:t,onApply:W,requireSynapse:!0,disable:Z',
  'id:"system",type:6,title:v.FSz,selected:b,onEdit:t,onApply:W,requireSynapse:!0,disable:Z']);
initial.push(seed('homeScreenDisplay:{enabled:!0,selected:0}'));
const homeModes = [
  ['animation',0,'RG','ANIMATION'], ['image',1,'KTY','IMAGE'],
  ['emote',4,'Nj5','EMOTE'], ['banner',2,'$Mz','BANNER'],
  ['media',5,'rwQ','AUDIO_METER'], ['system',6,'FSz','SYSTEM_INFO'],
  ['keyboard',3,'lwA','KEYBOARD_INFO']
].map(([id,value,symbol,suffix]) => {
  proof('di', [`id:"${id}",type:${value},title:v.${symbol}`]);
  const label = `OLED_HOME_SCREEN_DISPLAY_TITLE_${suffix}`;
  if (!main.includes('="'+label+'"')) throw Error('Missing home mode translation');
  return {label,value,...([5,6].includes(value)?{disabled_on_ble:true}:{})};
});
initial.push(seed('Vt={oledScreensaver:0,'));
for (const key of ['OLED_BRIGHTNESS_TITLE','OLED_BRIGHTNESS_DESC','OLED_TIME_TO_HOME_SCREEN_TITLE',
  'OLED_TIME_TO_HOME_SCREEN_DESC','OLED_DIM_DISPLAY_TITLE','OLED_DIM_DISPLAY_DESC']) {
  if (!main.includes('="'+key+'"')) throw Error('Missing source translation '+key);
}
const field = 'oled';
const editorData = JSON.parse(read('crates/razer-pages/src/features/keyboard_oled_editor_data.json'));
if(editorData.source.sha256!==mainFile.sha256) throw Error('OLED editor defaults are stale');
const section = (title, description, control) => ({title, description, controls:[control]});
const output = [{product_id:691, device_fields:[field], profile:{oled:{oledBrightness:50,
  timeToHomeScreen:5,oledDimDisplay:{enabled:true,value:1},oledLanguage:0,oledScreensaver:0,
  homeScreenDisplay:{enabled:true,selected:0},...editorData.preset_defaults,media:editorData.media_default}}, pages:[{key:'OLED', sections:[
  section('OLED_BRIGHTNESS_TITLE','OLED_BRIGHTNESS_DESC',{key:'691:oled-brightness',kind:'slider',
    label:'BRIGHTNESS_HEADER',path:'/oled/oledBrightness',min:20,max:100,step:1,source:brightness}),
  {...section('OLED_LANGUAGE_TITLE','OLED_LANGUAGE_SELECT_LABEL',{key:'691:oled-language',kind:'select',
    label:'OLED_LANGUAGE_SELECT_LABEL',hide_label:true,path:'/oled/oledLanguage',apply_label:'APPLY',
    complemented_byte:true,disabled_on_ble:true,
    options:product.config.OLED_LANGUAGE_VALUES.map(({name,value}) => ({label:name,value})),source:language}),
    note:'OLED_LANGUAGE_DESC'},
  section('OLED_TIME_TO_HOME_SCREEN_TITLE','OLED_TIME_TO_HOME_SCREEN_DESC',{key:'691:oled-home',kind:'options',
    label:'OLED_TIME_TO_HOME_SCREEN_TITLE',hide_label:true,path:'/oled/timeToHomeScreen',options:product.config.OLED_TIME_TO_HOME_SCREEN_VALUES,source:home}),
  section('OLED_DIM_DISPLAY_TITLE','OLED_DIM_DISPLAY_DESC',{key:'691:oled-dim',kind:'options',label:'OLED_DIM_DISPLAY_TITLE',hide_label:true,
    path:'/oled/oledDimDisplay/value',disabled_unless:'/oled/oledDimDisplay/enabled',
    options:product.config.OLED_DIM_DISPLAY_VALUES.map(value => ({label:String(value),value})),source:dim}),
  section('OLED_SCREEN_SAVER_TITLE','OLED_SCREEN_SAVER_DESC',{key:'691:oled-screensaver',kind:'image_options',
    label:'OLED_SCREEN_SAVER_TITLE',path:'/oled/oledScreensaver',source:screensaver,options:[
      {label:'OLED_SCREEN_SAVER_OPTION_NO_SCREEN_SAVER',value:0},
      ...[1,2,3].map(value => ({label:`${value}`,value,image:`synapse/oled-screensaver-${value}.webp`}))]})
]}]}];
for (const section of output[0].pages[0].sections) {
  section.column = ['OLED_BRIGHTNESS_TITLE','OLED_LANGUAGE_TITLE'].includes(section.title) ? 'left' : 'right';
}
output[0].pages[0].sections.unshift({title:'OLED_HOME_SCREEN_DISPLAY_TITLE',
  description:'OLED_HOME_SCREEN_DISPLAY_DESC',controls:[
    {key:'691:oled-home-enabled',kind:'switch',label:'OLED_HOME_SCREEN_DISPLAY_TITLE',
      path:'/oled/homeScreenDisplay/enabled',source:homeDisplay},
    {key:'691:oled-home-mode',kind:'options',label:'OLED_HOME_SCREEN_DISPLAY_TITLE',hide_label:true,
      path:'/oled/homeScreenDisplay/selected',disabled_unless:'/oled/homeScreenDisplay/enabled',
      options:homeModes,source:homeDisplay}
    ,{key:'691:oled-presets',kind:'oled_presets',label:'OLED_HOME_SCREEN_DISPLAY_TITLE',
      path:'/oled/homeScreenDisplay/selected',disabled_unless:'/oled/homeScreenDisplay/enabled',source:homeDisplay}
  ]});
fs.writeFileSync(path.join(root,'crates/razer-pages/src/features/keyboard_oled_data.json'), JSON.stringify(output)+'\n');
fs.writeFileSync(path.join(root,'docs/re/keyboard-oled-current-evidence.json'),JSON.stringify({
  source_files:[mainFile,{path:page.path,sha256:page.sha256}],initial,layout,
  home_display:homeDisplay,status:'partial',pending:['home-screen previews, card layout and editors','language download transport and gating',
    'live device transport and download state','original layout and tooltips']},null,2)+'\n');
console.log('Generated 691 OLED: 8 device-scoped controls including the preset editor entry; full home cards and advanced editors remain pending.');
