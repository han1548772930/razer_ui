// Source/resource preparation only; never execute downloaded code.
const fs=require('fs'),path=require('path');
const {Source,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),check=process.argv.includes('--check');
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
const directory='.ref/devices/226',manifestPath=directory+'/asset-manifest.json';
const manifest=JSON.parse(read(manifestPath));
const source=Object.assign(Object.create(Source.prototype),{directory,
 files:[...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.js')).map(f=>directory+'/'+f.slice(2)),
 modules:new Map(),texts:new Map(),parsed:new Set()});
source.parse(directory+'/static/js/8355.3d5e573e.chunk.js');
const receipts=['wi','Qs','Ks','qs','ei','x'].map(symbol=>({symbol,...source.receipt(4125,source.binding(4125,symbol))}));
const labels={};
for(const symbol of ['aqm','yer','cDK']){
 labels[symbol]=source.literal(4693,source.exported(4693,symbol));
 receipts.push({symbol:'label:'+symbol,...source.receipt(4693,source.binding(4693,source.exported(4693,symbol).name))});
}
const css=[];
for(const relative of [...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css'))){
 const file=directory+'/'+relative.slice(2),text=read(file);
 const rules=parseCSS(text).filter(r=>/^\.img-text(?: |\{|$)/.test(r.selector)&&/windows|external|^\.img-text$/.test(r.selector));
 if(rules.length)css.push({path:file,sha256:hash(text),rules});
}
(async()=>{
 const assets=[];
 for(const [key,name] of [['windows_logo.svg','mouse-226-properties-legacy.svg'],['common-windows-11.svg','mouse-226-properties-win11.svg']]){
  const relative=manifest.files['static/media/'+key].slice(2),file=directory+'/'+relative;
  const url='https://apps.razer.com/synapse/products/226/ui/'+relative;
  if(!fs.existsSync(path.join(root,file))){
   if(check)throw Error('Missing resource '+file);
   const response=await fetch(url);
   if(!response.ok)throw Error('HTTP '+response.status+' '+url);
   const bytes=Buffer.from(await response.arrayBuffer());
   if(!bytes.toString('utf8').includes('<svg'))throw Error('Expected SVG '+url);
   fs.writeFileSync(path.join(root,file),bytes);
   fs.writeFileSync(path.join(root,file+'.http.json'),JSON.stringify({url,final_url:response.url,status:response.status,
    fetched_at:new Date().toISOString(),bytes:bytes.length,sha256:hash(bytes),etag:response.headers.get('etag'),last_modified:response.headers.get('last-modified')},null,2)+'\n');
  }
  const bytes=fs.readFileSync(path.join(root,file)),output='assets/synapse/'+name;
  if(check){if(!bytes.equals(fs.readFileSync(path.join(root,output))))throw Error('Asset differs '+output);}
  else fs.writeFileSync(path.join(root,output),bytes);
  assets.push({source:file,source_url:url,output,sha256:hash(bytes)});
 }
 const file='docs/re/mouse-226-properties-current-evidence.json';
 const output=JSON.stringify({product_id:226,method:'Static AST/CSS and SVG byte comparisons only; UTF-16 offsets',
  manifest:{path:manifestPath,sha256:hash(read(manifestPath))},receipts,labels,css,assets},null,2)+'\n';
 if(check){if(read(file)!==output)throw Error('Stale '+file);}else fs.writeFileSync(path.join(root,file),output);
 console.log(`226 Mouse Properties: ${receipts.length} AST; ${css.reduce((n,c)=>n+c.rules.length,0)} CSS; ${assets.length} SVG`);
})().catch(error=>{console.error(error);process.exitCode=1;});
