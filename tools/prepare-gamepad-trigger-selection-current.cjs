// Prepare inert source SVGs and syntax-authored geometry. Never evaluates JS.
const fs=require('fs'),path=require('path'),acorn=require('acorn'),crypto=require('crypto');
const {parseCSS}=require('./css-source.cjs');
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
const walk=(node,visit)=>{if(!node?.type)return;visit(node);for(const v of Object.values(node)){if(Array.isArray(v))for(const c of v)walk(c,visit);else if(v?.type)walk(v,visit);}};
const key=n=>n?.name??n?.value;
const props=n=>Object.fromEntries(n.properties.filter(p=>p.type==='Property').map(p=>[key(p.key),p.value]));
const escape=s=>String(s).replace(/&/g,'&amp;').replace(/"/g,'&quot;').replace(/</g,'&lt;');
const attrName=s=>['viewBox','gradientUnits','preserveAspectRatio'].includes(s)?s:s.replace(/[A-Z]/g,m=>'-'+m.toLowerCase());
function inertElement(tag,attributes){return `<${tag} `+Object.entries(attributes).map(([k,v])=>`${attrName(k==='className'?'class':k)}="${escape(v)}"`).join(' ')+'/>';}
const assets=[],bindings=[],animations=[],styles=[];
function output(file,bytes,source){bytes=Buffer.from(bytes);fs.writeFileSync(file,bytes);assets.push({output:file,sha256:hash(bytes),...source});}
for(const pid of [2676,2684]){
 const file=`docs/re/gamepad-${pid}-trigger-resources-live-source.json`,r=JSON.parse(fs.readFileSync(file,'utf8'));
 const raw=fs.readFileSync(r.source,'utf8'),dir=r.source.split('/static/js/')[0],jsdir=dir+'/static/js';
 const receipt=symbol=>{const x=r.receipts.find(x=>x.symbol===symbol);if(!x)throw Error('Missing '+symbol);return x;};
 const ast=symbol=>acorn.parseExpressionAt(receipt(symbol).source,0,{ecmaVersion:'latest',preserveParens:true});
 // Images are chosen from the actual per-product webpack contexts, independently.
 for(const kind of ['prd','ActuationDevice']){
  const matches=[...raw.matchAll(new RegExp('"\\./'+pid+'_(\\d+)/svg_prods/'+kind+'\\.svg":\\[(\\d+),1,(\\d+)\\]','g'))],seen=new Set();
  for(const m of matches){
   if(seen.has(m[1]))continue;seen.add(m[1]);
   const chunk=fs.readdirSync(jsdir).find(f=>f.startsWith(m[3]+'.')&&f.endsWith('.js'));
   if(!chunk)throw Error('Missing '+pid+'/'+m[3]);
   const bytes=fs.readFileSync(jsdir+'/'+chunk),media=/"(static\/media\/[^" ]+\.svg)"/.exec(bytes.toString());
   if(!media)throw Error('SVG chunk lacks static media '+chunk);
   const source=dir+'/'+media[1],image=fs.readFileSync(source),out=`assets/synapse/gamepad-${pid}-calibration-${kind==='prd'?'selection':'trigger-artwork'}-edition-${m[1]}.svg`;
   output(out,image,{source,source_sha256:hash(image),source_bytes:image.length,module:Number(m[2]),chunk:jsdir+'/'+chunk,chunk_sha256:hash(bytes),edition:Number(m[1]),kind,product_id:pid});
   bindings.push([pid,Number(m[1]),kind,out]);
  }
  if(!seen.has('0'))throw Error('No base edition '+pid+'/'+kind);
 }
 const connector=receipt(pid===2676?'Xm':'Nc'),paths=[];
 walk(ast(connector.symbol),n=>{
  if(n.type!=='CallExpression'||n.arguments[0]?.value!=='path'||n.arguments[1]?.type!=='ObjectExpression')return;
  const values=Object.fromEntries(Object.entries(props(n.arguments[1])).map(([k,v])=>{if(v.type!=='Literal')throw Error('Dynamic connector '+k);return[k,v.value];}));
  paths.push(values);
 });
 if(paths.length!==4)throw Error('Expected exactly four source connectors');
 for(const [part,side] of [[0,null],[1,'left-thumbstick'],[2,'right-thumbstick'],[3,'left-trigger'],[4,'right-trigger']]){
  const nodes=paths.map(p=>({...p,stroke:side&&p.className.endsWith('--'+side)?'#FFFFFF':p.stroke}));
  output(`assets/synapse/gamepad-${pid}-calibration-connectors-${part}.svg`,'<svg width="292" height="304" viewBox="0 0 292 304" fill="none" xmlns="http://www.w3.org/2000/svg">'+nodes.map(p=>inertElement('path',p)).join('')+'</svg>\n',{source:r.source,source_sha256:r.sha256,receipt:file,symbol:connector.symbol,offset:connector.offset,end:connector.end,product_id:pid,preparation:'Literal source createElement paths serialized as inert SVG; only the hovered connector stroke follows source CSS (#FFFFFF).'});
 }
 // The two-step calibration stepper is distinct from the six-step stick wizard.
 const stepper=receipt(pid===2676?'dm':'zl');let stepSvg;
 walk(ast(stepper.symbol),n=>{if(n.type==='CallExpression'&&n.arguments[0]?.value==='svg'&&props(n.arguments[1]).width?.value==='90.25')stepSvg=n;});
 if(!stepSvg)throw Error('No two-step indicator');
 function value(n,step){
  if(n.type==='Literal')return n.value;
  if(n.type==='Identifier'&&n.name==='t')return step;
  if(n.type==='Identifier')return n.name==='n'&&pid===2676||n.name==='i'&&pid===2684?'active':'pending';
  if(n.type==='BinaryExpression'&&n.operator==='>=')return value(n.left,step)>=value(n.right,step);
  if(n.type==='ConditionalExpression')return value(value(n.test,step)?n.consequent:n.alternate,step);
  throw Error('Non-inert stepper '+n.type);
 }
 function jsx(n,step){
  if(n.type!=='CallExpression'||typeof n.arguments[0]?.value!=='string')throw Error('Unexpected source SVG');
  const tag=n.arguments[0].value,p=props(n.arguments[1]);let attributes='',children='';
  for(const [k,n] of Object.entries(p)){
   if(k==='children'){children=(n.type==='ArrayExpression'?n.elements:[n]).map(x=>jsx(x,step)).join('');continue;}
   if(k==='className'){const c=value(n,step)==='active'?'#44D62C':'#707070';attributes+=` fill="${c}" stroke="${c}"`;continue;}
   attributes+=` ${attrName(k)}="${escape(value(n,step))}"`;
  }
  return `<${tag}${attributes}>${children}</${tag}>`;
 }
 for(const step of [1,2])output(`assets/synapse/gamepad-${pid}-trigger-steps-${step}.svg`,jsx(stepSvg,step+.5)+'\n',{source:r.source,source_sha256:r.sha256,receipt:file,symbol:stepper.symbol,offset:stepper.offset,end:stepper.end,product_id:pid,preparation:'Source two-step JSX indicator serialized with the original active/pending CSS colors.'});
 // Keep source meter paths/gradient properties verbatim; Rust fills only the
 // source-controlled coordinates, colors, direction and visibility placeholders.
 const meter=receipt(pid===2676?'FL':'Sl'),literalPaths=[];
 walk(ast(meter.symbol),n=>{if(n.type==='CallExpression'&&n.arguments[0]?.value==='path'&&n.arguments[1]?.type==='ObjectExpression'){const p=props(n.arguments[1]);if(p.d?.type==='Literal')literalPaths.push(p.d.value);}});
 if(literalPaths.length!==2)throw Error('Unexpected calibration meter paths');
 const template=`<svg width="74.94" height="138.91" viewBox="0 0 77 141" preserveAspectRatio="xMidYMid meet" xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="prompt" x1="70.4707" y1="38.4853" x2="6.50006" y2="38.4853" gradientUnits="userSpaceOnUse"><stop stop-color="#555555" stop-opacity="0"/><stop offset="1" stop-color="#555555"/></linearGradient></defs><g transform="{{mirror}}"><path d="${literalPaths[0]}" fill="none" stroke="url(#prompt)" stroke-width="13" stroke-linecap="round" opacity="{{prompt_opacity}}" transform="rotate({{prompt_angle}} 70.4707 70.4707)"/><path d="${literalPaths[1]}" fill="none" stroke="{{track}}" stroke-linecap="round"/><circle cx="{{x}}" cy="{{y}}" r="5.485" fill="#44D62C"/></g></svg>\n`;
 output(`assets/synapse/gamepad-${pid}-trigger-meter-template.svg`,template,{source:r.source,source_sha256:r.sha256,receipt:file,symbol:meter.symbol,offset:meter.offset,end:meter.end,product_id:pid,preparation:'Literal source calibration meter path data and gradient geometry, with placeholders only for source-controlled direction, marker coordinates, track color and prompt animation.'});
 const timer=receipt(pid===2676?'ep':'xc');
 output(`assets/synapse/gamepad-${pid}-trigger-timer-template.svg`,'<svg width="100" height="100" viewBox="0 0 100 100" xmlns="http://www.w3.org/2000/svg"><path d="M 50 50 L 50 0 A 50 50 0 {{large_arc}} 1 {{x}} {{y}} Z" fill="#44D62C" opacity="0.3"/></svg>\n',{source:r.source,source_sha256:r.sha256,receipt:file,symbol:timer.symbol,offset:timer.offset,end:timer.end,product_id:pid,preparation:'Exact source timer sector template. Rust applies the source clamped percentage formula; at 100% source uses a full circle, at 0% there is no timer SVG.'});
 const warning=receipt(pid===2676?'pm':'ac');let warningProps;
 walk(ast(warning.symbol),n=>{if(n.type==='CallExpression'&&n.arguments[1]?.type==='ObjectExpression'&&props(n.arguments[1]).d?.type==='Literal')warningProps=props(n.arguments[1]);});
 if(!warningProps)throw Error('No source warning path');
 output(`assets/synapse/gamepad-${pid}-trigger-warning.svg`,`<svg width="20" height="20" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg"><path d="${warningProps.d.value}" transform="translate(${warningProps.transformX.value} ${warningProps.transformY.value})" fill="${warningProps.fill.value}"/></svg>\n`,{source:r.source,source_sha256:r.sha256,receipt:file,symbol:warning.symbol,offset:warning.offset,end:warning.end,product_id:pid,preparation:'Literal source warning path and source translation/fill serialized as SVG.'});
 for(const sheet of parentCss(pid)){
  const text=fs.readFileSync(sheet.path,'utf8');
  const rules=parseCSS(text).filter(r=>/^\.btnCancel(?:[ ,:{]|$)|^\.btnPrimary(?:[ ,:{]|$)|actuation-device-artwork|body-widgets \.widget|^\.widget \.titleRow|customize-setting-button/.test(r.selector));
  styles.push({product_id:pid,path:sheet.path,sha256:hash(text),rules});
  for(const name of ['trigger-calibration-prompt-sweep','trigger-calibration-prompt-fade']){
   const offset=text.indexOf('@keyframes '+name);if(offset<0)continue;
   const start=text.indexOf('{',offset);let depth=1,end=start+1;
   for(;end<text.length&&depth;end++){if(text[end]==='{')depth++;else if(text[end]==='}')depth--;}
   const source=text.slice(offset,end);animations.push({product_id:pid,path:sheet.path,sha256:hash(text),offset,end,name,source});
   console.log(pid,name,source);
  }
 }
}
function parentCss(pid){return JSON.parse(fs.readFileSync(`docs/re/gamepad-${pid}-calibration-live-source.json`,'utf8')).css;}
fs.writeFileSync('assets/synapse/gamepad-trigger-selection-embedded.rs','&[\n'+assets.map(a=>`    ("synapse/${path.basename(a.output)}", include_bytes!("${path.basename(a.output)}") as &[u8]),`).join('\n')+'\n]\n');
fs.writeFileSync('assets/synapse/gamepad-calibration-svg-sources.rs','&[\n'+bindings.map(([pid,edition,kind,out])=>`    (${pid}, ${edition}, "${kind}", "synapse/${path.basename(out)}"),`).join('\n')+'\n]\n');
fs.writeFileSync('docs/re/gamepad-trigger-selection-resources-current.json',JSON.stringify({preparation_tool:'tools/prepare-gamepad-trigger-selection-current.cjs',assets,bindings,animations,styles},null,2)+'\n');
const manifest=JSON.parse(fs.readFileSync('assets/synapse/manifest.json','utf8'));
for(const a of assets){
 const sourceBytes=fs.readFileSync(a.source);if(hash(sourceBytes)!==a.source_sha256)throw Error('Source hash changed '+a.source);
 manifest.entries=manifest.entries.filter(e=>e.output!==a.output);
 manifest.entries.push({...a,source_url:'https://apps.razer.com/'+a.source.split('apps.razer.com/')[1],source_bytes:sourceBytes.length,output_bytes:fs.statSync(a.output).size,preparation:a.preparation||'Exact original per-product, per-edition SVG bytes selected by current webpack context. No substitution or redraw.',evidence:'docs/re/gamepad-trigger-selection-resources-current.json'});
}
fs.writeFileSync('assets/synapse/manifest.json',JSON.stringify(manifest,null,2)+'\n');
console.log(JSON.stringify({assets:assets.length,bindings}));
