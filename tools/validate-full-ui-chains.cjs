// Verify complete static archives against current files. Never load vendor JS.
const fs=require('fs'),path=require('path'),crypto=require('crypto'),zlib=require('zlib'),acorn=require('acorn');
const root=path.resolve(__dirname,'..'),sha=b=>crypto.createHash('sha256').update(b).digest('hex'),cache=new Map(),texts=new Map();
const assert=(condition,message)=>{if(!condition)throw Error(message);};
function file(name,expected,wantText=true){
 assert(name.startsWith('.ref/devices/')||name.startsWith('.ref/applications/')||name.startsWith('tools/')||name.startsWith('docs/re/'),`Invalid evidence root ${name}`);
 assert(!name.includes('..'),`Invalid path ${name}`);
 if(!cache.has(name)){const bytes=fs.readFileSync(path.join(root,name));cache.set(name,sha(bytes));texts.set(name,bytes.toString('utf8'));}
 assert(!expected||expected===cache.get(name),`Changed file ${name}`);
 if(!wantText)return;
 if(!texts.has(name))texts.set(name,fs.readFileSync(path.join(root,name),'utf8'));
 const text=texts.get(name);texts.delete(name);texts.set(name,text);while(texts.size>8)texts.delete(texts.keys().next().value);return text;
}
function archive(name,scanner){
 const index=JSON.parse(file(`docs/re/${name}.json`)),compressed=fs.readFileSync(path.join(root,'docs/re',index.data_file));
 assert(sha(compressed)===index.data_sha256,`Compressed hash ${name}`);
 const raw=zlib.gunzipSync(compressed);assert(raw.length===index.uncompressed_bytes,`Size ${name}`);assert(sha(raw)===index.uncompressed_sha256,`Content hash ${name}`);
 const data=JSON.parse(raw.toString('utf8'));assert(JSON.stringify(data.summary)===JSON.stringify(index.summary),`Summary ${name}`);file(scanner,data.scanner_sha256);return data;
}
function prefix(text,receipt,max){assert(receipt.offset>=0&&receipt.end<=text.length&&receipt.end>receipt.offset,'Invalid source range');assert(text.slice(receipt.offset,receipt.end).slice(0,max)===receipt.expression,`Changed expression @${receipt.offset}`);}
const apps=archive('all-application-chains-current','tools/extract-all-application-chains.cjs');
file('docs/re/application-catalog.json',apps.catalog_sha256);
for(const app of apps.applications){
 if(app.html)file(app.html.path,app.html.sha256,false);if(app.manifest)file(app.manifest.path,app.manifest.sha256,false);
 for(const css of app.css)file(css.path,css.sha256,false);
}
let appModules=0;
for(const source of apps.sources){
 const text=file(source.path,source.sha256);assert(source.parse_status==='parsed',`Unparsed application ${source.path}`);
 for(const module of source.modules){
  assert(module.offset>=0&&module.end<=text.length,'Invalid module bounds');
  if(/^function\b|^(?:\([^)]*\)|[\w$]+)\s*=>/.test(text.slice(module.offset,module.offset+150))){
   let expression;const bounded='('+text.slice(module.offset,module.end)+')';try{expression=acorn.parseExpressionAt(bounded,0,{ecmaVersion:'latest'});}catch(error){throw Error(`Module parse ${source.path}:${module.module_id}@${module.offset}: ${error}`);}
   const fn=expression.type==='SequenceExpression'?expression.expressions[0]:expression;
   assert(fn.start===1&&fn.end===bounded.length-1,`Module AST range ${source.path}:${module.module_id}`);
  } else assert(text[module.offset]==='(',`Unexpected concise module ${source.path}:${module.module_id}`);
  appModules++;
 }
 for(const receipt of [...source.imports,...source.state_observations,...source.message_candidates,...source.webpack_runtime_candidates,...source.root_mount_candidates])prefix(text,receipt,300);
}
console.log(`Application source ranges verified: ${apps.sources.length} JS / ${appModules} module factories`);
const graph=archive('all-product-page-chains-current','tools/extract-all-product-page-chains.cjs');
file('docs/re/product-registration-audit.json',graph.registration_sha256);
const registration=JSON.parse(file('docs/re/product-registration-audit.json')),products=new Map(registration.products.map(p=>[p.product_id,p]));
let productPages=0,componentReceipts=0,resolutionReceipts=0,resolutionEvidenceRanges=0;
for(const product of graph.products){
 const registry=products.get(product.product_id);assert(registry,`Unregistered product ${product.product_id}`);
 const entries=new Map(registry.navigation.flatMap(n=>n.items.map(i=>[i.key,{n,i}])));assert(entries.size===product.pages.length,`Product scope ${product.product_id}`);
 for(const page of product.pages){
  const {n,i}=entries.get(page.page_id);assert(n.sha256===page.navigation.sha256&&i.offset===page.navigation.item_offset&&i.component===page.navigation.component,`Navigation identity ${page.page_id}`);
  const navigationSource=file(page.navigation.path,page.navigation.sha256);productPages++;
  if(page.navigation.recovered_component){
   const recovered=page.navigation.recovered_component;assert(navigationSource.slice(recovered.offset,recovered.end)===recovered.expression,`Changed recovered root ${page.page_id}`);
   for(const nested of [recovered.render,recovered.render_view,recovered.render_call,recovered.navigation_name_guard,recovered.return_expression,...recovered.branches??[]].filter(Boolean))assert(navigationSource.slice(nested.offset,nested.end)===nested.expression,`Changed recovered root chain ${page.page_id}:${nested.offset}`);
   if(recovered.resolution==='owner_render_calls_renderView_once_then_unique_navigation_name_guard_returns_JSX'){
    const method=acorn.parseExpressionAt('('+recovered.render_view.expression+')',0,{ecmaVersion:'latest'}),guard=acorn.parseExpressionAt('('+recovered.navigation_name_guard.expression+')',0,{ecmaVersion:'latest'});
    assert(guard.type==='BinaryExpression'&&guard.operator==='===','Pairing name guard shape');
    const nodeText=(node,text)=>text.slice(node.start-1,node.end-1),nameSide=[guard.left,guard.right].find(node=>nodeText(node,recovered.navigation_name_guard.expression)===recovered.key_expression),other=guard.left===nameSide?guard.right:guard.left;
    assert(nameSide&&other.type==='MemberExpression'&&other.object.type==='Identifier'&&other.property.name==='name','Pairing exact name condition');
    const local=method.body.body.flatMap(s=>s.type==='VariableDeclaration'?s.declarations:[]).find(d=>d.id.name===other.object.name),lookup=local?.init;
    assert(lookup?.type==='CallExpression'&&lookup.callee.type==='MemberExpression'&&lookup.callee.property.name==='find'&&lookup.callee.object.type==='MemberExpression'&&lookup.callee.object.property.name==='navs'&&lookup.callee.object.object.type==='MemberExpression'&&lookup.callee.object.object.property.name==='state'&&lookup.callee.object.object.object.type==='ThisExpression','Pairing current navigation lookup');
    const predicate=lookup.arguments[0],test=predicate?.body;assert(predicate?.type==='ArrowFunctionExpression'&&test?.type==='BinaryExpression'&&test.operator==='===','Pairing selection predicate');
    const selected=test.right;assert(test.left.type==='MemberExpression'&&test.left.object.name===predicate.params[0]?.name&&test.left.property.name==='name'&&selected.type==='MemberExpression'&&selected.property.name==='active_view'&&selected.object.type==='MemberExpression'&&selected.object.property.name==='state'&&selected.object.object.type==='ThisExpression','Pairing exact active_view selection');
    assert(recovered.render.expression.includes('children:'+recovered.render_call.expression),'Pairing renderView mounted as children');
   }
  }
  for(const receipt of n.root_evidence?.chain??[])file(receipt.path,receipt.sha256);
  for(const component of page.components){
   const text=file(component.path,component.sha256);assert(component.offset>=0&&component.end<=text.length&&component.end>component.offset,`Component range ${component.path}:${component.offset}`);componentReceipts++;
   for(const receipt of [...component.state,...component.calls,...component.conditions])prefix(text,receipt,240);
   for(const cls of component.css_classes){const token=acorn.tokenizer(text.slice(cls.offset),{ecmaVersion:'latest'}).getToken();assert(token.type.label==='string'&&token.value===cls.value,`Changed literal class ${component.path}:${cls.offset}`);}
  }
  for(const resolution of page.resolutions??[]) {
   assert(resolution.from===null||(Number.isInteger(resolution.from)&&resolution.from>=0&&resolution.from<page.components.length),`Resolution owner ${page.page_id}`);
   assert(resolution.evidence?.length>0,`Unproved resolution ${page.page_id}:${resolution.kind}`);
   for(const receipt of resolution.evidence) {
    const text=file(receipt.path,receipt.sha256);
    assert(receipt.offset>=0&&receipt.end<=text.length&&receipt.end>receipt.offset,`Resolution range ${receipt.path}:${receipt.offset}`);
    assert(text.slice(receipt.offset,receipt.end)===receipt.source,`Changed resolution proof ${receipt.path}:${receipt.offset}`);
    resolutionEvidenceRanges++;
   }
   if(resolution.kind==='react_symbol_token') {
    const expression=acorn.parseExpressionAt(resolution.evidence[0].source,0,{ecmaVersion:'latest'});
    assert(expression.type==='CallExpression'&&expression.callee.type==='MemberExpression'&&expression.callee.object.name==='Symbol'&&expression.callee.property.name==='for'&&expression.arguments[0]?.value===resolution.target.value,`React symbol proof ${page.page_id}`);
   }
   if(resolution.kind==='webpack_require') {
    const expression=acorn.parseExpressionAt(resolution.evidence[0].source,0,{ecmaVersion:'latest'});
    assert(expression.type==='CallExpression'&&expression.arguments[0]?.value===resolution.target.module_id,`Require target proof ${page.page_id}`);
   }
   if(resolution.kind==='commonjs_require_reexport') {
    const proof=resolution.evidence[0].source,expression=acorn.parseExpressionAt(proof,0,{ecmaVersion:'latest'});
    assert(expression.type==='AssignmentExpression'&&expression.left.type==='MemberExpression'&&expression.left.property.name==='exports',`CommonJS reexport proof ${page.page_id}`);
   }
   resolutionReceipts++;
  }
  for(const unresolved of page.unresolved)for(const candidate of unresolved.candidate_factories??[]) {
    const text=file(candidate.path,candidate.sha256);
    assert(candidate.offset>=0&&candidate.end>candidate.offset&&candidate.end<=text.length,`Conflict factory bounds ${candidate.path}:${candidate.offset}`);
    const expression=acorn.parseExpressionAt('('+text.slice(candidate.offset,candidate.end)+')',0,{ecmaVersion:'latest'});
    assert(['FunctionExpression','ArrowFunctionExpression'].includes(expression.type)&&expression.start===1&&expression.end===candidate.end-candidate.offset+1,`Conflict factory source ${candidate.path}:${candidate.offset}`);
  }
 }
}
console.log(`Product source ranges verified: ${productPages} roots / ${componentReceipts} components / ${resolutionReceipts} resolutions (${resolutionEvidenceRanges} proof ranges)`);
const layouts=archive('all-product-layout-chains-current','tools/extract-all-product-layout-chains.cjs');
file('docs/re/all-product-page-chains-current.json',layouts.page_graph_sha256);
let ruleCandidates=0;
for(const product of layouts.products){
 file(product.manifest.path,product.manifest.sha256,false);for(const css of product.css)file(css.path,css.sha256,false);
 for(const page of product.pages)for(const rule of page.css_rule_candidates){file(rule.path,rule.sha256,false);ruleCandidates++;}
}
assert(appModules===apps.summary.webpack_modules,'App module count');assert(productPages===graph.summary.primary_pages+graph.summary.independent_pages,'Product page count');assert(componentReceipts===graph.summary.component_receipts,'Component count');assert(ruleCandidates===layouts.summary.page_css_rule_candidates,'CSS candidate count');
assert(resolutionReceipts===graph.summary.resolution_receipts,'Resolution count');
console.log(JSON.stringify({current_source_files_checked:cache.size,application_js_files:apps.sources.length,application_modules:appModules,product_pages:productPages,component_receipts:componentReceipts,resolution_receipts:resolutionReceipts,resolution_evidence_ranges:resolutionEvidenceRanges,css_rule_candidates:ruleCandidates,runtime_validation:'not_run',semantic_or_visual_completion_claimed:false}));
