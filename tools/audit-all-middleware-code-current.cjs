// Static syntax and webpack-module index. Vendor JavaScript is read as data;
// it is never required, imported, evaluated, or executed.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const zlib = require('zlib');
const {Worker, isMainThread, parentPort} = require('worker_threads');
const acorn = require('acorn');
const ROOT = path.resolve(__dirname, '..');
const SIGNATURE = `middleware-static-index-v2/acorn-${acorn.version}`;
const CACHE = path.join(ROOT, '.work/middleware-code-current-cache.json');
const DETAILS = 'docs/re/middleware-code-current-evidence.json.gz';
const SUMMARY = 'docs/re/middleware-code-current-summary.json';
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const key = node => node?.name ?? node?.value;
function traverse(node, visit) {
  const stack = [node];
  while (stack.length) {
    const current = stack.pop();
    if (!current || typeof current.type !== 'string') continue;
    visit(current);
    for (const value of Object.values(current)) {
      if (Array.isArray(value)) {
        for (let i=value.length-1;i>=0;i--) if (value[i]?.type) stack.push(value[i]);
      } else if (value?.type) stack.push(value);
    }
  }
}
function analyze(file, expectedHash) {
  const bytes = fs.readFileSync(path.join(ROOT, file));
  if (hash(bytes) !== expectedHash) throw Error('Source changed while indexing: ' + file);
  const source = bytes.toString('utf8');
  let ast, sourceType='script', firstError;
  try { ast=acorn.parse(source,{ecmaVersion:'latest',sourceType}); }
  catch (error) {
    firstError={message:error.message,pos:error.pos,line:error.loc?.line,column:error.loc?.column};
    sourceType='module';
    try { ast=acorn.parse(source,{ecmaVersion:'latest',sourceType}); }
    catch (moduleError) { return {parse_status:'error',script_error:firstError,module_error:{message:moduleError.message,pos:moduleError.pos,line:moduleError.loc?.line,column:moduleError.loc?.column}}; }
  }
  const modules=[], imports=[], lazyCandidates=[], actionCandidates=[];
  let functions=0, classes=0, nodes=0, switchCases=0;
  traverse(ast,node=>{
    nodes++;
    if (['FunctionDeclaration','FunctionExpression','ArrowFunctionExpression'].includes(node.type)) functions++;
    if (['ClassDeclaration','ClassExpression'].includes(node.type)) classes++;
    if (node.type==='SwitchCase') switchCases++;
    if (node.type==='ImportDeclaration' || node.type==='ImportExpression') imports.push({kind:node.type,module:node.source?.value??null,start:node.start,end:node.end});
    if (node.type==='CallExpression' && node.callee.name==='require') imports.push({kind:'require',module:node.arguments[0]?.value??null,start:node.start,end:node.end});
    if ((node.type==='Property' || node.type==='PropertyDefinition') && key(node.key)==='action' && typeof node.value?.value==='string') actionCandidates.push({kind:'action-field',value:node.value.value,start:node.start,end:node.end});
    if (node.type==='SwitchStatement' && node.discriminant.type==='MemberExpression' && key(node.discriminant.property)==='action') {
      for (const branch of node.cases) if(typeof branch.test?.value==='string') actionCandidates.push({kind:'action-switch',value:branch.test.value,start:branch.start,end:branch.end});
    }
    if (node.type!=='ObjectExpression' || !node.properties.length || !node.properties.every(p=>p.type==='Property' && Number.isInteger(key(p.key)) && ['FunctionExpression','ArrowFunctionExpression'].includes(p.value.type))) return;
    for(const property of node.properties) {
      const fn=property.value, requireParam=fn.params[2]?.name;
      const item={id:key(property.key),start:fn.start,end:fn.end,functions:0,webpack_require_candidates:[],lazy_candidates:[],action_candidates:0};
      traverse(fn,child=>{
        if(['FunctionDeclaration','FunctionExpression','ArrowFunctionExpression'].includes(child.type))item.functions++;
        if((child.type==='Property'||child.type==='PropertyDefinition')&&key(child.key)==='action'&&typeof child.value?.value==='string')item.action_candidates++;
        if(child.type==='CallExpression'&&requireParam&&child.callee.name===requireParam&&typeof child.arguments[0]?.value==='number')item.webpack_require_candidates.push(child.arguments[0].value);
        if(child.type==='CallExpression'&&requireParam&&child.callee.type==='MemberExpression'&&child.callee.object.name===requireParam&&key(child.callee.property)==='e')item.lazy_candidates.push({chunk:child.arguments[0]?.value??null,start:child.start,end:child.end});
      });
      item.webpack_require_candidates=[...new Set(item.webpack_require_candidates)].sort((a,b)=>a-b);
      modules.push(item);
      lazyCandidates.push(...item.lazy_candidates);
    }
  });
  return {parse_status:'ok',source_type:sourceType,ast_nodes:nodes,function_count:functions,class_count:classes,switch_case_count:switchCases,webpack_module_count:modules.length,static_import_count:imports.length,lazy_candidate_count:lazyCandidates.length,action_candidate_count:actionCandidates.length,modules,imports,action_candidates:actionCandidates};
}
if (!isMainThread) {
  parentPort.on('message',job=>{
    try { parentPort.postMessage({sha256:job.sha256,analysis:analyze(job.path,job.sha256)}); }
    catch(error) { parentPort.postMessage({sha256:job.sha256,analysis:{parse_status:'tool_error',error:error.message}}); }
  });
} else main().catch(error=>{console.error(error.stack);process.exitCode=1;});
async function main() {
  const acquisition=JSON.parse(fs.readFileSync(path.join(ROOT,'docs/re/middleware-source-acquisition-current.json'),'utf8'));
  if(!acquisition.all_original_missing_acquired || acquisition.currently_missing_manifest_js_paths)throw Error('Complete pinned middleware acquisition required');
  const files=[], representatives=new Map(), manifests=[];
  for(const manifestReceipt of acquisition.manifests) {
    const bytes=fs.readFileSync(path.join(ROOT,manifestReceipt.path));
    if(hash(bytes)!==manifestReceipt.sha256)throw Error('Pinned manifest changed: '+manifestReceipt.path);
    manifests.push(manifestReceipt);
    const manifest=JSON.parse(bytes), directory=path.posix.dirname(manifestReceipt.path);
    for(const file of [...new Set(Object.values(manifest))].filter(v=>typeof v==='string'&&v.endsWith('.js')).sort()) {
      if(!/^[A-Za-z0-9_.-]+\.js$/.test(file))throw Error('Unsafe declared source name');
      const relative=`${directory}/${file}`, source=fs.readFileSync(path.join(ROOT,relative)), sha256=hash(source);
      files.push({product_id:manifestReceipt.product_id,path:relative,bytes:source.length,sha256});
      if(!representatives.has(sha256))representatives.set(sha256,{path:relative,sha256});
    }
  }
  const prior=fs.existsSync(CACHE)?JSON.parse(fs.readFileSync(CACHE,'utf8')):{};
  const analyses=prior.signature===SIGNATURE?prior.analyses:{};
  const jobs=[...representatives.values()].filter(job=>!analyses[job.sha256]);
  console.log(`Pinned middleware JS=${files.length}; unique SHA=${representatives.size}; cache=${representatives.size-jobs.length}; parse=${jobs.length}`);
  let cursor=0,completed=0;
  if(jobs.length)await Promise.all(Array.from({length:Math.min(3,jobs.length)},()=>new Promise((resolve,reject)=>{
    const worker=new Worker(__filename);
    worker.on('error',reject);
    worker.on('message',result=>{
      analyses[result.sha256]=result.analysis;
      completed++;
      if(completed%500===0||completed===jobs.length)console.log(`Static parsed ${completed}/${jobs.length}`);
      if(cursor<jobs.length)worker.postMessage(jobs[cursor++]);
      else worker.terminate().then(resolve,reject);
    });
    worker.postMessage(jobs[cursor++]);
  })));
  const selected=Object.fromEntries([...representatives.keys()].map(digest=>[digest,analyses[digest]]));
  for(const result of Object.values(selected))if(result.parse_status==='ok')result.webpack_require_candidate_count=result.modules.reduce((sum,module)=>sum+module.webpack_require_candidates.length,0);
  if(!process.argv.includes('--check')) {
    fs.mkdirSync(path.dirname(CACHE),{recursive:true});
    fs.writeFileSync(CACHE,JSON.stringify({signature:SIGNATURE,analyses:selected}));
  }
  const byStatus={}, failures=[];
  const totals={function_nodes_across_declared_paths:0,webpack_modules_across_declared_paths:0,static_imports_across_declared_paths:0,webpack_require_candidate_edges_across_declared_paths:0,lazy_candidates_across_declared_paths:0,action_candidates_across_declared_paths:0};
  for(const file of files) {
    const result=selected[file.sha256];
    file.parse_status=result.parse_status;
    byStatus[result.parse_status]=(byStatus[result.parse_status]??0)+1;
    if(result.parse_status!=='ok')failures.push({...file,error:result});
    else {
      totals.function_nodes_across_declared_paths+=result.function_count;
      totals.webpack_modules_across_declared_paths+=result.webpack_module_count;
      totals.static_imports_across_declared_paths+=result.static_import_count;
      totals.webpack_require_candidate_edges_across_declared_paths+=result.webpack_require_candidate_count;
      totals.lazy_candidates_across_declared_paths+=result.lazy_candidate_count;
      totals.action_candidates_across_declared_paths+=result.action_candidate_count;
    }
  }
  const evidence={schema:1,parser:SIGNATURE,method:'Acorn AST only; each unique SHA parsed once, no vendor evaluation',manifests,files,analyses:selected,boundaries:['Function/module syntax counts do not establish complete behavior','Webpack require and lazy candidates use the module require parameter spelling; lexical-shadow ambiguities remain','Action candidates are syntactic fields/switches, not proof of reachability, actual IPC routes, ABI or successful device operations','AST locations are UTF-16 source offsets, not byte offsets','Source maps and original author build trees are not reconstructed']};
  const compressed=zlib.gzipSync(Buffer.from(JSON.stringify(evidence)),{level:9});
  const summary={schema:1,parser:SIGNATURE,method:evidence.method,manifest_count:manifests.length,declared_js_paths:files.length,unique_sha256:representatives.size,source_bytes_across_declared_paths:files.reduce((sum,file)=>sum+file.bytes,0),parse_status_by_path:byStatus,unique_parse_status:Object.values(selected).reduce((counts,a)=>(counts[a.parse_status]=(counts[a.parse_status]??0)+1,counts),{}),totals,failures,details:{path:DETAILS,sha256:hash(compressed),compressed_bytes:compressed.length},boundaries:evidence.boundaries};
  const rendered=JSON.stringify(summary,null,2)+'\n';
  if(process.argv.includes('--check')) {
    if(fs.readFileSync(path.join(ROOT,SUMMARY),'utf8').replace(/\r\n/g,'\n')!==rendered || hash(fs.readFileSync(path.join(ROOT,DETAILS)))!==hash(compressed))throw Error('Middleware syntax/module evidence stale');
    console.log('Current middleware syntax/module evidence verified');
  } else {fs.writeFileSync(path.join(ROOT,DETAILS),compressed);fs.writeFileSync(path.join(ROOT,SUMMARY),rendered);console.log(`Wrote ${SUMMARY}; statuses=${JSON.stringify(byStatus)}; gzip bytes=${compressed.length}`);}
  if(failures.length)process.exitCode=2;
}
