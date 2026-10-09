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
  const files=['crates/razer-pages/src/features/source_controls.rs','crates/razer-pages/src/features/source_controls/oled_page.rs',
    'crates/razer-pages/src/features/source_controls/oled_home_cards.rs','crates/razer-pages/src/features/source_controls/oled_presets.rs','crates/razer-pages/src/features/keyboard_products.rs',
    'crates/razer-pages/src/features/source_workspace/profile_bar.rs','crates/razer-pages/src/features/source_controls/oled_system_editor.rs'];
  const local=Object.fromEntries(files.map(file=>[file,hash(read(file))]));
  assert(read(files[1]).includes('surface::page_column') && read(files[1]).includes('surface::css(530.)'), 'OLED local columns/screensaver width drift');
  assert(!read(files[3]).includes('fn render_home_mode_branch'), 'Legacy invented preview branch returned');
  assert(read(files[5]).includes('691 => !matches!(key, "OLED" | "TAB_POWER" | "HELP")'), '691 profile sync state drift');
  assert(read(files[2]).includes('6 => this.open_oled_system(window, cx)')
    && read(files[2]).includes('matches!(mode, 0 | 1 | 5 | 6)')
    && read(files[6]).includes('pub(super) fn system_preview'), '691 current System editor/preview route drift');
  // 命令拨盘帮助：源 `<i className="help" …/>` + `<Un.A position="bottom-right"
  // isMounted target className="command-dial-tooltip">…富文本…</Un.A>`，CSS
  // `.command-dial .help{background-color:#4a4a4a;border-radius:50%;height:14px;margin:0;
  //  position:absolute;right:10px;top:10px;width:14px}` 与 `.command-dial-tooltip{line-height:17px;white-space:pre-wrap}`。
  const dialCssFile = '.ref/devices/691/static/css/6375.333aa5fc.chunk.css';
  const dialCss = read(dialCssFile);
  const dialHelpDecl = (() => {
    const i = dialCss.indexOf('.command-dial .help{');
    return i < 0 ? null : dialCss.slice(i + '.command-dial .help{'.length, dialCss.indexOf('}', i));
  })();
  assert(dialHelpDecl, 'Current source no longer declares .command-dial .help');
  for (const token of ['background-color:#4a4a4a', 'border-radius:50%', 'height:14px',
    'position:absolute', 'right:10px', 'top:10px', 'width:14px']) {
    assert(dialHelpDecl.includes(token), 'Command dial help CSS drift: ' + token);
  }
  const dialTipDecl = (() => {
    const i = dialCss.indexOf('.command-dial-tooltip{');
    return i < 0 ? null : dialCss.slice(i + '.command-dial-tooltip{'.length, dialCss.indexOf('}', i));
  })();
  assert(dialTipDecl && dialTipDecl.includes('line-height:17px'), 'Command dial tooltip CSS drift');
  const dialSource = read('.ref/devices/691/static/js/6375.a5fed9ed.chunk.js');
  for (const token of ['className:"help"', 'position:"bottom-right"', 'className:"command-dial-tooltip"',
    'isMounted:']) {
    assert(dialSource.includes(token), 'Command dial help markup drift: ' + token);
  }
  const keyboardControls = read('crates/razer-pages/src/features/keyboard_controls.rs');
  for (const token of ['dial_help_hovered', '.id("dial-help")', 'source_hover_tip_element(',
    'SourceTipPlacement::BottomRight', 'rgba(0xffffff4d)', 'rgba(0x4a4a4aff)']) {
    assert(keyboardControls.includes(token), 'Missing native command-dial help contract: ' + token);
  }
  assert(!keyboardControls.includes('dial-help-content'), 'Legacy Kit rich tooltip returned');
  const dialHelp = {css: dialCssFile, declaration: dialHelpDecl, tooltip: dialTipDecl,
    native: 'dial_help_hovered + source_hover_tip_element(SourceTipPlacement::BottomRight), 14px #4a4a4a/#ffffff4d control'};
  // 拨盘 `icon-add` 的 `tooltip` 属性 → 全局 `[tooltip]:before` 伪元素（691 chunk CSS）。
  const tipCssFile = '.ref/devices/691/static/css/5171.1330bdc6.chunk.css';
  const tipCss = read(tipCssFile);
  // 该文件里 `.nav-tabs .batt[tooltip]:before{…}` 等更具体规则也含同一子串，
  // 因此用基础规则的声明本身定位。
  const tipAt = tipCss.indexOf('[tooltip]:before{background-color:#000;');
  assert(tipAt >= 0, 'Current source no longer declares the global [tooltip]:before rule');
  const tipDecl = tipCss.slice(tipAt + '[tooltip]:before{'.length, tipCss.indexOf('}', tipAt));
  for (const token of ['background-color:#000', 'border:1px solid #5d5d5d', 'color:#ccc',
    'content:attr(tooltip)', 'font-size:14px', 'line-height:16px', 'opacity:0', 'padding:8px 10px',
    'pointer-events:none', 'position:absolute', 'right:0', 'top:calc(100% + 5px)',
    'transition:visibility 0s,opacity .3s linear', 'white-space:nowrap']) {
    assert(tipDecl.includes(token), 'Global tooltip attribute CSS drift: ' + token);
  }
  assert(tipCss.includes('[tooltip]:hover:before{opacity:1;visibility:visible}'),
    'Global tooltip attribute hover rule drift');
  for (const token of ['className:"icon-add"', 'tooltip:', 'getTextItem']) {
    assert(dialSource.includes(token), 'Dial add button markup drift: ' + token);
  }
  const attributeTip = read('crates/razer-widgets/src/attribute_tip.rs');
  for (const token of ['pub(crate) fn attribute_tip(', '.right_0()', '.top_full()', '.mt(surface::css(5.))',
    '.px(surface::css(10.))', '.py(surface::css(8.))', 'TooltipColors::border()', 'TooltipColors::background()',
    'TooltipColors::foreground()', '.whitespace_nowrap()', 'Animation::new(Duration::from_millis(300))']) {
    assert(attributeTip.includes(token), 'Missing native attribute tooltip contract: ' + token);
  }
  for (const token of ['dial_add_hovered', 'attribute_tip::attribute_tip(', '"ADD_NEW_MODE"', '"dial-add-tip"']) {
    assert(keyboardControls.includes(token), 'Dial add tooltip not wired: ' + token);
  }
  // 源 `icon-delete` 的提示是**有条件**的：`tooltip:R?void0:(0,s.getTextItem)(vn.DELETE)`
  // —— `R`（确认展开）为真时不渲染提示。本地 delete 触发器用 `attribute_tip_group`
  // 复刻同一皮肤与显隐条件（`!reset && !open`）；reset 触发器没有对应的源标记，仍留 Kit 提示。
  const dialCompact = dialSource.replaceAll(/\s+/g, '');
  assert(dialCompact.includes('tooltip:R?void0:(0,s.getTextItem)(vn.DELETE)'),
    'Delete conditional tooltip markup drift');
  for (const token of ['attribute_tip_group(', '!reset && !open', '"dial-confirm-trigger"']) {
    assert(keyboardControls.includes(token), 'Delete tooltip condition not wired: ' + token);
  }
  const attributeTipSrc = read('crates/razer-widgets/src/attribute_tip.rs');
  for (const token of ['pub(crate) fn attribute_tip_group(', '.group_hover(group, |style| style.opacity(1.))',
    '.opacity(0.)']) {
    assert(attributeTipSrc.includes(token), 'Missing grouped attribute tooltip contract: ' + token);
  }
  // `dial_icon_button` 必须能二选一：源里带 `tooltip` 属性的图标用共享徽标，不能让 Kit
  // 提示叠在同一控件上（`dial-add`），源标记未定位的图标（`dial-expand`）继续用 Kit。
  assert(keyboardControls.includes('kit_tooltip: bool'), 'dial_icon_button lost the tooltip switch');
  assert(keyboardControls.includes('let button = if kit_tooltip {'),
    'dial_icon_button no longer branches on kit_tooltip');
  assert(keyboardControls.split('ADD_NEW_MODE')[1].includes('false,'),
    'dial-add must not attach a Kit tooltip next to the attribute badge');
  assert(keyboardControls.split('旋转方向的按键分配')[1].includes('true,'),
    'dial-expand must keep its Kit tooltip until its source mark is located');
  const attributeTooltip = {css: tipCssFile, declaration: tipDecl,
    delete_condition: 'source tooltip=R?void0:DELETE, local !reset && !open + attribute_tip_group (group_hover opacity; the source 300ms linear fade is not reachable from the &App-only trigger closure)',
    icon_switch: 'dial_icon_button(kit_tooltip) — dial-add false (attribute badge), dial-expand true (source mark unlocated)',
    native: 'crates/razer-widgets/src/attribute_tip.rs attribute_tip/attribute_tip_group + dial_add_hovered on the 691 dial add button'};
  const evidence={schema_version:1,date:'2026-10-05',product_id:691,
    method:'Current manifest ownership, Acorn AST and CSS parsing only; no app, build, test or vendor code execution.',
    boot:mainReceipt(boot),css_loader:mainReceipt(cssLoader),bindings,styles,local,dial_help:dialHelp,attribute_tooltip:attributeTooltip,
    profile_contract:'Ca disables isEnableProfileBar for OLED/Power/Help; Kt uses it only for loader disable. Profile bar remains mounted and enableSwitchProfile controls dropdown independently.',
    corrected:['Lazy base body Roboto16/#ccc/#222 and padding10/20/20','OLED 1220/600 home and fixed600 setting columns','Seven home card shells in source order; animation/image/crop/import/media editors and partial System editor mounted','Emote/Banner edit actions disabled because their editors are absent','OLED title switch, corner tips, language Apply staging, numeric buttons, screen-saver dimensions','691 existing Power dim/sleep title switches and raw48x27 numeric choices'],
    limitations:['Emote/Banner home previews and editors are unimplemented; System has a partial text preview/editor, with source SVG presentation and full workflow still incomplete','Power low-battery warning, indicator and low-power information widgets are still absent','BLE/download/service-derived flags and source system-slide Apply semantics incomplete','Card title hover and source tooltip mechanisms are mounted; runtime pixels and the BLE edit-button versus source-card anchor still require comparison','Shared slider source release semantics/hover transition and CSS normal font metrics remain unverified','Customize/Lighting/Help content and other keyboard-specific widgets remain partial'],
    editor_scope:{emote:'Preview and editor unavailable; Edit disabled.',banner:'Preview and editor unavailable; Edit disabled.',mounted:['PresetEditor','CropDraft','OledMediaEditor','OledSystemEditor','Import/crop/Apply callbacks'],system:'Local device-owned layout/preferences with source sample text; original SVG presentation, telemetry and service workflow remain incomplete.'}};
  const target='docs/re/keyboard-691-current-evidence.json',output=JSON.stringify(evidence,null,2)+'\n';
  if(process.argv.includes('--check'))assert(read(target)===output,'Stale 691 audit receipt');
  else fs.writeFileSync(target,output);
  console.log('691: current lazy CSS, OLED card/settings mounts, profile consumer and two Power widgets audited; full-page parity remains incomplete.');
}
