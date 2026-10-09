// Current source `OTA` slider (`.slider-container`): declarations, product CSS
// receipts and the shared local layer's fingerprints. Parses the current
// stylesheets as text and never executes vendor JavaScript.
const fs=require('node:fs'),path=require('node:path'),crypto=require('node:crypto');
const acorn=require('acorn');
const {parseCSS}=require('./css-source.cjs');
const {walk}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..'),read=p=>fs.readFileSync(path.join(root,p),'utf8');
const hash=p=>crypto.createHash('sha256').update(fs.readFileSync(path.join(root,p))).digest('hex');
const assert=(ok,message)=>{if(!ok)throw Error(message);};

/// Exact parsed selectors; offsets use JavaScript UTF-16 code units.
const rules=(css,selector)=>{
  return parseCSS(css).filter(rule=>!rule.conditions.length&&rule.selector===selector)
    .map(rule=>({selector,offset:rule.offset,text:`${rule.selector}{${rule.declarations}}`}));
};
/// Declarations of a rule body, keyed by property, so prefixed or reordered
/// output from a different bundle still matches on the audited values.
const declarations=rule=>{
  const body=rule.text.slice(rule.text.indexOf('{')+1,-1),map={};
  for(const part of body.split(';')){
    const colon=part.indexOf(':');
    if(colon>0) map[part.slice(0,colon).trim()]=part.slice(colon+1).trim();
  }
  return map;
};
const carries=(css,selector,expected)=>{
  const found=rules(css,selector);
  assert(found.length>0,`No ${selector} rule`);
  const matched=found.find(rule=>{
    const declared=declarations(rule);
    return Object.entries(expected).every(([property,value])=>declared[property]===value);
  });
  assert(matched,`Different current declaration: ${selector} ${JSON.stringify(expected)}`);
  return matched;
};

// 3858/3880 monitors, 3893/3900/3907/3921 cooling, and the 1383 OLED page all
// mount the same `OTA` slider; each bundle repeats the declarations below.
const products=['3858','3880','3893','3900','3907','3921','1383','1303','1304'];
const sources=[];
const components=[];
for(const pid of products){
  const manifestPath=`.ref/devices/${pid}/asset-manifest.json`,manifest=JSON.parse(read(manifestPath));
  const declared=Object.values(manifest.files);
  const local=value=>{
    assert(typeof value==='string'&&value.includes('static/'),`Not a static manifest entry: ${pid}/${value}`);
    const relative=value.slice(value.indexOf('static/'));
    assert(!relative.includes('..'),`Unsafe manifest entry ${value}`);
    return `.ref/devices/${pid}/${relative}`;
  };
  const file=local(manifest.files['main.css']);
  sources.push({product:pid,file,manifest:{path:manifestPath,sha256:hash(manifestPath)}});
  // Resolve the actual range component, including lazy bundles for 1383/3921.
  // No minified symbol from another product is reused as evidence.
  const matches=[];
  for(const value of [...new Set(declared)].filter(value=>/static\/js\/.*\.js$/.test(value))){
    const jsFile=local(value),text=read(jsFile);
    if(!text.includes('slider-container '))continue;
    walk(acorn.parse(text,{ecmaVersion:'latest'}),node=>{
      if(!['ClassExpression','ClassDeclaration'].includes(node.type))return;
      const render=node.body.body.find(method=>(method.key?.name??method.key?.value)==='render');
      if(!render||!/["'`]slider-container /.test(text.slice(render.start,render.end)))return;
      const source=text.slice(node.start,node.end);
      // Some audio pages also use the container for their distinct game/chat
      // slider. Only the value-tip/fill component is shared by this renderer.
      if(!source.includes('type:"range"')||!source.includes('updateValue')||!source.includes('getPercent'))return;
      matches.push({path:jsFile,sha256:hash(jsFile),symbol:node.id?.name??null,offset:node.start,end:node.end,source});
      return false;
    });
  }
  assert(matches.length,`No manifest-declared range component for ${pid}`);
  components.push({product:pid,range_components:matches});
}

const expected={
  '.slider-container':{'height':'64px','opacity':'.3','pointer-events':'none','position':'relative','transition':'opacity .3s'},
  '.slider-container.on':{'opacity':'1','pointer-events':'auto'},
  '.slider-container.no-tip':{'height':'36px'},
  '.slider':{'background':'#0000','border-radius':'3px','bottom':'25px','height':'6px','width':'100%'},
  '.slider::-webkit-slider-thumb':{'background':'#44d62c','border-radius':'8px','height':'16px','width':'16px','transition':'transform .2s,background .3s'},
  '.slider-container .track':{'background':'#44d62c4d','border-radius':'3px','bottom':'25px','height':'6px'},
  '.slider-tip':{'background-color':'#44d62c','border-radius':'3px','bottom':'42px','line-height':'14px','padding':'4px 8px'},
  '.slider-tip,.thumb-tag':{'color':'#212121','font-size':'12px','position':'absolute'},
};
const receipts=[];
for(const {product,file,manifest} of sources){
  const css=read(file),record={product,manifest,path:file,sha256:hash(file),rules:[]};
  for(const [selector,declar] of Object.entries(expected)){
    const rule=carries(css,selector,declar);
    record.rules.push({selector,offset:rule.offset,text:rule.text});
  }
  // The two pointer states sit behind `.slider-container.on` on the input.
  for(const [state,background] of [['hover','#5d5d5d'],['active','#383838']]){
    const rule=carries(css,`.slider-container.on .slider::-webkit-slider-thumb:${state}`,{background,'border':'2px solid #44d62c'});
    record.rules.push({selector:`.slider-container.on .slider::-webkit-slider-thumb:${state}`,offset:rule.offset,text:rule.text});
  }
  // The fill's paint lives in the shared `.left,.right` rule, its own inset and
  // `width:50%` in the standalone `.left` rule; both are receipts.
  for(const [selector,declar] of [
    ['.slider-container .left,.slider-container .right',{'background':'#44d62c','border-radius':'3px','bottom':'25px','height':'6px','position':'absolute'}],
    ['.slider-container .left',{'left':'0','max-width':'100%','width':'50%'}],
  ]){
    const rule=carries(css,selector,declar);
    record.rules.push({selector,offset:rule.offset,text:rule.text});
  }
  receipts.push(record);
}

// The shared local layer must still carry the same geometry and colors.
const slider='crates/razer-widgets/src/source_slider.rs',sliderText=read(slider);
for(const marker of ['if tip.is_some() { 64. } else { 36. }','bottom(surface::css(25.))','h(surface::css(6.))',
  'rounded(surface::css(3.))','bottom(surface::css(42.))','bottom(surface::css(20.))','h(surface::css(16.))',
  'size(surface::css(16.))','rounded(surface::css(8.))','ml(surface::css(-8.))','.left(surface::css(8.))',
  '.right(surface::css(8.))','8. - 16. * progress','window.rem_size() * (3. / 16.)'])
  assert(sliderText.includes(marker),'source slider layer lost '+marker);
for(const marker of ['SliderColors::track()','SliderColors::fill()','SliderColors::thumb()','SliderColors::thumb_hover()',
  'SliderColors::thumb_active()','SliderColors::thumb_border()','SliderColors::tip_text()'])
  assert(sliderText.includes(marker),'source slider layer lost '+marker);
for(const marker of ['source_thumb(&self.state, self.enabled, window, cx)',
  '("source-slider-opacity", self.state.entity_id())','if self.enabled { 1. } else { 0.3 }',
  'Duration::from_millis(300)','Easing::Ease','ThumbBackground(target.into())',
  '.capture_any_mouse_down(','.on_mouse_up_out('])
  assert(sliderText.includes(marker),'source slider motion lost '+marker);
const theme=read('crates/razer-widgets/src/theme.rs');
for(const color of ['0x44d62c4d','0x44d62c','0x5d5d5d','0x383838','0x212121'])
  assert(theme.includes(`rgb(${color})`)||theme.includes(`rgba(${color})`),'SliderColors lost '+color);

// Products that already draw this slider from the shared layer.
const accessory=read('crates/razer-pages/src/features/accessory_system_products.rs');
assert(accessory.includes('SourceSlider::new(slider, self.slider_progress(path)).enabled(enabled)'),
  'Accessory slider rows no longer mount the shared source slider');
const oled=read('crates/razer-pages/src/features/audio_oled.rs');
assert(oled.includes('SourceSlider::new(state, progress).tip(Some(format!("{value:.0}")))'),
  'OLED brightness slider no longer mounts the shared source slider with its value tip');

const evidence={method:'Static CSS parsing plus local source fingerprints; vendor code is never executed',
  offset_unit:'JavaScript UTF-16 code units; end is exclusive',
  generator_sha256:crypto.createHash('sha256').update(fs.readFileSync(__filename)).digest('hex'),sources:receipts,components,
  motion:{container_opacity:'300ms CSS ease; disabling input is immediate',
    thumb_background:'300ms CSS ease; green/gray RGB channels interpolate independently of the instant border',
    transform:'No transform endpoint in these selectors; no movement animation is invented',
    tip:'Later transition:left 0s,opacity 0s overrides the earlier opacity .3s declaration',
    validation:'Static parsing only. No runtime interaction, pixel or DPI acceptance.'},
  local:{slider:{path:slider,sha256:hash(slider)},theme:{path:'crates/razer-widgets/src/theme.rs',sha256:hash('crates/razer-widgets/src/theme.rs')},
    call_sites:[{path:'crates/razer-pages/src/features/accessory_system_products.rs',sha256:hash('crates/razer-pages/src/features/accessory_system_products.rs')},
      {path:'crates/razer-pages/src/features/audio_oled.rs',sha256:hash('crates/razer-pages/src/features/audio_oled.rs')}]}};
const target='docs/re/source-slider-current-evidence.json',serialized=JSON.stringify(evidence,null,2)+'\n';
if(process.argv.includes('--check'))
  assert(read(target)===serialized,'docs/re/source-slider-current-evidence.json is stale');
else fs.writeFileSync(path.join(root,target),serialized);
console.log(`Current source slider: ${receipts.length} stylesheets, ${receipts[0].rules.length} declarations each, shared layer fingerprints verified.`);
