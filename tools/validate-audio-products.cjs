// Validate static source receipts and native descriptor contracts; no app/tests.
const fs=require('fs'),path=require('path'),crypto=require('crypto');
const root=path.resolve(__dirname,'..'),read=p=>fs.readFileSync(path.join(root,p),'utf8');
const sha=p=>crypto.createHash('sha256').update(fs.readFileSync(path.join(root,p))).digest('hex');
const primary=JSON.parse(read('docs/re/audio-product-evidence.json'));
const additional=JSON.parse(read('docs/re/audio-additional-evidence.json'));
const products=JSON.parse(read('crates/razer-pages/src/features/audio_products_data.json'));
const coverage=JSON.parse(read('docs/re/audio-product-native-coverage.json'));
const assert=(ok,message)=>{if(!ok)throw Error(message);};
const scanner='tools/extract-audio-evidence.cjs';
assert(primary.scanner_sha256===sha(scanner),'Primary evidence parser has changed');
assert(additional.parser_sha256===sha(scanner),'Additional evidence parser has changed');
assert(additional.scanner_sha256===sha('tools/extract-audio-additional.cjs'),'Additional scanner has changed');
assert(coverage.generator_sha256===sha('tools/prepare-audio-products.cjs'),'Native descriptors are stale');
const sources=new Map();
for(const p of [...primary.products,...additional.products]){
 const productTexts=new Map();
 for(const file of p.source_files){if(!sources.has(file.path))sources.set(file.path,file.sha256);assert(sources.get(file.path)===file.sha256,'Conflicting source hash '+file.path);}
 for(const pg of p.pages)for(const c of pg.components){
  assert(!/^\{\d+:/.test(c.source),'Webpack object leaked into '+p.product_id+'/'+pg.key);
  if(!productTexts.has(c.path))productTexts.set(c.path,read(c.path));const text=productTexts.get(c.path);assert(text.slice(c.offset,c.end)===c.source,'Wrong component receipt '+c.path+':'+c.offset);
 }
}
for(const [file,digest] of sources)assert(sha(file)===digest,'Source bytes changed '+file);
for(const p of coverage.products)for(const receipt of p.supplementary_source??[]){
 assert(sources.has(receipt.path),'Supplementary source is not in the current product inventory');
 assert(read(receipt.path).slice(receipt.offset,receipt.end)===receipt.source,'Wrong supplementary source receipt');
}
function at(object,pointer){for(const part of pointer.split('/').slice(1)){if(object===null||object===undefined)return undefined;object=object[part];}return object;}
let controls=0,equalizers=0;
const empty=[];
for(const p of products){
 const source=[...primary.products,...additional.products].find(s=>s.product_id===p.product_id);
 assert(source,'No product source '+p.product_id);
 assert(JSON.stringify(p.pages.map(p=>p.key))===JSON.stringify(source.pages.map(p=>p.key)),'Navigation drift '+p.product_id);
 for(const pg of p.pages){
  if(!pg.sections.length)empty.push({product_id:p.product_id,page:pg.key});
  for(const section of pg.sections){
   if(section.visible_when)assert(at(p.draft,section.visible_when.path)!==undefined,'Missing visibility binding');
   for(const c of section.controls){controls++;const value=at(p.draft,c.path);assert(value!==undefined,'Missing binding '+p.product_id+c.path);
    if(c.enabled_by)assert(typeof at(p.draft,c.enabled_by)==='boolean','Invalid enable binding '+c.enabled_by);
    for(const gate of c.enabled_all??[])assert(typeof at(p.draft,gate)==='boolean','Invalid combined enable binding '+p.product_id+gate);
    if(c.kind==='toggle')assert(typeof value==='boolean','Invalid toggle '+c.path);
    if(c.kind==='slider')assert(typeof value==='number'&&Number.isFinite(value)&&c.min<c.max&&c.step>0&&value>=c.min&&value<=c.max,'Invalid slider '+p.product_id+c.path);
    if(c.kind==='select')assert(c.options.length>0&&c.options.some(o=>JSON.stringify(o.value)===JSON.stringify(value)),'Invalid selection '+p.product_id+c.path);
    if(c.kind==='options')assert(c.options.length>0&&c.options.some(o=>JSON.stringify(o.value)===JSON.stringify(value)),'Invalid option set '+p.product_id+c.path);
    if(c.kind==='image_options'){
     assert(c.options.length>0&&c.options.some(o=>JSON.stringify(o.value)===JSON.stringify(value)),'Invalid image option set '+p.product_id+c.path);
     // Every preview tile must point at a prepared local asset, not a source URL.
     for(const o of c.options)if(o.image!==undefined)assert(/^synapse\/[^/]+\.(?:webp|png|gif|avif)$/.test(o.image),'Unprepared preview asset '+o.image);
    }
    if(c.kind==='link')assert(/^https:\/\/(?:www\.razer\.com|mysupport\.razer\.com)\//.test(c.url),'Unrecognized external link');
    for(const peer of c.exclusive_with??[])assert(typeof at(p.draft,peer)==='boolean','Invalid mutually-exclusive binding');
   }
  }
 }
 for(const eq of p.equalizers){equalizers++;assert(eq.min<eq.max&&eq.frequencies.length>0,'Invalid EQ bounds');assert(eq.presets.some(p=>p.key===eq.selected),'Unknown EQ preset');
  for(const preset of eq.presets)assert(preset.bands.length===eq.frequencies.length&&preset.bands.every(v=>typeof v==='number'&&v>=eq.min&&v<=eq.max),'Invalid EQ data '+p.product_id+'/'+eq.key+'/'+preset.key);
 }
}
const result={schema_version:1,verification:'Static receipts and descriptor validation only; no application, build or tests executed',parser_sha256:sha(scanner),generator_sha256:sha('tools/prepare-audio-products.cjs'),rust_sha256:sha('crates/razer-pages/src/features/audio_products.rs'),data_sha256:sha('crates/razer-pages/src/features/audio_products_data.json'),products:products.length,source_files:sources.size,controls,equalizers,empty_pages:empty};
fs.writeFileSync(path.join(root,'docs/re/audio-product-validation.json'),JSON.stringify(result,null,2)+'\n');
console.log(`Validated ${products.length} products, ${controls} controls, ${equalizers} EQs and ${sources.size} current source hashes; ${empty.length} empty pages remain explicit.`);
