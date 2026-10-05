// Current product modules are parsed as Acorn data, never imported or executed.
const fs = require('fs');
const {Source, walk, hash, key} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
class ProductSource extends Source {
  constructor(pid) {
    // Source's application constructor deliberately accepts applications only.
    super('synapse/armory');
    this.directory = `.ref/devices/${pid}`;
    this.manifestPath = `${this.directory}/asset-manifest.json`;
    this.manifest = JSON.parse(fs.readFileSync(this.manifestPath, 'utf8'));
    this.files = [...new Set(Object.values(this.manifest.files))]
      .filter(f => /^\.\/static\/js\/[^/]+\.js$/.test(f))
      .map(f => `${this.directory}/${f.slice(2)}`);
    this.modules = new Map(); this.texts = new Map(); this.parsed = new Set();
  }
}
const products = [];
for (const pid of [3894, 3907]) {
  const source = new ProductSource(pid);
  const info = source.binding(8193, source.exported(8193, 'DeviceInfo').name);
  const category = source.literal(8193, info.properties.find(p => key(p.key) === 'category').value);
  const moduleIds = pid === 3894 ? [8193, 7855, 3288, 1422, 3765] : [8193, 1368];
  const modules = moduleIds.map(id => ({id, ...source.receipt(id, source.module(id).fn)}));
  const mainPath = `${source.directory}/${source.manifest.files['main.js'].slice(2)}`;
  const text = source.text(mainPath);
  const declarations = [];
  const symbols = new Set(pid === 3907
    ? ['rU','nU','cm','qP','Rm','Im','Kp','Em','am','Rn','Dn','dm','rm','On','In','JP','An','Sn','ln']
    : []);
  walk(require('acorn').parse(text, {ecmaVersion: 'latest'}), node => {
    const name = node.id?.name;
    if (node.start < 4180000 || !symbols.has(name)
      || !['VariableDeclarator','ClassDeclaration'].includes(node.type)) return;
    const value = node.init || node;
    declarations.push({symbol:name,path:mainPath,sha256:hash(text),offset:value.start,end:value.end,
      source:text.slice(value.start,value.end)});
  });
  const css = [...new Set(Object.values(source.manifest.files))].filter(f => f.endsWith('.css')).map(f => {
    const path = `${source.directory}/${f.slice(2)}`, text = fs.readFileSync(path,'utf8');
    return {path,sha256:hash(text),rules:parseCSS(text).filter(r =>
      /SystemInfo|FanControl|config-wrapper|config-block|config-btns|coolingPerformance|chart-content|line-chart|multiple-tab|widget-prod|body-widgets|widget-col/.test(r.selector))};
  });
  const localization = pid === 3907 ? Object.fromEntries(
    ['ztb','Isz','H51','kdZ','FmD','_TM','Y_1','gah','QaP','eNf','ZuR','Lxu','nnF','nUA','eL8','O1I','bZ7','K0Z','YLI']
      .map(k => [k,source.literal(4693,source.exported(4693,k))])) : {};
  const axisLabels=[];
  if(pid===3907){
    const values=[];
    walk(source.module(5955).fn,node=>{
      if(node.type==='Property'&&key(node.key)==='FAN_SPEED'&&node.value.type==='ArrowFunctionExpression')
        values.push({value:source.literal(5955,node.value.body),...source.receipt(5955,node)});
    });
    for(const file of fs.readdirSync('locales').filter(f=>f.endsWith('.json'))){
      const value=JSON.parse(fs.readFileSync('locales/'+file,'utf8')).FAN_SPEED;
      const matches=values.filter(v=>v.value===value);
      if(matches.length!==1)throw Error('Current product axis locale mismatch: '+file);
      axisLabels.push({locale:file.slice(0,-5),...matches[0]});
    }
  }
  products.push({product_id:pid,manifest:{path:source.manifestPath,sha256:hash(fs.readFileSync(source.manifestPath))},
    device_info:source.receipt(8193,info),category,modules,declarations,css,localization,axis_labels:axisLabels});
}
const result = {method:'Acorn scopes and CSS parser. Downloaded JavaScript is data only.',products,
  conclusions:{
    3894:{product:'Razer Head Cushion Chroma',category:'ACCESSORY',active_root:'7855 default g -> m product image',
      dormant_default:'3288 default ba -> Fa mounts image, brightness, switch-off and effects. The current static DeviceInfo.category cannot choose DEFAULT.',
      category_source:'8193.DeviceInfo.category, not the local discovery Device.category'},
    3907:{active_root:'TU -> rU -> sU/nU -> um/cm',
      initial_runtime:'Dn runtime metrics and initStatus are undefined; not measured zeros.',
      settled_without_hardware:'am sets displayWarning=true because initStatus is undefined; CPU/GPU read Not detected, with em dashes. systemMode becomes none, so Rm is empty.',
      controls:'FanControl defaults to enabled Smart CPU Balanced; am does not disable the secondary mode tabs, curve, unit controls or reset.',
      armory_css:'rU does not mount #coolingPerformance or .body-widgets; their descendant-only rules do not apply to this Armory tree.',
      source_initial_curve:'CPU Balanced: (40,1000),(90,2200),(100,2200), temperature 40..100, RPM 500..3200.',
      limitations:['No hardware telemetry, remote mappings or hardware write acknowledgement. Exact product raster remains absent; current AVIF bytes do not obey the SVG MD4 naming rule, so that rule must not be used to invent an AVIF match.',
        'Native initial-state layout is not pixel validated. The speed-unit selector uses the source 270-degree rotation with native vertical hitboxes and prepared glyph outlines. Named installed Windows fallback fonts for missing Roboto glyphs are recorded, not asserted to be the browser fallback. Generic tooltip positioning/transitions still differ. Fixed-RPM/hyperboost/live-telemetry branches remain unavailable without their source hardware states.',
        'No application or screenshot validation.']}
  }};
const nativePath='src/features/armory_product/cooling_pad.rs';
const native=fs.readFileSync(nativePath,'utf8');
for(const token of ['fn vertical_speed_units(', 'cooling_axis_data.json', 'window.on_mouse_event(',
  'phase.capture()', 'percentage_label(', 'state.curves[state.sensor][state.mode]',
  'Not detected', '15.625%', 'PERFORMANCE_BALANCED', 'PERFORMANCE_MAXIMUM_POWER']){
  if(!native.includes(token))throw Error('Missing native source contract: '+token);
}
result.native={path:nativePath,sha256:hash(native),axis_resources:'docs/re/armory-cooling-axis-resources.json',
  interaction:'Native vertical hitboxes; global pointer capture during curve drag; local edits only'};
const output = 'docs/re/armory-remaining-roots-current-evidence.json';
const rendered = JSON.stringify(result,null,2)+'\n';
if (process.argv.includes('--check')) {
  if(fs.readFileSync(output,'utf8') !== rendered) throw Error('Armory remaining roots receipt is stale');
} else fs.writeFileSync(output,rendered);
console.log('Armory remaining roots: current manifests, category branches, renderers, defaults and CSS verified.');
