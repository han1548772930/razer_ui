// Link static JSX class tokens to manifest CSS rules, without browser execution.
const fs=require('fs'),path=require('path'),crypto=require('crypto'),zlib=require('zlib');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),read=p=>fs.readFileSync(path.join(root,p),'utf8'),sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const source=JSON.parse(zlib.gunzipSync(fs.readFileSync(path.join(root,'docs/re/all-product-page-chains-current.json.gz'))).toString('utf8')),cssCache=new Map(),products=[];
for(const product of source.products){
 const manifestPath=`.ref/devices/${product.product_id}/asset-manifest.json`,manifest=JSON.parse(read(manifestPath));
 const assets=Object.entries(manifest.files).map(([name,file])=>({name,path:`.ref/devices/${product.product_id}/${file.replace(/^\.\//,'').replace(/^\/?synapse\/products\/\d+\/ui\//,'')}`}));
 const cssFiles=assets.filter(a=>/\.css$/.test(a.path)),index=new Map(),css=[];
 for(const asset of cssFiles){
  const text=read(asset.path),hash=sha(text);let parsed=cssCache.get(hash);
  if(!parsed){parsed=parseCSS(text);cssCache.set(hash,parsed);}
  css.push({path:asset.path,sha256:hash,rules:parsed.length,fonts:[...text.matchAll(/@font-face\s*\{/g)].map(m=>({offset:m.index})),asset_urls:[...new Set([...text.matchAll(/url\(([^)]+)\)/g)].map(m=>m[1]))]});
  for(const rule of parsed){
   const classes=[...new Set([...rule.selector.matchAll(/\.(-?[_a-zA-Z]+[_a-zA-Z0-9-]*)/g)].map(m=>m[1]))];
   const receipt={path:asset.path,sha256:hash,offset:rule.offset,selector:rule.selector,conditions:rule.conditions,properties:rule.properties};
   for(const token of classes){if(!index.has(token))index.set(token,[]);index.get(token).push(receipt);}
  }
 }
 const pages=product.pages.map(page=>{
  const tokens=[...new Set(page.components.flatMap(c=>c.css_classes.flatMap(v=>v.value.split(/\s+/))).filter(Boolean))].sort(),used=new Map(),unmatched=[];
  for(const token of tokens){const matches=index.get(token);if(!matches){unmatched.push(token);continue;}for(const rule of matches)used.set(`${rule.path}:${rule.offset}`,rule);}
  return {page_id:page.page_id,page_key:page.page_key,display_mode:page.display_mode,static_class_tokens:tokens,unmatched_tokens:unmatched,css_rule_candidates:[...used.values()],semantic_layout_review:'not_completed_by_selector_candidates',runtime_validation:'not_run'};
 });
 products.push({product_id:product.product_id,manifest:{path:manifestPath,sha256:sha(read(manifestPath)),entrypoints:manifest.entrypoints},assets,css,pages});
 console.log(`Product ${product.product_id}: ${css.length} CSS files, ${pages.reduce((n,p)=>n+p.css_rule_candidates.length,0)} page-rule candidates`);
}
const summary={products:products.length,pages:products.reduce((n,p)=>n+p.pages.length,0),css_file_references:products.reduce((n,p)=>n+p.css.length,0),unique_css_hashes:cssCache.size,manifest_asset_references:products.reduce((n,p)=>n+p.assets.length,0),page_css_rule_candidates:products.reduce((n,p)=>n+p.pages.reduce((m,v)=>m+v.css_rule_candidates.length,0),0),visually_completed_pages_claimed:0};
const payload={schema_version:1,scanner_sha256:sha(fs.readFileSync(__filename)),page_graph_sha256:sha(read('docs/re/all-product-page-chains-current.json')),summary,limitations:['Class tokens are extracted from literal JSX className values only; computed expressions, CSS modules and runtime appended classes are not exhaustive.','Candidate selector matching is lexical, not browser matching: compound selectors, ancestors, media conditions, order, specificity, inheritance and imports still require semantic review.','Rule properties preserve original values, including colors/spacing/fonts, but do not establish computed style or actual layout.','Manifest resources preserve official paths; file declaration alone does not prove each lazy resource is mounted.','No application, vendor JavaScript, DLL, build or tests executed.'],products};
const full=Buffer.from(JSON.stringify(payload)+'\n'),compressed=zlib.gzipSync(full,{level:9});
fs.writeFileSync(path.join(root,'docs/re/all-product-layout-chains-current.json.gz'),compressed);
const index={schema_version:1,scanner_sha256:payload.scanner_sha256,page_graph_sha256:payload.page_graph_sha256,summary,limitations:payload.limitations,data_file:'all-product-layout-chains-current.json.gz',data_sha256:sha(compressed),uncompressed_sha256:sha(full),uncompressed_bytes:full.length,products:products.map(p=>({product_id:p.product_id,manifest:p.manifest,manifest_assets:p.assets.length,css_files:p.css.length,pages:p.pages.map(({css_rule_candidates,static_class_tokens,unmatched_tokens,...page})=>({...page,static_class_tokens:static_class_tokens.length,unmatched_tokens:unmatched_tokens.length,css_rule_candidates:css_rule_candidates.length}))}))};
fs.writeFileSync(path.join(root,'docs/re/all-product-layout-chains-current.json'),JSON.stringify(index,null,2)+'\n');console.log(JSON.stringify(summary));
