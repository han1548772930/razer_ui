// Inspect product 691's manifest-owned lazy modules as syntax/data, never eval.
const fs = require('fs');
const path = require('path');
const {Source, hash, walk} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const directory = '.ref/devices/691';
const manifest = JSON.parse(fs.readFileSync(`${directory}/asset-manifest.json`, 'utf8'));
const source = Object.create(Source.prototype);
source.directory = directory;
source.files = [...new Set(Object.values(manifest.files))]
  .filter(file => file.includes('/static/js/') && file.endsWith('.js'))
  .map(file => `${directory}/${file.slice(file.indexOf('static/'))}`);
source.modules = new Map();
source.texts = new Map();
source.parsed = new Set();
const escape = value => String(value).replaceAll('&','&amp;').replaceAll('"','&quot;').replaceAll('<','&lt;');
function svg(node) {
  if (!node || node.type === 'ConditionalExpression') return '';
  if (['LogicalExpression','AssignmentExpression'].includes(node.type)) return svg(node.right);
  if (node.type === 'Literal') return node.value == null ? '' : escape(node.value);
  if (node.type !== 'CallExpression' || node.callee.property?.name !== 'createElement') throw Error('Nonliteral SVG');
  const [tag, props, ...children] = node.arguments;
  const object = props.type === 'CallExpression' ? props.arguments[0] : props;
  let attributes = '';
  if (object.type === 'ObjectExpression') for (const p of object.properties) {
    const key = p.key.name || p.key.value;
    if (['ref','aria-labelledby','nonce'].includes(key)) continue;
    if (p.value.type !== 'Literal') throw Error('Nonliteral SVG attribute');
    const name = ({className:'class',xmlnsXlink:'xmlns:xlink',fillRule:'fill-rule',clipRule:'clip-rule',strokeWidth:'stroke-width',strokeLinecap:'stroke-linecap',strokeLinejoin:'stroke-linejoin'}[key] || key);
    attributes += ` ${name}="${escape(p.value.value)}"`;
  }
  return `<${tag.value}${attributes}>${children.map(svg).join('')}</${tag.value}>`;
}
if (process.argv.includes('--assets') || process.argv.includes('--assets-check') || process.argv.includes('--register-assets')) {
  const check = process.argv.includes('--assets-check'), records = [];
  const save = (asset, bytes, node) => {
    if (check) { if (!fs.readFileSync(asset).equals(bytes)) throw Error('Stale '+asset); }
    else fs.writeFileSync(asset, bytes);
    const proof = source.receipt(42553,node);
    records.push({asset,sha256:hash(bytes),source:proof.path,source_sha256:proof.sha256,offset:proof.offset,end:proof.end});
  };
  for (const [binding,name] of [['y','edit'],['E','apply'],['k','requires-synapse'],['lt','unavailable']]) {
    const node = source.binding(42553,binding); let root;
    walk(node,n=>{if(n.type==='CallExpression' && n.callee.property?.name==='createElement' && n.arguments[0]?.value==='svg')root=n;});
    if (!root) throw Error('Missing SVG '+binding);
    save(`assets/synapse/oled-691-${name}.svg`,Buffer.from(svg(root)+'\n'),node);
  }
  const home = source.binding(42553,'di'); let png;
  walk(home,n=>{if(n.type==='Literal' && typeof n.value==='string' && n.value.startsWith('data:image/png;base64,'))png=n;});
  if(!png)throw Error('Missing keyboard preview');
  save('assets/synapse/oled-691-keyboard.png',Buffer.from(png.value.split(',')[1],'base64'),png);
  const output='docs/re/keyboard-691-assets-current-evidence.json', text=JSON.stringify(records,null,2)+'\n';
  if(check){if(fs.readFileSync(output,'utf8')!==text)throw Error('Stale asset receipt');}
  else fs.writeFileSync(output,text);
  if (check || process.argv.includes('--register-assets')) {
    const manifestPath='assets/synapse/manifest.json', embeddedPath='assets/synapse/embedded.rs';
    const manifest=JSON.parse(fs.readFileSync(manifestPath,'utf8'));
    let embedded=fs.readFileSync(embeddedPath,'utf8');
    for (const record of records) {
      const name=path.basename(record.asset), asset='synapse/'+name;
      const entry={source:record.source,output:record.asset,source_sha256:record.source_sha256,
        sha256:record.sha256,source_offset:record.offset,source_end:record.end,
        source_kind:name.endsWith('.png')?'inline_png_literal':'inline_svg_literal',
        transform:name.endsWith('.png')?'Original base64 PNG bytes':'Static literal createElement SVG serialization'};
      if(name.endsWith('.png')) {
        const bytes=fs.readFileSync(record.asset);
        entry.width=bytes.readUInt32BE(16);entry.height=bytes.readUInt32BE(20);
      }
      const index=manifest.entries.findIndex(e=>e.output===record.asset);
      if (check) {
        if(index<0 || JSON.stringify(manifest.entries[index])!==JSON.stringify(entry) || !embedded.includes('"'+asset+'"'))
          throw Error('Missing/stale OLED registration '+asset);
      } else {
        if(index<0)manifest.entries.push(entry);else manifest.entries[index]=entry;
        if(!embedded.includes('"'+asset+'"'))embedded=embedded.replace(/\]\s*$/,`    ("${asset}", include_bytes!("${name}") as &[u8]),\n]\n`);
      }
    }
    if(!check){fs.writeFileSync(manifestPath,JSON.stringify(manifest,null,2)+'\n');fs.writeFileSync(embeddedPath,embedded);}
  }
  console.log('691 OLED: four literal SVGs and original keyboard PNG verified.');
} else if (process.argv.includes('--exports')) {
  const index = process.argv.indexOf('--exports'), id = Number(process.argv[index + 1]);
  for (const name of process.argv.slice(index + 2)) console.log(name, source.literal(id,source.exported(id,name)));
} else if (process.argv.includes('--inspect')) {
  const index = process.argv.indexOf('--inspect'), id = Number(process.argv[index + 1]);
  const scope = source.module(id), names = process.argv.slice(index + 2);
  if (!names.length) {
    console.log(JSON.stringify({id, file: scope.file, exports: [...scope.exports],
      definitions: [...scope.definitions].map(([name,node]) => ({name, type: node?.type,
        offset: node?.start, excerpt: node ? source.snippet(id,node).slice(0,130) : null}))},null,2));
  } else for (const name of names) console.log(JSON.stringify({id,name,...source.receipt(id,source.binding(id,name))}));
} else {
  const assert = (condition, message) => { if (!condition) throw Error(message); };
  const read = file => fs.readFileSync(file,'utf8');
  const main = source.files.find(file=>/\/main\./.test(file));
  const ast = require('acorn').parse(source.text(main),{ecmaVersion:'latest'});
  let boot, cssLoader;
  walk(ast,node=>{
    if(node.type==='CallExpression' && node.callee.property?.name==='then'
      && node.arguments[0]?.type==='CallExpression' && node.arguments[0].callee.property?.name==='bind'
      && node.arguments[0].arguments[1]?.value===82189) boot=node;
    if(node.type==='AssignmentExpression' && node.left.property?.name==='miniCssF') cssLoader=node;
  });
  assert(boot && cssLoader,'Missing current lazy boot/css loader');
  const mainReceipt=node=>({path:main,sha256:hash(source.text(main)),offset:node.start,end:node.end,source:source.text(main).slice(node.start,node.end)});
  assert(mainReceipt(boot).source.includes('n.e(5171)') && mainReceipt(boot).source.includes('n.e(7668)'), 'Root base chunks changed');
  assert(mainReceipt(cssLoader).source.includes('5171:"5171"') || mainReceipt(cssLoader).source.includes('5171'), 'Base CSS mapping changed');
  const bindings = [[82189,['Ca','_a']],[55714,['Kt']],[42553,['na','di','Q','G','hi','wi','xi','Gi','ea']],[35665,['W','y','u','w','b','F']]]
    .flatMap(([id,names])=>names.map(symbol=>({module:id,symbol,...source.receipt(id,source.binding(id,symbol))})));
  const component=(id,name)=>bindings.find(b=>b.module===id&&b.symbol===name).source;
  assert(component(82189,'Ca').includes('isEnableProfileBar:this.state.displayProfileBar'), 'Profile prop changed');
  assert(JSON.stringify(source.literal(82189, source.binding(82189,'_a'))) === '["OLED","TAB_POWER","HELP"]', 'Profile disabled page keys changed');
  const consumer=component(55714,'Kt');
  assert(consumer.includes('!this.props.isEnableProfileBar||this.props.activeDynamicMode')
    && consumer.includes('className:"loader disable"') && consumer.includes('this.renderProfileBarIcon()'), 'Profile consumer changed');
  const home=component(42553,'di');
  for(const [id,type] of [['animation',0],['image',1],['emote',4],['banner',2],['media',5],['system',6],['keyboard',3]])
    assert(home.includes(`id:"${id}",type:${type}`), 'Home card order/type changed '+id);
  assert(component(42553,'na').includes('direction:"left"') && component(42553,'na').includes('direction:"right"'), 'OLED columns changed');
  const cssFiles = [...new Set(Object.values(manifest.files))]
    .filter(file=>/\/(5171|7668|2535|OLED|Power)\.[^/]+\.css$/.test(file))
    .map(file=>`${directory}/${file.slice(file.indexOf('static/'))}`);
  const styles=cssFiles.map(file=>({path:file,sha256:hash(read(file)),rules:parseCSS(read(file)).filter(rule=>
    /body,html|body-wrapper|body-widgets|widget-col|titleRow|\.widget \.help|\.h1-body|config-wrapper|HomeScreenDisplay_|DisplayWidget_|OLEDLanguage_|OLEDScreensaver_|DimKeyboardLighting_|polling-btn-set|customize-polling-rate-button|keyboard-btn-size|^\.slider/.test(rule.selector))}));
  assert(styles.some(s=>s.path.includes('/5171.')), 'Lazy base CSS absent');
  const files=['src/features/source_controls.rs','src/features/source_controls/oled_page.rs',
    'src/features/source_controls/oled_home_cards.rs','src/features/source_controls/oled_presets.rs','src/features/keyboard_products.rs',
    'src/features/source_workspace/profile_bar.rs'];
  const local=Object.fromEntries(files.map(file=>[file,hash(read(file))]));
  assert(read(files[1]).includes('surface::page_column') && read(files[1]).includes('surface::css(530.)'), 'OLED local columns/screensaver width drift');
  assert(!read(files[3]).includes('fn render_home_mode_branch'), 'Legacy invented preview branch returned');
  assert(read(files[5]).includes('691 => !matches!(key, "OLED" | "TAB_POWER" | "HELP")'), '691 profile sync state drift');
  const evidence={schema_version:1,date:'2026-10-05',product_id:691,
    method:'Current manifest ownership, Acorn AST and CSS parsing only; no app, build, test or vendor code execution.',
    boot:mainReceipt(boot),css_loader:mainReceipt(cssLoader),bindings,styles,local,
    profile_contract:'Ca disables isEnableProfileBar for OLED/Power/Help; Kt uses it only for loader disable. Profile bar remains mounted and enableSwitchProfile controls dropdown independently.',
    corrected:['Lazy base body Roboto16/#ccc/#222 and padding10/20/20','OLED 1220/600 home and fixed600 setting columns','Seven home card shells in source order; existing animation/image/crop/import/media editors preserved','Removed invented530x120 placeholders and fake Emote/Banner dialogs','OLED title switch, corner tips, language Apply staging, numeric buttons, screen-saver dimensions','691 existing Power dim/sleep title switches and raw48x27 numeric choices'],
    limitations:['Emote/Banner/System home previews and real editors are still unimplemented; their edit actions are disabled locally, a known source difference','Power low-battery warning, indicator and low-power information widgets are still absent','BLE/download/service-derived flags and source system-slide Apply semantics incomplete','Card title hover, require-Synapse tooltip and pixel/raster parity not yet verified','Shared slider source release semantics/hover transition and CSS normal font metrics remain unverified','Customize/Lighting/Help content and other keyboard-specific widgets remain partial'],
    removed_fake_editors:{emote:'Old callback mounted only a service-unavailable note, no editor entity or fields.',banner:'Old callback mounted only a transfer-unavailable note; Top/Bottom buttons had no handlers.',preserved:['PresetEditor','CropDraft','OledMediaEditor','Import/crop/Apply callbacks']}};
  const target='docs/re/keyboard-691-current-evidence.json',output=JSON.stringify(evidence,null,2)+'\n';
  if(process.argv.includes('--check'))assert(read(target)===output,'Stale 691 audit receipt');
  else fs.writeFileSync(target,output);
  console.log('691: current lazy CSS, OLED card/settings mounts, profile consumer and two Power widgets audited; full-page parity remains incomplete.');
}
