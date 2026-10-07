// Static source receipts and resource preparation; never evaluates vendor JS.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {execFileSync}=require('child_process');
const {Source,walk,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),directory='.ref/devices/2636';
const manifest=JSON.parse(fs.readFileSync(path.join(root,directory,'asset-manifest.json'),'utf8'));
const s=Object.assign(Object.create(Source.prototype),{directory,files:[...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.js')).map(f=>directory+'/'+f.replace(/^\.\//,'')),modules:new Map(),texts:new Map(),parsed:new Set()});
const file=directory+'/static/js/main.82d8a835.js',raw=s.text(file);
if(hash(raw)!=='a5b49ece13b13202b65e97d565b77ca61d22c8c6e14f9ec560dcee68d1155f9a')throw Error('2636 source changed; re-audit');
const receipts=[];
walk(acorn.parse(raw,{ecmaVersion:'latest'}),n=>{
 if(n.type==='VariableDeclarator'&&['XM','ZM','qM','zM','xM','aM','jM','$M'].includes(n.id.name)&&n.start>6500000)receipts.push({symbol:n.id.name,path:file,sha256:hash(raw),offset:n.init.start,end:n.init.end,source:raw.slice(n.init.start,n.init.end)});
 if(n.type==='ClassDeclaration'&&n.id.name==='ZM')receipts.push({symbol:'ZM',path:file,sha256:hash(raw),offset:n.start,end:n.end,source:raw.slice(n.start,n.end)});
 if(n.type==='Property'&&n.key.name==='controllerCalibrationReducer')receipts.push({symbol:'controllerCalibrationReducer',path:file,sha256:hash(raw),offset:n.value.start,end:n.value.end,source:raw.slice(n.value.start,n.value.end)});
 if(n.type==='VariableDeclarator'&&['Jh','eM','tM','Qh'].includes(n.id.name)&&n.start>6600000&&n.start<6680000)receipts.push({symbol:n.id.name,path:file,sha256:hash(raw),offset:n.init.start,end:n.init.end,source:raw.slice(n.init.start,n.init.end)});
});
const labels={};
for(const key of ['Kr7','dqO','YHw','R5i','Kx$','gEn','$JN','MKQ','bOp','DHM','w2s','KPZ','AGU','YmC','Jvl'])labels[key]=s.literal(54693,s.exported(54693,key));
const css=[];
for(const relative of [...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css'))){
 const file=directory+'/'+relative.replace(/^\.\//,''),text=fs.readFileSync(path.join(root,file),'utf8');
 const rules=parseCSS(text).filter(r=>/calibration|joystick|thumbstick-simulator|thumbstick-value|prodImage-container|step-indicator|popup-overlay|popup-content|popupTitle|header-warning|popup-description|left-thumbstick-overlay|right-thumbstick-overlay|lb-overlay|btnCancel|btnPrimary/.test(r.selector));
 if(rules.length)css.push({path:file,sha256:hash(text),rules});
}
const check=process.argv.includes('--check'),assets=[];
function output(file,bytes){bytes=Buffer.from(bytes);if(check){if(!fs.readFileSync(path.join(root,file)).equals(bytes))throw Error('Stale '+file);}else fs.writeFileSync(path.join(root,file),bytes);}
function asset(name,bytes,source){const file='assets/synapse/gamepad-2636-calibration-'+name;output(file,bytes);assets.push({output:file,sha256:hash(bytes),...source});}
// Deliberately limited AST interpretation for inert, source-authored SVGs.
function literal(node,steps){
 if(node.type==='Literal')return node.value;
 if(node.type==='Identifier'&&node.name==='t')return 'activeStep';
 if(node.type==='Identifier'&&node.name==='a')return 'pendingStep';
 if(node.type==='MemberExpression'&&node.property.name==='steps')return steps;
 if(node.type==='BinaryExpression'&&node.operator==='>=')return literal(node.left,steps)>=literal(node.right,steps);
 if(node.type==='ConditionalExpression')return literal(literal(node.test,steps)?node.consequent:node.alternate,steps);
 throw Error('Nonliteral SVG '+node.type);
}
function svg(node,steps){
 if(node.type!=='CallExpression'||node.arguments[0]?.type!=='Literal')throw Error('Unexpected SVG element');
 const name=node.arguments[0].value,props=node.arguments[1].properties;let attrs='',children='';
 for(const prop of props){const key=prop.key.name;
  if(key==='children'){children=(prop.value.type==='ArrayExpression'?prop.value.elements:[prop.value]).map(n=>svg(n,steps)).join('');continue;}
  if(key==='className'){const color=literal(prop.value,steps)==='activeStep'?'#44d62c':'#707070';attrs+=` fill="${color}" stroke="${color}"`;continue;}
  const attr=key.replace(/[A-Z]/g,m=>'-'+m.toLowerCase());attrs+=` ${key==='viewBox'?'viewBox':attr}="${literal(prop.value,steps)}"`;
 }
 return `<${name}${attrs}>${children}</${name}>`;
}
const stepper=acorn.parseExpressionAt(receipts.find(r=>r.symbol==='zM').source,0,{ecmaVersion:'latest'});let stepSvg;
walk(stepper,n=>{if(n.type==='CallExpression'&&n.arguments[0]?.value==='svg')stepSvg=n;});
for(let step=1;step<=6;step++)asset('steps-'+step+'.svg',svg(stepSvg,step+1)+'\n',{symbol:'zM'});
const wizard=acorn.parseExpressionAt(receipts.find(r=>r.symbol==='XM').source,0,{ecmaVersion:'latest'});let circle,bumper;
walk(wizard,n=>{if(n.type==='CallExpression'&&n.arguments[0]?.value==='svg'){const width=n.arguments[1].properties.find(p=>p.key.name==='width')?.value.value;if(width===38)circle=n;if(width==='64')bumper=n;}});
asset('thumbstick.svg',svg(circle)+'\n',{symbol:'XM.S'});
asset('bumper.svg',svg(bumper)+'\n',{symbol:'XM.u'});
const warning=receipts.find(r=>r.symbol==='jM').source;
if(!warning.includes('transformX:-.04,transformY:2,fill:"rgba(253, 134, 17, 1)"'))throw Error('Warning SVG transform changed');
asset('warning.svg',`<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><path d="${/d:"([^"]+)"/.exec(warning)[1]}" transform="translate(-0.04 2)" fill="#fd8611"/></svg>\n`,{symbol:'jM'});
// Canvas backgrounds translated to SVG geometry. Moving samples remain native
// observations; these static rings/arrows never encode a device position.
asset('simulator.svg','<svg xmlns="http://www.w3.org/2000/svg" width="162" height="162" viewBox="0 0 162 162"><circle cx="81" cy="81" r="78" stroke="#707070" stroke-width="2" fill="none"/><circle cx="81" cy="81" r="5" fill="#44d62c"/></svg>\n',{symbol:'aM'});
for(let step=1;step<=5;step++)for(const valid of [false,true]){
 const guide=step<5?`<rect x="73" y="69.5" width="75" height="11" rx="4" fill="url(#direction)" transform="rotate(${[0,0,90,180,-90][step]} 75 75)"/>`:'<path d="M75 7 A68 68 0 0 1 143 75" stroke="url(#rotation)" stroke-width="10" stroke-linecap="round" fill="none"/>';
 asset(`direction-${step}-${valid?'edge':'rest'}.svg`,`<svg xmlns="http://www.w3.org/2000/svg" width="150" height="150" viewBox="0 0 150 150"><defs><linearGradient id="direction" x1="72" y1="75" x2="147" y2="75" gradientUnits="userSpaceOnUse"><stop stop-color="#555" stop-opacity="0"/><stop offset="1" stop-color="#555"/></linearGradient><linearGradient id="rotation" x1="75" y1="7" x2="143" y2="75" gradientUnits="userSpaceOnUse"><stop stop-color="#555" stop-opacity="0"/><stop offset="1" stop-color="#555"/></linearGradient></defs><circle cx="75" cy="75" r="73" stroke="${valid?'white':'#707070'}" stroke-width="2" fill="#1f1f1f" fill-opacity="0.5"/>${guide}<circle cx="75" cy="75" r="3" fill="#bbb"/></svg>\n`,{symbol:'xM.C'});
}
const mwFile='.ref/middleware/2636/main.f0ca95429464cd09cbdb.js',mw=fs.readFileSync(path.join(root,mwFile),'utf8'),middleware=[];
walk(acorn.parse(mw,{ecmaVersion:'latest'}),n=>{
 if(n.type==='Property'&&n.key.value===66118)middleware.push({symbol:'module:66118',path:mwFile,sha256:hash(mw),offset:n.value.start,end:n.value.end,source:mw.slice(n.value.start,n.value.end)});
 if(n.type==='ClassDeclaration'&&n.start>1304000&&n.start<1306000||n.type==='Property'&&n.key.name==='ON_SET_CONTROLLER_CALIBRATION'&&n.start>1300000){middleware.push({path:mwFile,sha256:hash(mw),offset:n.start,end:n.end,source:mw.slice(n.start,n.end)});}
});
async function main(){
 for(const edition of [0,128,129]){
  const key=`./2636_${edition}/img_prods/0-3x.png`,match=raw.match(new RegExp('"'+key.replace(/[.*+?^${}()|[\]\\]/g,'\\$&')+'":\\[(\\d+),'));
  if(!match)throw Error('Missing edition image '+edition);
  const moduleId=Number(match[1]),source=s.snippet(moduleId,s.module(moduleId).fn),relative=/"(static\/media\/[^" ]+\.avif)"/.exec(source)?.[1];
  if(!relative)throw Error('Unexpected image module '+moduleId);
  const sourceFile=directory+'/'+relative,target=path.join(root,sourceFile),url='https://apps.razer.com/synapse/products/2636/ui/'+relative;
  if(!fs.existsSync(target)){if(check)throw Error('Missing '+sourceFile);const response=await fetch(url);if(!response.ok)throw Error('Image fetch '+response.status);fs.writeFileSync(target,Buffer.from(await response.arrayBuffer()));}
  const bytes=fs.readFileSync(target);if(bytes.toString('ascii',4,8)!=='ftyp')throw Error('Invalid AVIF '+sourceFile);
  const png=execFileSync('magick',[target,'-strip','PNG32:-'],{maxBuffer:16*1024*1024});
  asset(`edition-${edition}.png`,png,{source:sourceFile,source_sha256:hash(bytes),url,module:s.receipt(moduleId,s.module(moduleId).fn),edition});
 }
 output('assets/synapse/gamepad-2636-calibration-embedded.rs','&[\n'+assets.map(a=>{const file=path.basename(a.output);return `    ("synapse/${file}", include_bytes!("${file}") as &[u8]),`;}).join('\n')+'\n]\n');
 const evidence={product_id:2636,source:{path:file,sha256:hash(raw)},receipts,labels,css,middleware,assets};
 output('docs/re/gamepad-2636-calibration-current-evidence.json',JSON.stringify(evidence,null,2)+'\n');
 console.log(`2636 calibration: ${receipts.length} UI / ${middleware.length} MW AST receipts, ${assets.length} assets`);
}
main().catch(e=>{console.error(e);process.exitCode=1;});
