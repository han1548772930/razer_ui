// Manifest-declared SVG data only. Downloaded JavaScript is parsed, never run.
const fs=require('fs'),path=require('path');
const {source:s}=require('./audit-chroma-studio.cjs');
const {hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),check=process.argv.includes('--check');
async function main(){
 const manifest=JSON.parse(fs.readFileSync(path.join(root,s.directory,'asset-manifest.json'),'utf8'));
 const assets=[];
 for(const name of ['checkered','eyedropper-white','add-gray']){
  const relative=Object.values(manifest.files).find(v=>new RegExp(`/static/media/${name}\\.[a-f0-9]+\\.svg$`).test(v));
  if(!relative)throw Error(`Missing declared ${name}`);
  const source=`${s.directory}/${relative.slice(2)}`,target=`assets/synapse/chroma-studio-${name}.svg`;
  const sourcePath=path.join(root,source);
  if(!fs.existsSync(sourcePath)){
   if(check)throw Error(`Missing ${source}`);
   const response=await fetch(`https://apps.razer.com/synapse/chroma-studio/${relative.slice(2)}`);
   if(!response.ok)throw Error(`HTTP ${response.status}`);
   const bytes=Buffer.from(await response.arrayBuffer());
   if(!bytes.toString().includes('<svg')||/<script\b/i.test(bytes.toString()))throw Error('Not inert SVG');
   fs.mkdirSync(path.dirname(sourcePath),{recursive:true});fs.writeFileSync(sourcePath,bytes);
  }
  const bytes=fs.readFileSync(sourcePath);
  if(check){if(!fs.readFileSync(path.join(root,target)).equals(bytes))throw Error(`Stale ${target}`);}
  else fs.writeFileSync(path.join(root,target),bytes);
  assets.push({source,target,sha256:hash(bytes)});
 }
 const pointer='<svg xmlns="http://www.w3.org/2000/svg" width="8" height="4" viewBox="0 0 8 4"><path fill="#fff" d="M0 0h8L4 4Z"/></svg>\n';
 const target=path.join(root,'assets/synapse/chroma-studio-brightness-pointer.svg');
 if(check){if(fs.readFileSync(target,'utf8')!==pointer)throw Error('Stale pointer');}else fs.writeFileSync(target,pointer);
 const selector='.color-picker>.tools>.slider-brightness .thumb:before';
 let pointerSource;
 for(const relative of new Set(Object.values(manifest.files).filter(v=>v.endsWith('.css')))){
  const source=`${s.directory}/${relative.slice(2)}`,raw=fs.readFileSync(path.join(root,source),'utf8');
  const rule=parseCSS(raw).find(r=>r.selector===selector);
  if(rule){pointerSource={source,sha256:hash(raw),rule};break;}
 }
 if(!pointerSource||!pointerSource.rule.declarations.includes('border-top:4px solid #fff')||!pointerSource.rule.declarations.includes('border-left:4px solid #0000')||!pointerSource.rule.declarations.includes('border-right:4px solid #0000'))throw Error('Changed source pointer');
 const evidence={method:'Current manifest-declared SVG bytes; brightness pointer is the exact 4px CSS border triangle.',assets,pointer_source:pointerSource,receipts:[1698,9170,4809,3181].map(id=>({module:id,...s.receipt(id,s.module(id).fn)}))};
 const out=JSON.stringify(evidence,null,2)+'\n',file=path.join(root,'docs/re/chroma-studio-color-source.json');
 if(check){if(fs.readFileSync(file,'utf8')!==out)throw Error('Stale color receipts');}else fs.writeFileSync(file,out);
 const records=assets.map(a=>({source:a.source,source_sha256:a.sha256,output:a.target,output_sha256:a.sha256}));
 records.push({source:pointerSource.source,source_sha256:pointerSource.sha256,output:'assets/synapse/chroma-studio-brightness-pointer.svg',output_sha256:hash(pointer),method:'4px CSS border triangle'});
 const embedded='// Generated from current Studio color source.\n&[\n'+records.map(r=>`    ("${r.output.slice(7)}", include_bytes!("${path.basename(r.output)}")),`).join('\n')+'\n]\n';
 for(const [relative,text] of [['assets/synapse/chroma-studio-color-assets.json',JSON.stringify(records,null,2)+'\n'],['assets/synapse/chroma-studio-color-embedded.rs',embedded]]){
  const target=path.join(root,relative);
  if(check){if(fs.readFileSync(target,'utf8')!==text)throw Error(`Stale ${relative}`);}else fs.writeFileSync(target,text);
 }
 console.log('Studio color: 3 declared SVGs, source CSS pointer, 4 current module receipts.');
}
main().catch(e=>{console.error(e);process.exitCode=1;});
