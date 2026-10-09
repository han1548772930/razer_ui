// Static control semantics and manifest/page-reference receipts. Never evaluate vendor JS.
const fs=require('fs'),path=require('path'),crypto=require('crypto'),zlib=require('zlib');
const {Corpus,walk,key}=require('./extract-all-product-page-chains.cjs');
const {parseCSS,declarations}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..');
const read=p=>fs.readFileSync(path.join(root,p),'utf8');
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
const specs=[
 ['slider',70,130,'T'],['legacy-slider',70,801,'n'],['number',70,4230,'o'],
 ['dropdown',241,7734,'d'],['dropdown-option',241,7278,'c'],
 ['keymap-dropdown',70,3317,'O'],['rename-input',241,9163,'ot'],
 ['delete-popup',241,9163,'et'],['action-popup',241,9163,'ie'],
 ['tab',241,9163,'z'],['nav',241,9163,'We'],['profile',241,9163,'yi'],
 ['modal',241,3746,'Z'],['modal-root',241,3746,'X'],
 ['widget-tooltip',241,6299,'p'],['positioned-tooltip',241,8837,'u'],
 ['cursor-observer',241,8837,'n'],['macro-draft',70,2508,'P'],
];
const corpora=new Map([70,241].map(pid=>[pid,new Corpus(pid)]));
function receipt(c,m,n){const d=c.file(m.file);return{path:m.file,sha256:d.sha256,offset:n.start,end:n.end,source:d.text.slice(n.start,n.end)};}
function binding(c,m,name){const b=c.lookup(name,c.nodeScopes.get(m.fn.body));if(!b?.node)throw Error('Missing current binding '+m.id+':'+name);return b.node;}
const seeds=specs.map(([family,pid,id,name])=>{
 const c=corpora.get(pid),m=c.module(id),n=binding(c,m,name),methods=[],events=[],defaults=[];
 walk(n,v=>{
  if(v.type==='MethodDefinition')methods.push({name:key(v.key),...receipt(c,m,v)});
  if(v.type==='AssignmentExpression'&&v.left.type==='MemberExpression'&&v.left.object.type==='ThisExpression'&&/Function/.test(v.right.type))methods.push({name:key(v.left.property),...receipt(c,m,v)});
  if(v.type==='Property'&&/^on[A-Z]/.test(key(v.key)||''))events.push({event:key(v.key),...receipt(c,m,v)});
 });
 walk(m.fn,v=>{if(v.type==='AssignmentExpression'&&v.left.type==='MemberExpression'&&v.left.object.name===name&&key(v.left.property)==='defaultProps')defaults.push(receipt(c,m,v));});
 return{family,product_id:pid,module_id:id,symbol:name,node:n,...receipt(c,m,n),body_sha256:hash(receipt(c,m,n).source),methods,events,defaults};
});
// Scoped import/export and alias resolution for actual JSX callsites. HOC arguments
// are references, not branch reachability or runtime component-instance proof.
function resolve(c,n,s,seen=new Set(),trail=[]){
 if(!n||seen.has(n))return[];seen=new Set([...seen,n]);
 const matched=seeds.filter(t=>t.product_id===c.pid&&t.node===n);if(matched.length)return matched.map(t=>({...t,resolution_receipts:trail}));
 if(n.type==='Identifier'){const b=c.lookup(n.name,s);return b?.node?resolve(c,b.node,b.scope,seen,[...trail,{kind:'lexical_definition',identifier:n.name,...receipt(c,{file:b.scope.file},b.node)}]):[];}
 if(n.type==='MemberExpression'&&!n.computed&&n.object.type==='Identifier'){
  let b=c.lookup(n.object.name,s),v=b?.node;
  while(v?.type==='Identifier'){b=c.lookup(v.name,b.scope);v=b?.node;}
  if(v?.type==='CallExpression'&&v.callee.type==='Identifier'&&c.isRequire(v.callee.name,b.scope)&&Number.isInteger(v.arguments[0]?.value)){
   const m=c.module(v.arguments[0].value);if(!m)return[];
   return(m.exports.get(key(n.property))||[]).flatMap(e=>resolve(c,e.node,c.nodeScopes.get(e.node)||m.scope,seen,[...trail,{kind:'webpack_require_binding',module_id:m.id,...receipt(c,{file:b.scope.file},v)},{kind:e.kind,export:key(n.property),...receipt(c,m,e.evidence)}]));
  }
 }
 if(n.type==='CallExpression')return n.arguments.flatMap(a=>resolve(c,a,c.nodeScopes.get(a)||s,seen,[...trail,{kind:'call_argument_reference',...receipt(c,{file:s.file},n)}]));
 return[];
}
const callers=[];
for(const c of corpora.values())for(const m of c.modules.values()){
 const ancestors=[];
 function visit(n){if(!n?.type)return;
  const callee=n.callee?.type==='SequenceExpression'?n.callee.expressions.at(-1):n.callee;
  if(n.type==='CallExpression'&&['jsx','jsxs','createElement'].includes(key(callee?.property)||callee?.name)){
   const targets=resolve(c,n.arguments[0],c.nodeScopes.get(n.arguments[0]));
   const guards=ancestors.filter(a=>['ConditionalExpression','IfStatement','LogicalExpression'].includes(a.type)).map(a=>{
    const test=a.type==='LogicalExpression'?a.left:a.test;
    const branch=a.type==='LogicalExpression'?a.right:a.consequent;
    const contains=branch&&n.start>=branch.start&&n.end<=branch.end;
    return{kind:a.type,operator:a.operator,branch:contains?(a.type==='LogicalExpression'?'right':'consequent'):'alternate',...receipt(c,m,test)};
   });
   for(const t of targets)callers.push({family:t.family,product_id:c.pid,module_id:m.id,target:{module_id:t.module_id,symbol:t.symbol,body_sha256:t.body_sha256},resolution_receipts:t.resolution_receipts,guards,component_expression:receipt(c,m,n.arguments[0]),props:n.arguments[1]?receipt(c,m,n.arguments[1]):null,...receipt(c,m,n)});
  }
  ancestors.push(n);
  for(const v of Object.values(n))if(Array.isArray(v)){for(const ch of v)if(ch?.type)visit(ch);}else if(v?.type)visit(v);
  ancestors.pop();
 }
 visit(m.fn);
}
const helperSpecs=[[70,1124,'r','visibility wrapper'],[241,8837,'p','target coordinates'],[241,9163,'on','profile bar exclusions'],[241,9163,'nn','normal root and profile mount'],[241,9163,'xi','profile Redux action bridge'],[241,9163,'Kt','reset popup'],[241,3746,'ts','pairing dialog caller layout'],[241,3746,'Ps','pairing widget modal invocation'],[241,1867,'N','case-sensitive name duplicate validation'],[241,1867,'v','create name suffix choice'],[241,1867,'m','duplicate name suffix choice'],[241,2478,'o','localStorage wrapper; updateUserData is empty']];
const helpers=helperSpecs.map(([pid,id,name,meaning])=>{const c=corpora.get(pid),m=c.module(id);return{product_id:pid,module_id:id,symbol:name,meaning,...receipt(c,m,binding(c,m,name))};});
for(const [pid,id,names,meaning]of [[70,5107,['closeMapping','clear','saveMapping','setSaveMapRef','saveChanges','displaySaveAlert','dismissSave','dontSave','submitDialog','activeConfirmDialog','activeWarningDialog','nextAction','renderConfirmDialog'],'mapping draft Save / Discard / Cancel UI; actions do not prove DLL writes'],[241,3112,['a','r','c','l','_','E','d','p','h'],'profile action creators; Redux dispatch only']]){
 const c=corpora.get(pid),m=c.module(id);
 if(id===3112)for(const name of names)helpers.push({product_id:pid,module_id:id,symbol:name,meaning,...receipt(c,m,binding(c,m,name))});
 else walk(m.fn,n=>{if(n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&n.left.object.type==='ThisExpression'&&names.includes(key(n.left.property)))helpers.push({product_id:pid,module_id:id,symbol:'this.'+key(n.left.property),meaning,...receipt(c,m,n)});});
}
// Candidate index over every registered product's existing page reference graph.
// A method-name signature is NOT proof that different source bodies behave alike.
const signatures={
 slider:['roundValue','onChange','cancelMouseUp','enableSliderRange'],
 'legacy-slider':['onChange','getPercent','getPosition','slider-container'],
 number:['parseInput','handleBlur','volumeUp','volumeDown'],
 dropdown:['allowOneItem','toggleDropdown','s3-dropdown','selectOption'],
 'keymap-dropdown':['addKeymap','toggleDropdown','s3-dropdown','selectOption'],
 'rename-input':['handleChange','handleBlur','inputDom','sendToParent'],
 'delete-popup':['confirmDel','delAlert','mouseClick'],
 'action-popup':['calculatePosition','menuClick','toggleDropdown','disabledItems'],
 tab:['aria-selected','changeView','onKeyDown','role:"tab"'],
 nav:['renderNavs','renderDropdownNavs','cacheNavWidth'],
 profile:['createProfileDataSet','setProfileName','confirmDel'],
 'widget-tooltip':['onHover','onLeave','tipDiv','widget-container'],
 'positioned-tooltip':['showTooltip','hideTooltip','handleReposition'],
 'macro-draft':['saveMapping','getMappingData','changeTimes'],
};
const graphPath='docs/re/all-product-page-chains-current.json.gz',graphBytes=fs.readFileSync(path.join(root,graphPath));
const graph=JSON.parse(zlib.gunzipSync(graphBytes));
const candidates=[],counts={};
for(const product of graph.products){
 const files=new Map(),instances=new Map();
 for(const page of product.pages)for(let i=0;i<page.components.length;i++){
  const r=page.components[i];
  if(!files.has(r.path)){const text=read(r.path);if(hash(text)!==r.sha256)throw Error('Stale graph source '+r.path);files.set(r.path,text);}
  const text=files.get(r.path).slice(r.offset,r.end);
  // Bound reference components can contain other controls: only actual class
  // roots are indexed here, excluding enclosing HOCs and page functions.
  if(r.node_type!=='ClassDeclaration'&&r.node_type!=='ClassExpression')continue;
  for(const[family,required]of Object.entries(signatures))if(required.every(s=>text.includes(s))){
   if(family==='legacy-slider'&&text.includes('cancelMouseUp'))continue;
   const identity=family+':'+r.path+':'+r.offset+':'+r.end;
   let candidate=instances.get(identity);
   if(!candidate){candidate={family,product_id:product.product_id,symbol:r.symbol,path:r.path,sha256:r.sha256,offset:r.offset,end:r.end,body_sha256:hash(text),exact_seed_bodies:seeds.filter(s=>s.family===family&&s.body_sha256===hash(text)).map(s=>({product_id:s.product_id,module_id:s.module_id,symbol:s.symbol})),pages:[],incoming_references:[]};instances.set(identity,candidate);candidates.push(candidate);}
   if(!candidate.pages.includes(page.page_id))candidate.pages.push(page.page_id);
   for(const edge of page.edges.filter(e=>e.to===i&&e.from!==null)){
    const from=page.components[edge.from];candidate.incoming_references.push({page_id:page.page_id,page_key:page.page_key,kind:'bounded lexical/JSX/import reference; not branch execution',path:from.path,sha256:from.sha256,offset:from.offset,end:from.end,symbol:from.symbol});
   }
  }
 }
}
for(const family of Object.keys(signatures)){
 const rows=candidates.filter(c=>c.family===family);
 counts[family]={product_count:new Set(rows.map(r=>r.product_id)).size,class_instances:rows.length,raw_body_variants:new Set(rows.map(r=>r.body_sha256)).size,exact_seed_instances:rows.filter(r=>r.exact_seed_bodies.length).length,semantic_review_scope:'named seed bodies only; other bodies remain variant candidates'};
}
function blockEnd(text,offset){let level=0,quote='';for(let i=text.indexOf('{',offset);i<text.length;i++){const ch=text[i];if(ch==='\\'){i++;continue;}if(quote){if(ch===quote)quote='';continue;}if(ch==='"'||ch==="'"){quote=ch;continue;}if(ch==='{')level++;if(ch==='}'&&!--level)return i+1;}throw Error('Unclosed CSS block');}
const css=[],fonts=[];
for(const c of corpora.values())for(const relative of [...new Set(Object.values(c.manifest.files))].filter(p=>p.endsWith('.css'))){
 const file=`.ref/devices/${c.pid}/`+relative.replace(/^\.\//,''),text=read(file);
 const rules=parseCSS(text).filter(r=>/slider|stepper|s3-dropdown|s3-options|dropdown-area|drop-tips|nav-tabs|navs-wrapper|profile-bar|profile-act|rename-rect|profile-del|del-alert|tooltip-razer|\.widget\b|\.widget-container|\.help\b|\.tip\b|modal_|^html\b|^body\b|^input\b|^\*\b/.test(r.selector));
 css.push({product_id:c.pid,path:file,sha256:hash(text),rules:rules.map(r=>({...r,end:blockEnd(text,r.offset),source:text.slice(r.offset,blockEnd(text,r.offset)),status_terms:[...new Set(r.selector.match(/:[a-z-]+|\.(?:disabled|on|active|show|inactive|expand|selected|no-tip|no-tag|placeholder|raw-text|custom-tip)\b/g)||[])]}))});
 for(const match of text.matchAll(/@font-face\s*\{/g)){const end=blockEnd(text,match.index);fonts.push({product_id:c.pid,path:file,sha256:hash(text),offset:match.index,end,source:text.slice(match.index,end),properties:declarations(text.slice(text.indexOf('{',match.index)+1,end-1))});}
}
const result={schema_version:1,method:'Current manifest-scoped static Acorn parsing, source range/hash receipts, scoped JSX import resolution, CSS declarations, and all-product page-reference candidate index. No application, DLL, vendor JS, build or tests executed.',offset_unit:'UTF-16 code units; end exclusive; SHA-256 of UTF-8 source bytes',generator_sha256:hash(fs.readFileSync(__filename)),graph:{path:graphPath,sha256:hash(graphBytes),products:graph.products.length},summary:{semantic_seeds:seeds.length,actual_jsx_callsite_receipts:callers.length,helper_receipts:helpers.length,graph_control_candidates:candidates.length,css_rule_receipts:css.reduce((n,c)=>n+c.rules.length,0),font_face_receipts:fonts.length},manifests:[...corpora.values()].map(c=>{const p=`.ref/devices/${c.pid}/asset-manifest.json`;return{product_id:c.pid,path:p,sha256:hash(read(p))};}),seeds:seeds.map(({node,...s})=>s),callers,helpers,all_product_family_counts:counts,all_product_class_candidates:candidates,css,fonts,limits:['Different minified class bodies are variants, not audited semantic equivalents. Equal method-name signatures are only candidates.','Page-reference graph incoming edges include lexical and HOC references; execution and per-product root profile bars are not inferred.','Semantic evidence is limited to the named seed bodies and their actual JSX callers. No claim of all control variants complete.','CSS declarations preserve selector/order/conditions; no browser cascade, physical DPI measurement or screenshot equivalence claimed.','UI dispatches and local component drafts are not successful device writes or proof of DLL persistence.']};
const target='docs/re/shared-ui-controls-current.json',serialized=JSON.stringify(result,null,2)+'\n';
if(process.argv.includes('--check')){if(read(target)!==serialized)throw Error('Stale '+target);}else fs.writeFileSync(path.join(root,target),serialized);
console.log(JSON.stringify(result.summary));console.log(JSON.stringify(counts));
