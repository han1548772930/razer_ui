// Parse current application files as data; never import/evaluate vendor JS.
const fs=require('fs'),path=require('path'),crypto=require('crypto'),acorn=require('acorn'),zlib=require('zlib');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),read=p=>fs.readFileSync(path.join(root,p),'utf8');
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
const key=n=>n?.name??n?.value;
function walk(n,visit){if(!n?.type||visit(n)===false)return;for(const v of Object.values(n)){if(Array.isArray(v)){for(const c of v)if(c?.type)walk(c,visit);}else if(v?.type)walk(v,visit);}}
const catalog=JSON.parse(read('docs/re/application-catalog.json')),applications=[],sources=new Map();
function js(file){
 if(sources.has(file))return sources.get(file);
 const text=read(file),state=[],messages=[],modules=[],imports=[],startup=[],rootMounts=[];let ast,parseError;
 const webpackBundle=/webpackChunk|webpackJsonp/.test(text);
 try{ast=acorn.parse(text,{ecmaVersion:'latest',sourceType:'module'});}catch(error){parseError=String(error);}
 const receipt=n=>({offset:n.start,end:n.end,expression:text.slice(n.start,n.end).slice(0,300)});
 function moduleReceipt(moduleId,fn){
   const dependencies=[],exports=[],components=[],lazy=[];const requireName=fn.params[2]?.name,exportName=fn.params[1]?.name,moduleName=fn.params[0]?.name;
   walk(fn.body,c=>{
    if(requireName&&c.type==='CallExpression'&&c.callee.name===requireName&&Number.isInteger(c.arguments[0]?.value))dependencies.push({module_id:c.arguments[0].value,offset:c.start});
    if(requireName&&c.type==='CallExpression'&&c.callee.type==='MemberExpression'&&c.callee.object.name===requireName&&key(c.callee.property)==='d'){
     if(c.arguments[1]?.type==='ObjectExpression')for(const p of c.arguments[1].properties)exports.push({name:key(p.key),...receipt(p)});
     else if(c.arguments[1]?.type==='Literal')exports.push({name:c.arguments[1].value,...receipt(c)});
    }
    if(c.type==='AssignmentExpression'&&c.left.type==='MemberExpression'&&(exportName&&c.left.object.name===exportName||moduleName&&c.left.object.name===moduleName&&key(c.left.property)==='exports'))exports.push({name:c.left.object.name===moduleName?'module.exports':key(c.left.property),...receipt(c)});
    if(requireName&&c.type==='CallExpression'&&key(c.callee.property)==='bind'&&c.callee.object?.name===requireName&&Number.isInteger(c.arguments[1]?.value))lazy.push({module_id:c.arguments[1].value,offset:c.start});
    if(c.type==='CallExpression'&&c.callee.type==='SequenceExpression'&&['jsx','jsxs'].includes(key(c.callee.expressions.at(-1)?.property)))components.push({component:text.slice(c.arguments[0].start,c.arguments[0].end),offset:c.start});
    if(c.type==='CallExpression'&&key(c.callee.property)==='createElement')components.push({component:text.slice(c.arguments[0].start,c.arguments[0].end),offset:c.start,kind:'react_createElement'});
   });
   modules.push({module_id:moduleId,offset:fn.start,end:fn.end,require_name:requireName,exports,dependencies,lazy,jsx_references:components});
 }
 if(ast)walk(ast,n=>{
  if(n.type==='ImportDeclaration'||n.type==='ExportNamedDeclaration'&&n.source)imports.push({...receipt(n),target:n.source.value,kind:'esm_static'});
  if(n.type==='ImportExpression')imports.push({...receipt(n),target:n.source.type==='Literal'?n.source.value:null,kind:'esm_dynamic'});
  if(webpackBundle&&n.type==='Property'&&Number.isInteger(key(n.key))&&/FunctionExpression$/.test(n.value?.type)&&n.value.params.length<=3)moduleReceipt(key(n.key),n.value);
  if(webpackBundle&&n.type==='ArrayExpression'&&n.elements.some(e=>/FunctionExpression$/.test(e?.type))&&n.elements.every(e=>!e||/FunctionExpression$/.test(e.type))){n.elements.forEach((fn,id)=>{if(fn)moduleReceipt(id,fn);});}
  if(n.type==='CallExpression'){
   const method=n.callee.type==='MemberExpression'?key(n.callee.property):n.callee.name;
   if(['useState','useReducer','useSelector','setState','dispatch','subscribe','unsubscribe'].includes(method))state.push({method,...receipt(n)});
   if(typeof method==='string'&&/^(get|set|read|write|query|send|save|load|apply|request|register|open|close|start|stop|connect|disconnect|invoke|emit|publish|execute)[A-Z_]/.test(method))messages.push({method,...receipt(n)});
   if(['createRoot','hydrateRoot','render'].includes(method)&&text.slice(Math.max(0,n.start-100),n.end).includes('getElementById'))rootMounts.push({method,...receipt(n)});
   if(n.callee.type==='MemberExpression'&&['O','s'].includes(key(n.callee.property)))startup.push(receipt(n));
  }
 });
 const value={path:file,sha256:hash(text),bytes:Buffer.byteLength(text),parse_status:parseError?'failed':'parsed',parse_error:parseError,bundle_system_hint:webpackBundle?'webpack_chunk_marker':imports.length?'esm_imports':'no_supported_module_registry_marker',imports,modules,webpack_runtime_candidates:startup,root_mount_candidates:rootMounts,state_observations:state,message_candidates:messages};
 sources.set(file,value);return value;
}
for(const app of catalog.applications){
 const dir=`.ref/applications${app.route.replace(/\/$/,'')}`;
 const htmlPath=`${dir}/index.html`,manifestPath=`${dir}/asset-manifest.json`;
 const html=fs.existsSync(path.join(root,htmlPath))?read(htmlPath):null;
 const manifest=fs.existsSync(path.join(root,manifestPath))?JSON.parse(read(manifestPath)):null;
 const files=app.files.filter(f=>f.result==='ok'),entries=manifest?.entrypoints??[];
 const scripts=html?[...html.matchAll(/<script\b[^>]*\bsrc=["']([^"']+)["']/gi)].map(m=>({src:m[1],offset:m.index})):[];
 const styles=html?[...html.matchAll(/<link\b[^>]*\bhref=["']([^"']+)["'][^>]*>/gi)].map(m=>({href:m[1],offset:m.index})):[];
 const jsPaths=[...new Set(files.filter(f=>/\.js$/.test(f.path)).map(f=>f.path))],css=[];
 for(const f of files.filter(f=>/\.css$/.test(f.path))){const text=read(f.path),rules=parseCSS(text);css.push({path:f.path,sha256:hash(text),rules:rules.length,rule_receipts:rules,conditional_rules:rules.filter(r=>r.conditions.length).length,font_rules:[...text.matchAll(/@font-face\s*\{/g)].map(m=>({offset:m.index})),asset_urls:[...new Set([...text.matchAll(/url\(([^)]+)\)/g)].map(m=>m[1]))]});}
 for(const file of jsPaths)js(file);
 applications.push({route:app.route,catalog_evidence:app.evidence,html:html?{path:htmlPath,sha256:hash(html),scripts,links:styles}:null,manifest:manifest?{path:manifestPath,sha256:hash(read(manifestPath)),entrypoints:entries}:null,js_files:jsPaths,css,scope_status:html?'partial_static_module_inventory':'current_endpoint_no_page_source',behavioral_completion_claimed:false,runtime_validation:'not_run'});
 console.log(`${app.route}: ${jsPaths.length} JS, ${css.length} CSS`);
}
const all=[...sources.values()],summary={application_endpoints:applications.length,html_entries:applications.filter(a=>a.html).length,unique_js_files:all.length,parsed_js_files:all.filter(a=>a.parse_status==='parsed').length,parse_failures:all.filter(a=>a.parse_status==='failed').length,webpack_modules:all.reduce((n,a)=>n+a.modules.length,0),css_file_references:applications.reduce((n,a)=>n+a.css.length,0),semantically_completed_applications_claimed:0};
const payload={schema_version:1,scope:'Current application HTML/manifest entrypoints, static JS modules/imports/lazy references/state/message candidates and CSS/resource receipts',scanner_sha256:hash(fs.readFileSync(__filename)),catalog_sha256:hash(read('docs/re/application-catalog.json')),summary,limitations:['The catalog reflects local evidence fetched on 2026-10-02, not a new network freshness check.','Webpack import/module references and JSX references do not prove route reachability or behavioral completion.','Method-prefix candidates require review; setters may update local state and are not automatically DLL writes.','CSS rule/url/font counts do not reconstruct browser cascade, dimensions or animation behavior.','No application, vendor JavaScript, DLL, build or tests executed.'],applications,sources:all};
const full=Buffer.from(JSON.stringify(payload)+'\n'),compressed=zlib.gzipSync(full,{level:9});
fs.writeFileSync(path.join(root,'docs/re/all-application-chains-current.json.gz'),compressed);
const index={schema_version:1,scope:payload.scope,scanner_sha256:payload.scanner_sha256,catalog_sha256:payload.catalog_sha256,summary,limitations:payload.limitations,data_file:'all-application-chains-current.json.gz',data_sha256:hash(compressed),uncompressed_sha256:hash(full),uncompressed_bytes:full.length,applications:applications.map(app=>({...app,css:app.css.map(({rule_receipts,...receipt})=>receipt)}))};
fs.writeFileSync(path.join(root,'docs/re/all-application-chains-current.json'),JSON.stringify(index,null,2)+'\n');console.log(JSON.stringify(summary));
