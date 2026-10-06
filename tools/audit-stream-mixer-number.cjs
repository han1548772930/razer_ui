// Current 3334/3337 stepper receipts. Vendor JavaScript is parsed, never executed.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {walk,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),read=file=>fs.readFileSync(path.join(root,file),'utf8');
const products=[];
for(const pid of [3334,3337]){
 const directory=`.ref/devices/${pid}`,manifest=JSON.parse(read(directory+'/asset-manifest.json'));
 const main=directory+'/'+manifest.files['main.js'].replace(/^\.\//,'');
 const source=read(main),receipts=[];
 walk(acorn.parse(source,{ecmaVersion:'latest'}),node=>{
  const name=node.type==='ClassDeclaration'?node.id?.name:node.type==='VariableDeclarator'?node.id?.name:null;
  if(node.start<4400000||!['AU','SU','uU'].includes(name))return;
  receipts.push({symbol:name,path:main,sha256:hash(source),offset:node.start,end:node.end,source:source.slice(node.start,node.end)});
 });
 for(const name of ['AU','SU','uU'])if(receipts.filter(r=>r.symbol===name).length!==1)throw Error(`Missing ${pid}:${name}`);
 const css=[];
 for(const relative of [...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css'))){
  const file=directory+'/'+relative.replace(/^\.\//,''),text=read(file);
  const rules=parseCSS(text).filter(r=>/stepper|spinner|level-input|adjust-level-container/.test(r.selector));
  if(rules.length)css.push({path:file,sha256:hash(text),rules});
 }
 const assets=[];
 for(const direction of ['up','down']){
  const file=directory+'/'+manifest.files[`static/media/stepper_${direction}.svg`].replace(/^\.\//,'');
  const output=`assets/synapse/stepper-${direction}.svg`,bytes=fs.readFileSync(path.join(root,file));
  if(!bytes.equals(fs.readFileSync(path.join(root,output))))throw Error(`Shared stepper asset differs: ${pid} ${direction}`);
  assets.push({source:file,output,sha256:hash(bytes)});
 }
 products.push({product_id:pid,receipts,css,assets});
}
const target=path.join(root,'docs/re/stream-mixer-number-current-evidence.json');
const output=JSON.stringify({method:'Static source parsing and resource comparison only',products},null,2)+'\n';
if(process.argv.includes('--check')){if(fs.readFileSync(target,'utf8')!==output)throw Error('Stale Stream Mixer number receipts');}
else fs.writeFileSync(target,output);
console.log('Stream Mixer number: 2 products, 6 AST receipts, 4 shared asset comparisons');
