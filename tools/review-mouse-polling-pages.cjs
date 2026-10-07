// Per-product mounted Polling subtree review, without executing product bundles.
// A checked subtree is not a completed page or a completed product.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {hash,walk}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..'),check=process.argv.includes('--check');
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
const input='docs/re/mouse-page-source.json',pages=JSON.parse(read(input));
const configs=JSON.parse(read('docs/re/mouse-product-source.json'));
const native=JSON.parse(read('docs/re/native-product-coverage.json'));
const implementation=read('src/features/mouse_products.rs');
const rows=[];
for(const product of pages.products){
 const page=product.pages.find(p=>p.key==='TAB_PERFORMANCE');
 if(!page)continue;
 const identity=native.products.find(p=>p.product_id===product.product_id);
 const row={product_id:product.product_id,name:identity?.name??'',page_key:'TAB_PERFORMANCE',
  page_id:identity?.pages.find(p=>p.key===page.key)?.page_id??null,subtree:'Polling Rate',
  page_status:'partial; other controls and runtime not accepted',source:{path:page.path,sha256:page.sha256},receipts:[]};
 const source=read(page.path);
 const expectedHash=page.sha256??configs.products.find(p=>p.product_id===product.product_id)?.config.sha256;
 if(!page.path.startsWith(`.ref/devices/${product.product_id}/`)||hash(source)!==expectedHash)throw Error('Current source changed '+page.path);
 row.source.sha256=hash(source);
 if(product.product_id===162){
  // Rebuild this Babel module's actual lexical bindings from the current file.
  // The catalog entry stays 162 even though its current DeviceInfo says 163.
  const directory='.ref/devices/162',manifest=JSON.parse(read(directory+'/asset-manifest.json'));
  if(!Object.values(manifest.files).some(f=>directory+'/'+f.slice(2)===page.path))throw Error('162 file not declared');
  const pollingOffset=page.components.find(c=>c.source.includes('className:"polling-btn-set"'))?.offset;
  let owner;
  walk(acorn.parse(source,{ecmaVersion:'latest'}),n=>{
   if(/Function/.test(n.type)&&n.body?.type==='BlockStatement'
     &&n.start<pollingOffset&&n.end>page.offset){
    if(!owner||n.end-n.start<owner.fn.end-owner.fn.start)owner={fn:n,definitions:new Map()};
   }
  });
  if(!owner)throw Error('162 missing current lexical owner');
  for(const statement of owner.fn.body.body){
   if(statement.type==='VariableDeclaration')for(const d of statement.declarations){if(d.id.type==='Identifier'&&d.init)owner.definitions.set(d.id.name,d.init);}
   if(['FunctionDeclaration','ClassDeclaration'].includes(statement.type))owner.definitions.set(statement.id.name,statement);
  }
  const mount=page.components.find(c=>c.offset>=page.offset&&c.offset<page.offset+150&&/^\(0,\w+\.jsx\)/.test(c.source));
  if(!mount||source.slice(mount.offset,mount.end)!==mount.source)throw Error('162 mount changed');
  page.component=mount.source;
  page.components=[...owner.definitions].map(([symbol,node])=>{
   const jsx=[];
   walk(node,n=>{
    if(n.type==='CallExpression'&&n.callee.type==='SequenceExpression'&&['jsx','jsxs'].includes(n.callee.expressions.at(-1)?.property?.name))
     jsx.push({component:source.slice(n.arguments[0].start,n.arguments[0].end),offset:n.start});
   });
   return {symbol,offset:node.start,end:node.end,source:source.slice(node.start,node.end),jsx};
  });
  row.catalog_identity_note='Current entry 162 CONFIG declares productId 163; entry route is intentionally still 162.';
 }
 if(!page.component||!page.components.every(c=>c.symbol)){
  row.review='pending-source-mount';row.remaining=['Unsupported receipt schema requires separate mounted subtree review; not inferred from another product.'];rows.push(row);continue;
 }
 const candidates=page.components.filter(c=>c.source.includes('className:"polling-btn-set"')&&c.source.includes('className:"polling-warn"'));
 if(candidates.length!==1){
  row.review='pending-source-mount';row.remaining=['Current extractor has no unique mounted Polling subtree; do not infer from another product.'];rows.push(row);continue;
 }
 const target=candidates[0],byName=new Map(page.components.map(c=>[c.symbol,c]));
 const rootName=/\.jsx(?:s)?\)\(([^,]+)/.exec(page.component)?.[1];
 const edges=new Map();
 for(const c of page.components){
  const refs=new Set(c.jsx.map(x=>x.component).filter(x=>byName.has(x)));
  if(byName.has(c.source))refs.add(c.source);
  if(c.source.endsWith(')')){
   const last=/\(([\w$]+)\)$/.exec(c.source)?.[1];
   if(byName.has(last))refs.add(last);
  }
  edges.set(c.symbol,[...refs]);
 }
 const pending=[[rootName]],seen=new Set();let chain;
 while(pending.length){const path=pending.shift(),name=path.at(-1);if(seen.has(name))continue;seen.add(name);
  if(name===target.symbol){chain=path;break;}
  for(const next of edges.get(name)??[])pending.push([...path,next]);
 }
 if(!chain)throw Error('No mounted chain '+product.product_id+': '+target.symbol);
 for(const symbol of chain){
  const c=byName.get(symbol);
  if(source.slice(c.offset,c.end)!==c.source)throw Error('Stale subtree '+product.product_id+': '+symbol);
  const ast=acorn.parseExpressionAt(source,c.offset,{ecmaVersion:'latest'});
  // parseExpressionAt can include following comma expressions in a declaration.
  const node=ast.type==='SequenceExpression'?ast.expressions[0]:ast;
  if(node.start!==c.offset||node.end!==c.end)throw Error('Wrong AST boundary '+product.product_id+': '+symbol);
  row.receipts.push({symbol,offset:c.offset,end:c.end,source:c.source});
 }
 row.mount=chain;
 row.source_controls={description:target.source.includes('className:"h1-body"'),
  help:target.source.includes('tips:'),
  connection_title:target.source.includes('title:this.props.isDongle||this.props.isBle?'),
  wired_wireless_fields:target.source.includes('pollingRateWireless')&&target.source.includes('setPollingRateWireless'),
  high_rate_warning:target.source.includes('className:"polling-warn"'),
  official_link:target.source.includes('https://www.razer.com/technology/razer-hyperpolling#best-practices-tips'),
  ble_state_key:target.source.includes('getPollingRateStateKey'),
  dock_limit:target.source.includes('maxPollingRateHz'),
  dual_link_limit:target.source.includes('getDualLinkPollingRateLimitHz'),
  in_game_branch_in_shared_component:target.source.includes('this.props.supportInGamePollingRate')};
 row.review=product.product_id===226?'source-subtree-reviewed-local-fix':'source-subtree-reviewed-gap';
 row.remaining=product.product_id===226?
  ['Real topology publisher; source window focus/1500ms refresh; full layout/input/runtime. Shared scoped rate/firmware consumption is wired, but current read-query coverage is independently limited to product 182; no 226 read or device success is inferred. See mouse-ui-current.md.']:
  ['Native generic polling still omits source description and help.',
   'Native generic polling has no scoped connection/rate observation; connection-dependent field/title parity applies only where this product source mounts that branch.',
   'High-rate warning/link and conditional branches require this product-specific CONFIG/props review before reuse.',
   'The rest of this Performance page and other pages have not been accepted by this subtree review.'];
 rows.push(row);
}
const output={date:'2026-10-07',scope:'Each listed current mouse ordinary Performance Polling subtree only; not page/product completion',
 method:'Current file SHA + original UTF-16 slices + Acorn expression boundaries + root/alias/connect/JSX mount paths; no reference execution',
 inputs:[{path:input,sha256:hash(read(input))}],implementation:{path:'src/features/mouse_products.rs',sha256:hash(implementation)},
 summary:{pages:rows.length,source_subtrees:rows.filter(r=>r.review!=='pending-source-mount').length,
  local_fix:rows.filter(r=>r.review==='source-subtree-reviewed-local-fix').length,
  source_gaps:rows.filter(r=>r.review==='source-subtree-reviewed-gap').length,
  pending_mount:rows.filter(r=>r.review==='pending-source-mount').length,completed_pages:0,completed_products:0},rows};
const destination='docs/re/mouse-polling-pages-review-2026-10-07.json',text=JSON.stringify(output,null,2)+'\n';
if(check){if(read(destination)!==text)throw Error('Stale '+destination);}else fs.writeFileSync(path.join(root,destination),text);
console.log(JSON.stringify(output.summary));
