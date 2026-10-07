// Current Raptor component/event audit and inert resource preparation.
// Never loads or executes vendor JavaScript.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {subtree,hash,walk}=require('./current-page-subtree.cjs');
const {Source}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),check=process.argv.includes('--check');
const inventory=JSON.parse(fs.readFileSync(path.join(root,'docs/re/source-product-pages.json'),'utf8'));
const products=[],assets=[];
function output(file,bytes){bytes=Buffer.from(bytes);const target=path.join(root,file);if(check){if(!fs.existsSync(target)||!fs.readFileSync(target).equals(bytes))throw Error('Stale '+file);}else fs.writeFileSync(target,bytes);}
async function main(){
for(const pid of [3858,3880]){
 const product=inventory.products.find(p=>p.product_id===pid),directory='.ref/devices/'+pid;
 const manifest=JSON.parse(fs.readFileSync(path.join(root,directory,'asset-manifest.json'),'utf8'));
 const source=Object.assign(Object.create(Source.prototype),{directory,files:[...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.js')).map(f=>directory+'/'+f.replace(/^\.\//,'')),modules:new Map(),texts:new Map(),parsed:new Set()});
 const pages=product.pages.filter(p=>['TAB_GAMING','TAB_COLOR','TAB_DISPLAY'].includes(p.key)).map(page=>{
  const mounts=page.components.map(c=>{const proof=subtree(page,c);if(!proof)throw Error('Unreachable '+pid+' '+page.key+' '+c.symbol);return{symbol:c.symbol,mount:proof.mount};});
  return {key:page.key,path:page.path,sha256:page.sha256,root:page.component,mounts,components:page.components.map(({symbol,offset,end,source})=>({symbol,offset,end,source}))};
 });
 const file=pages[0].path,raw=source.text(file),wanted=pid===3858?['sSA','rSA','ISA']:['vSA','BSA','ySA'],bindings=[];
 walk(acorn.parse(raw,{ecmaVersion:'latest'}),node=>{if(node.type==='VariableDeclarator'&&wanted.includes(node.id.name)&&node.init.type==='BinaryExpression'&&node.init.right.type==='Literal')bindings.push({symbol:node.id.name,path:file,sha256:hash(raw),offset:node.init.start,end:node.init.end,source:raw.slice(node.init.start,node.init.end),relative:node.init.right.value});});
 const auto=source.module(6370),autoReceipt=source.receipt(6370,auto.fn),autoRelative=/"(static\/media\/[^" ]+\.svg)"/.exec(autoReceipt.source)?.[1];
 if(!autoRelative||bindings.length!==3)throw Error('Input icon imports changed '+pid);
 bindings.unshift({...autoReceipt,symbol:'module:6370',relative:autoRelative});
 for(const binding of bindings){
  const file=directory+'/'+binding.relative,target=path.join(root,file),url=`https://apps.razer.com/synapse/products/${pid}/ui/${binding.relative}`;
  if(!fs.existsSync(target)){if(check)throw Error('Missing '+file);const response=await fetch(url);if(!response.ok)throw Error('Icon fetch '+response.status+' '+url);fs.mkdirSync(path.dirname(target),{recursive:true});fs.writeFileSync(target,Buffer.from(await response.arrayBuffer()));}
  const bytes=fs.readFileSync(target),svg=bytes.toString('utf8');
  if(!svg.includes('<svg')||/<script|onload\s*=|<foreignObject|(?:href|src)\s*=\s*["'](?:https?:|javascript:)/i.test(svg))throw Error('Unexpected SVG content '+file);
  const outputFile='assets/synapse/monitor-'+path.basename(binding.relative);output(outputFile,bytes);
  assets.push({product_id:pid,output:outputFile,sha256:hash(bytes),source:file,url,binding});
 }
 const css=[...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css')).map(relative=>{
  const file=directory+'/'+relative.replace(/^\.\//,''),raw=fs.readFileSync(path.join(root,file),'utf8');return{path:file,sha256:hash(raw),rules:parseCSS(raw).filter(r=>/inputSource|btn_custom|featureDisabled|screen-refresh-container|img-text|PillsSelectBox|slide-off|slide-on|dropdown-area/.test(r.selector))};
 });
 products.push({product_id:pid,pages,css});
}
const unique=[...new Set(assets.map(a=>path.basename(a.output)))];
output('assets/synapse/monitor-input-embedded.rs','&[\n'+unique.map(file=>`    ("synapse/${file}", include_bytes!("${file}") as &[u8]),`).join('\n')+'\n]\n');
output('docs/re/monitor-pages-review-current-evidence.json',JSON.stringify({date:'2026-10-07',method:'Current-source AST boundaries and actual mounted component paths; UI implementation remains partial and was not executed.',products,assets},null,2)+'\n');
console.log(`Monitor review: ${products.length} products, ${products.reduce((n,p)=>n+p.pages.length,0)} pages, ${products.reduce((n,p)=>n+p.pages.reduce((n,p)=>n+p.mounts.length,0),0)} mounted component receipts, ${unique.length} unique assets`);
}
main().catch(error=>{console.error(error);process.exitCode=1;});
