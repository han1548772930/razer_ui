// Called by generate-native-camera-controls.cjs. Only reads source literals.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const hash = text => crypto.createHash('sha256').update(text).digest('hex');
const receipts = [];
const preview = require('./camera-preview.cjs');
function prepare({result, audited, label, source, file}) {
 const pid = result.product_id;
 const groups = [];
 const receipt = (start, end) => ({path:file,offset:start,end,sha256:hash(source.slice(start,end)),source:source.slice(start,end)});
 const functionAt = start => {
  if(start<0)throw Error(`Missing camera presentation function ${pid}`);
  let node=acorn.parseExpressionAt(source,start,{ecmaVersion:'latest'});
  if(node.type==='SequenceExpression')node=node.expressions[0];
  if(node.type!=='ArrowFunctionExpression')throw Error(`Unexpected camera presentation function ${pid}:${start}`);
  return receipt(node.start,node.end);
 };
 const wrapper=functionAt(source.indexOf('e=>{const E=e.checkBoxName'));
 if(!wrapper.source.includes('className:"content-wrapper"')||!wrapper.source.includes('.useState)(!0)'))throw Error(`Changed camera disclosure behavior ${pid}`);
 const tipMarker=source.indexOf('.parentNode.querySelector(".tip")');
 const tip=functionAt(source.lastIndexOf('e=>{const E=(0,',tipMarker));
 if(!tip.source.includes('createPortal')||!tip.source.includes('clientHeight')||!tip.source.includes('E.right+t>window.innerWidth'))throw Error(`Changed camera portal behavior ${pid}`);
 const jsxFor = page => {
  const seen = new Map();
  for (const component of audited.pages.find(p => p.key === page)?.components ?? []) {
   if (source.slice(component.offset, component.end) !== component.source) throw Error(`Stale camera component ${pid}:${component.offset}`);
   for (const jsx of component.jsx) seen.set(jsx.offset, jsx);
  }
  return [...seen.values()];
 };
 const rowFor = (page, title) => {
  const rows = jsxFor(page).filter(j => j.props.name === title);
  if (rows.length !== 1) throw Error(`Ambiguous camera wrapper ${pid}:${page}:${title}:${rows.length}`);
  const row = rows[0];
  let expression = acorn.parseExpressionAt(source, row.offset, {ecmaVersion:'latest'});
  if(expression.type==='SequenceExpression')expression=expression.expressions[0];
  const object = expression.type==='CallExpression' ? expression.arguments[1] : expression;
  if (object.type !== 'ObjectExpression') throw Error(`Expected JSX props ${pid}:${row.offset}`);
  return {...row, object};
 };
 const processing = result.pages.find(p => p.key === 'PROCESSING');
 const processRoot = audited.pages.find(p => p.key === 'PROCESSING').components.find(c => c.source.includes('className:"camera-container'));
 const quality = processing.sections.find(s => ['MJPEG_QUALITY','RESOLUTION_AND_MJPEG_QUALITY'].includes(s.title));
 // The actual mount suppresses MJPEG entirely for 3594; 3595 suppresses only
 // the two children after its automatic-quality switch despite the prop name.
 if (pid === 3594) {
  if (!processRoot.source.includes('supportMJPEG:!1')) throw Error('Changed 3594 quality mount');
  processing.sections = processing.sections.filter(s => s !== quality);
 }
 if (pid === 3595) {
  if (!processRoot.source.includes('disableHDRSwitch:!0')) throw Error('Changed 3595 quality mount');
  const proof = audited.pages.find(p=>p.key==='PROCESSING').components.find(c=>c.source.includes('mjpegQuality'));
  if (!/E\?null:o\.autoQuality\?/.test(proof.source)) throw Error('Changed 3595 quality child condition');
  quality.controls = quality.controls.filter(c => c.key.endsWith(':auto-quality'));
 }
 if (pid === 3592) {
  const proof = audited.pages.find(p=>p.key==='PROCESSING').components.find(c=>c.source.includes('mjpegQuality'));
  const symbol = /dataSet:([\w$]+),value:/.exec(proof.source)?.[1];
  const literal = audited.arrays.find(a=>a.symbol===symbol && a.value.every(v=>v.width && v.height && v.fps));
  if (!literal) throw Error('Missing Ultra processing resolution literal');
  quality.controls.unshift({kind:'select',key:`${pid}:processing-resolution`,label:label('kM4'),path:'/camera/resolution',options:literal.value.map(v=>({label:v.name,value:{width:v.width,height:v.height,fps:v.fps}})),source:{path:proof.path,offset:proof.offset,end:proof.end}});
 }
 // Both processing roots render the quality/HDR component before the other
 // processing rows. The image root mounts anti-flicker, watermark, then mirror.
 const priority = s => ['RESOLUTION_AND_MJPEG_QUALITY','MJPEG_QUALITY','HDR'].includes(s.title) ? 0 : 1;
 processing.sections.sort((a,b)=>priority(a)-priority(b));
 const image = result.pages.find(p=>p.key==='IMAGE');
 const imageOrder=['IMAGE','ANTI_FLICKER','WATERMARK','MIRROR_VIDEO'];
 image.sections.sort((a,b)=>imageOrder.indexOf(a.title)-imageOrder.indexOf(b.title));
 for (const page of result.pages) {
  if (!['CAMERA','PROCESSING','IMAGE'].includes(page.key)) continue;
  const pageGroups=[];
  const add = (title, controls, extra={}) => {
   const row=rowFor(page.key,title),props=row.props;
   let collapsible=props.noToggle!==true;
   if (row.expressions.noToggle) {
    if(title!=='ZOOM'||!audited.pages.find(p=>p.key==='CAMERA').components.some(c=>c.source.includes('supportAutoFrame:!1')))throw Error(`Unresolved camera collapse ${pid}:${title}`);
    collapsible=true;
   }
   let tooltip=props.tooltipContent;
   if(row.expressions.tooltipContent) {
    if(title!=='AUTO_EXPOSURE')throw Error(`Unresolved camera tooltip ${pid}:${title}`);
    const mount=jsxFor('CAMERA').find(j=>j.props.tooltipContent&&!j.props.name);
    if(mount)tooltip=mount.props.tooltipContent;
    else {
     const owner=audited.pages.find(p=>p.key==='CAMERA').components.find(c=>c.source.includes('autoExposure')&&c.source.includes('tooltipContent'));
     const fallback=/void 0===[\w$]+\?[\w$]+\.([\w$]+):/.exec(owner.source);
     if(!fallback)throw Error(`Missing exposure tooltip default ${pid}`);
     tooltip=label(fallback[1]);
    }
   }
   const group={key:`${pid}:${page.key}:${title}`,title,controls:controls.map(c=>c.key),collapsible,...extra};
   if(tooltip)group.tooltip=tooltip;
   if(props.hasSwitch)group.header_switch=controls.find(c=>c.kind==='switch'&&c.label===title)?.key;
   if(props.hasStepper)group.header_stepper=controls.find(c=>c.kind==='slider'&&c.label===title)?.key;
   if(props.hasCheckbox)group.header_checkbox=controls.find(c=>c.kind==='toggle')?.key;
   if(props.hasReset)group.header_reset=controls.find(c=>c.kind==='reset')?.key;
   for(const [flag,key] of [['hasSwitch','header_switch'],['hasStepper','header_stepper'],['hasCheckbox','header_checkbox'],['hasReset','header_reset']])if(props[flag]&&!group[key])throw Error(`Missing camera header control ${pid}:${title}:${key}`);
   if(title==='PREVIEW_RESOLUTION_2') {
    if(!/\.isEnabled\|\|[\s\S]*\.isEnabled\?[\w$]+\.pKo:null/.test(row.expressions.warningTooltip))throw Error(`Changed resolution warning ${pid}`);
    group.warning=label('pKo');
    group.warning_any=['/camera/autoFraming/isEnabled','/camera/hdr/isEnabled'];
   }
   if(title==='LOW_LIGHT_COMPENSATION'&&pid!==3596) {
    if(!source.slice(row.object.start,row.object.end).includes('.guc'))throw Error(`Missing low-light description ${pid}`);
    group.description=label('guc');
   }
   if(title==='LENS_DISTORTION_COMPENSATION') {
    if(!source.slice(row.object.start,row.object.end).includes('.height)>=1440'))throw Error('Changed LDC processing condition');
    group.description=label('zjd');group.description_when_enabled='/camera/ldc';group.description_min_height=1440;
   }
   pageGroups.push(group);
   groups.push({group,jsx:receipt(row.object.start,row.object.end)});
  };
  for(const section of page.sections) {
   if(section.title==='IMAGE') {
    for(const control of section.controls.filter(c=>c.kind!=='toggle')) {
     const controls=[control];
     if(control.key.endsWith(':white-balance'))controls.push(section.controls.find(c=>c.kind==='toggle'));
     add(control.label,controls,{divider:control.key.endsWith(':image-preset')?'none':control.key.endsWith(':brightness')?'normal':'transparent'});
    }
   } else if(section.title==='HDR') {
    const proof=audited.pages.find(p=>p.key==='PROCESSING').components.find(c=>c.source.includes('mjpegQuality'));
    if(!proof.source.includes('children:"HDR"')||!proof.source.includes('.hd_'))throw Error(`Missing direct HDR header ${pid}`);
    const group={key:`${pid}:PROCESSING:HDR`,title:'HDR',controls:section.controls.map(c=>c.key),collapsible:false,header_switch:section.controls.find(c=>c.kind==='switch').key,tooltip:label('hd_')};
    pageGroups.push(group);groups.push({group,jsx:receipt(proof.offset,proof.end)});
   } else add(section.title,section.controls);
  }
  page.camera_groups=pageGroups;
 }
 preview.prepare({result, audited, label, source, file});
 for(const page of result.pages)for(const group of page.camera_groups??[])if(group.presentation) {
  const component=audited.pages.find(p=>p.key==='CAMERA').components.find(c=>c.source.includes('resolutionList'));
  groups.push({group,jsx:receipt(component.offset,component.end)});
 }
 const cssPath=path.join(root,`.ref/devices/${pid}/static/css`);
 const cssFile=fs.readdirSync(cssPath).find(n=>/^main\..*\.css$/.test(n));
 const css=fs.readFileSync(path.join(cssPath,cssFile),'utf8');
 const rules=[...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].filter(m=>/wrapper-container|tooltip_parent|drop-tips|mode-name-s2|camera-divider|mode-description/.test(m[1])).map(m=>({offset:m.index,source:m[0]}));
 receipts.push({product_id:pid,source:file,sha256:hash(source),wrapper,tooltip:tip,processing_mount:receipt(processRoot.offset,processRoot.end),groups,css:{path:`.ref/devices/${pid}/static/css/${cssFile}`,sha256:hash(css),rules}});
}
function write() {
 preview.write();
 fs.writeFileSync(path.join(root,'docs/re/camera-presentation-current-evidence.json'),JSON.stringify({schema_version:1,scanner_sha256:hash(fs.readFileSync(__filename)),products:receipts},null,2)+'\n');
}
module.exports={prepare,write};
