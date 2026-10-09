// Parse preserved current component ranges as data. Never execute reference JS.
const fs=require('fs'),path=require('path'),crypto=require('crypto'),zlib=require('zlib'),acorn=require('acorn');
const {once}=require('events');
const {walk,key}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..'),sha=b=>crypto.createHash('sha256').update(b).digest('hex');
const assert=(ok,message)=>{if(!ok)throw Error(message);};
const read=file=>fs.readFileSync(path.join(root,file));
const graphPath='docs/re/all-product-page-chains-current.json.gz',graphHash=sha(read(graphPath));
const graph=JSON.parse(zlib.gunzipSync(read(graphPath))),toolHash=sha(fs.readFileSync(__filename));
const textCache=new Map(),hashes=new Map();
function source(file,expected) {
  if(!textCache.has(file)){const bytes=read(file);hashes.set(file,sha(bytes));textCache.set(file,bytes.toString('utf8'));}
  assert(!expected||hashes.get(file)===expected,'Changed source '+file);
  const text=textCache.get(file);textCache.delete(file);textCache.set(file,text);
  while(textCache.size>10)textCache.delete(textCache.keys().next().value);
  return text;
}
function propertyName(n){return n?.computed?(n.key.type==='Literal'?String(n.key.value):null):String(key(n.key));}
function callee(n){const c=n?.type==='SequenceExpression'?n.expressions.at(-1):n;return c?.type==='MemberExpression'?key(c.property):c?.name;}
function uiCall(n){return n?.type==='CallExpression'&&['jsx','jsxs','createElement'].includes(callee(n.callee));}
function names(n){if(!n)return [];if(n.type==='Identifier')return [n.name];if(n.type==='RestElement')return names(n.argument);if(n.type==='AssignmentPattern')return names(n.left);if(n.type==='ArrayPattern')return n.elements.flatMap(names);if(n.type==='ObjectPattern')return n.properties.flatMap(p=>names(p.type==='RestElement'?p.argument:p.value));return [];}
function extract(component) {
  const text=source(component.path,component.sha256),raw=text.slice(component.offset,component.end),bounded='('+raw+')';
  const ast=acorn.parseExpressionAt(bounded,0,{ecmaVersion:'latest'});
  assert(ast.start===1&&ast.end===bounded.length-1,'Component AST boundary '+component.path+':'+component.offset);
  const base=component.offset-1,span=n=>n?.type?{offset:n.start+base,end:n.end+base,node_type:n.type}:null;
  const code=n=>bounded.slice(n.start,n.end),id=`${component.path}@${component.offset}:${component.end}`;
  const nodes=[],parents=new WeakMap(),scopeOf=new WeakMap(),scopes=[],assignments=[];
  function visit(n,parent,scope) {
    if(!n?.type)return;
    parents.set(n,parent);
    if(n.type==='FunctionDeclaration'||n.type==='ClassDeclaration')scope?.defs.set(n.id.name,{value:n,declaration:n,kind:'declaration'});
    if(n.type==='Program'||/Function/.test(n.type)||['BlockStatement','ForStatement','ForInStatement','ForOfStatement','SwitchStatement','CatchClause'].includes(n.type)) {
      scope={parent:scope,defs:new Map(),node:n,function:/Function/.test(n.type)};scopes.push(scope);
      if(scope.function)for(const p of n.params??[])for(const name of names(p))scope.defs.set(name,{value:null,declaration:p,kind:'parameter'});
      if(n.type==='CatchClause')for(const name of names(n.param))scope.defs.set(name,{value:null,declaration:n.param,kind:'catch_parameter'});
    }
    if(n.type==='VariableDeclarator') {
      let target=scope;if(parent.kind==='var')while(target?.parent&&!target.function)target=target.parent;
      for(const name of names(n.id))target?.defs.set(name,{value:n.id.type==='Identifier'?n.init:null,declaration:n,kind:n.id.type==='Identifier'?'variable':'destructured_variable'});
    }
    scopeOf.set(n,scope);nodes.push(n);
    if(n.type==='AssignmentExpression')assignments.push(n);
    for(const v of Object.values(n))if(Array.isArray(v))v.forEach(c=>{if(c?.type)visit(c,n,scope);});else if(v?.type)visit(v,n,scope);
  }
  visit(ast,null,{parent:null,defs:new Map(),node:null,function:true});
  const lookup=(name,n)=>{let s=scopeOf.get(n);while(s){if(s.defs.has(name))return s.defs.get(name);s=s.parent;}return null;};
  const elements=nodes.filter(uiCall),elementIDs=new Map(elements.map(n=>[n,`jsx@${n.start+base}`]));
  const ownReturns=fn=>nodes.filter(n=>n.type==='ReturnStatement'&&n.argument&&n.start>=fn.start&&n.end<=fn.end&&(()=>{let p=parents.get(n);while(p&&p!==fn&&!/Function/.test(p.type))p=parents.get(p);return p===fn;})());
  function literal(n){if(n?.type==='Literal')return {static_value:n.value};if(n?.type==='UnaryExpression'&&n.operator==='!'&&n.argument.type==='Literal')return {static_value:!n.argument.value};return {};}
  function binding(n) {
    if(n?.type==='Identifier'){const b=lookup(n.name,n);return b?{status:b.value?'component_local_declaration':'parameter_or_destructuring',name:n.name,declaration:span(b.declaration),value:span(b.value)}:{status:'outside_component_range',name:n.name};}
    if(n?.type==='MemberExpression'&&n.object.type==='ThisExpression'&&!n.computed){const candidates=assignments.filter(a=>a.left.type==='MemberExpression'&&a.left.object.type==='ThisExpression'&&!a.left.computed&&key(a.left.property)===key(n.property));return {status:candidates.length?'this_member_assignment_candidates':'this_member_outside_range',name:key(n.property),assignments:candidates.map(a=>({assignment:span(a),value:span(a.right)}))};}
    return {status:/Function/.test(n?.type??'')?'inline_function':'expression',value:span(n)};
  }
  function expression(n) {
    if(!n)return null;
    const result={...span(n),...literal(n)};
    if(elementIDs.has(n))return {...result,kind:'element_reference',element_id:elementIDs.get(n)};
    if(n.type==='ArrayExpression')return {...result,kind:'ordered_array',items:n.elements.map(expression)};
    if(n.type==='ConditionalExpression')return {...result,kind:'conditional',test:span(n.test),consequent:expression(n.consequent),alternate:expression(n.alternate)};
    if(n.type==='LogicalExpression')return {...result,kind:'logical',operator:n.operator,left:expression(n.left),right:expression(n.right)};
    if(n.type==='SequenceExpression')return {...result,kind:'sequence',expressions:n.expressions.map(expression)};
    if(n.type==='CallExpression') {
      const label=callee(n.callee),callbacks=n.arguments.filter(a=>/Function/.test(a.type));
      return {...result,kind:['map','filter','flatMap','reduce'].includes(label)?'iteration_call':'call',callee:span(n.callee),method:label??null,
        arguments:n.arguments.map(span),callbacks:callbacks.map(fn=>({function:span(fn),returns:ownReturns(fn).map(child=>expression(child.argument)),expression_body:fn.body.type==='BlockStatement'?null:expression(fn.body)}))};
    }
    if(n.type==='Identifier'||n.type==='MemberExpression')return {...result,kind:'reference',binding:binding(n)};
    return {...result,kind:'expression'};
  }
  function properties(n) {
    if(n?.type!=='ObjectExpression')return [];
    return n.properties.map((p,order)=>p.type==='SpreadElement'?{order,kind:'spread',...span(p),argument:span(p.argument),binding:binding(p.argument)}:
      {order,kind:p.kind??'init',...span(p),name:propertyName(p),computed:p.computed,shorthand:p.shorthand,key:span(p.key),value:span(p.value),...literal(p.value),binding:binding(p.value)});
  }
  const ui=elements.map(n=>{
    const type=n.arguments[0],props=n.arguments[1],list=properties(props),helperObjects=[];
    if(props?.type==='CallExpression')for(const [order,arg]of props.arguments.entries())if(arg.type==='ObjectExpression')helperObjects.push({argument_order:order,...span(arg),properties:properties(arg)});
    const directPropNodes=props?.type==='ObjectExpression'?props.properties.filter(p=>p.type==='Property'):[];
    const helperPropNodes=props?.type==='CallExpression'?props.arguments.flatMap(a=>a.type==='ObjectExpression'?a.properties.filter(p=>p.type==='Property'):[]):[];
    const propNodes=[...directPropNodes,...helperPropNodes];
    const child=directPropNodes.find(p=>!p.computed&&key(p.key)==='children');
    const childTree=callee(n.callee)==='createElement'?{kind:'ordered_arguments',items:n.arguments.slice(2).map(expression)}:child?expression(child.value):null;
    let ancestor=parents.get(n);while(ancestor&&!elementIDs.has(ancestor))ancestor=parents.get(ancestor);
    return {id:elementIDs.get(n),...span(n),syntax:callee(n.callee),callee:span(n.callee),component:{...span(type),...literal(type),kind:type?.type==='Literal'&&typeof type.value==='string'?'intrinsic_string':'component_expression',binding:binding(type)},
      lexical_parent_element:ancestor?elementIDs.get(ancestor):null,
      jsx_key_argument:callee(n.callee)==='createElement'?null:span(n.arguments[2]),
      additional_arguments:n.arguments.slice(callee(n.callee)==='createElement'?2:3).map(span),
      props:{...span(props),kind:props?.type==='ObjectExpression'?'ordered_object':props?.type==='Literal'&&props.value===null?'null':'opaque_expression',properties:list,helper_object_arguments:helperObjects},
      children:childTree,helper_children_candidates:helperPropNodes.filter(p=>!p.computed&&key(p.key)==='children').map(p=>({property:span(p),children:expression(p.value)})),
      class_and_style:propNodes.filter(p=>['className','style','dangerouslySetInnerHTML'].includes(key(p.key))).map(p=>({name:key(p.key),...span(p.value),static:p.value.type==='Literal',...literal(p.value),object_properties:properties(p.value),binding:binding(p.value)})),
      events:propNodes.filter(p=>/^on[A-Z]/.test(String(key(p.key)))).map(p=>({name:key(p.key),...span(p.value),callback:binding(p.value)})),
      text_and_localization_candidates:propNodes.filter(p=>['children','text','title','label','name','placeholder','description','tooltip','aria-label','alt'].includes(key(p.key))).map(p=>({name:key(p.key),...span(p.value),...literal(p.value),expression_kind:p.value.type,binding:binding(p.value)}))};
  });
  const definitions=[];
  for(const s of scopes)for(const [name,b]of s.defs)definitions.push({name,kind:b.kind,declaration:span(b.declaration),value:span(b.value),scope:span(s.node)});
  const functions=nodes.filter(n=>/Function/.test(n.type)).map(fn=>({id:`fn@${fn.start+base}`,...span(fn),name:fn.id?.name??null,parameters:(fn.params??[]).map(p=>({...span(p),names:names(p)})),
    returns:ownReturns(fn).map(n=>({statement:span(n),value:expression(n.argument)})),expression_body:fn.body.type==='BlockStatement'?null:expression(fn.body)}));
  const state=nodes.filter(n=>n.type==='CallExpression'&&['useState','useReducer','useSelector','setState','dispatch','useEffect','useLayoutEffect','useMemo','useCallback','subscribe','unsubscribe'].includes(callee(n.callee))).map(n=>({...span(n),method:callee(n.callee),callee:span(n.callee),arguments:n.arguments.map(span)}));
  const conditions=nodes.filter(n=>['IfStatement','ConditionalExpression','LogicalExpression','SwitchStatement','SwitchCase'].includes(n.type)).map(n=>({...span(n),test:span(n.test??n.discriminant??n.left),operator:n.operator??null,consequent:span(n.consequent??n.right),consequent_statements:Array.isArray(n.consequent)?n.consequent.map(span):null,alternate:span(n.alternate)}));
  const loops=nodes.filter(n=>['ForStatement','ForOfStatement','ForInStatement','WhileStatement','DoWhileStatement'].includes(n.type)||(n.type==='CallExpression'&&['map','filter','flatMap','reduce','forEach'].includes(callee(n.callee)))).map(n=>({...span(n),method:n.type==='CallExpression'?callee(n.callee):n.type,callee:span(n.callee),body:span(n.body),arguments:(n.arguments??[]).map(span)}));
  const methods=nodes.filter(n=>n.type==='MethodDefinition'||(n.type==='Property'&&n.method)).map(n=>({...span(n),name:key(n.key),computed:n.computed,value:span(n.value)}));
  return {id,path:component.path,sha256:component.sha256,offset:component.offset,end:component.end,node_type:component.node_type,source:raw,ui_elements:ui,
    function_definitions:functions,local_definitions:definitions,this_assignments:assignments.filter(a=>a.left.type==='MemberExpression'&&a.left.object.type==='ThisExpression').map(a=>({...span(a),property:key(a.left.property),value:span(a.right)})),
    methods,state_calls:state,conditions,loops,scope:'Syntax and component-local candidates; no parameter injection, module-outside declarations, computed DOM or callback execution inferred.'};
}
function summary(products) {
  const details=products.flatMap(p=>p.details),elements=details.flatMap(d=>d.ui_elements);
  return {products:products.length,pages:products.reduce((n,p)=>n+p.pages.length,0),component_references:products.reduce((n,p)=>n+p.pages.reduce((v,q)=>v+q.component_detail_ids.length,0),0),unique_component_ranges:details.length,
    components_with_ui:details.filter(d=>d.ui_elements.length).length,ui_call_receipts:elements.length,prop_receipts:elements.reduce((n,e)=>n+e.props.properties.length,0),helper_object_prop_receipts:elements.reduce((n,e)=>n+e.props.helper_object_arguments.reduce((v,o)=>v+o.properties.length,0),0),
    event_prop_receipts:elements.reduce((n,e)=>n+e.events.length,0),class_style_receipts:elements.reduce((n,e)=>n+e.class_and_style.length,0),text_candidate_receipts:elements.reduce((n,e)=>n+e.text_and_localization_candidates.length,0),
    function_definitions:details.reduce((n,d)=>n+d.function_definitions.length,0),state_call_receipts:details.reduce((n,d)=>n+d.state_calls.length,0),conditions:details.reduce((n,d)=>n+d.conditions.length,0),loop_receipts:details.reduce((n,d)=>n+d.loops.length,0),
    semantic_complete_pages_claimed:0,runtime_validation:'not_run'};
}
async function main(){
const minimum=process.argv.find(a=>a.startsWith('--min-product=')),maximum=process.argv.find(a=>a.startsWith('--max-product='));
const selected=graph.products.filter(p=>(!minimum||p.product_id>=Number(minimum.split('=')[1]))&&(!maximum||p.product_id<=Number(maximum.split('=')[1])));
const products=[],checkpoints=[],statistics=[];
function retain(record,checkpoint) {
  statistics.push(summary([record]));checkpoints.push(checkpoint);
  products.push({...record,unique_component_ranges:record.details.length,details:record.details.map(d=>({id:d.id,path:d.path,sha256:d.sha256,offset:d.offset,end:d.end,node_type:d.node_type,ui_calls:d.ui_elements.length,state_calls:d.state_calls.length,conditions:d.conditions.length,loops:d.loops.length}))});
}
for(const product of selected) {
  const checkpoint=path.join(root,'.work/product-ui-details-cache',product.product_id+'.json.gz');
  if(fs.existsSync(checkpoint)) {
    const saved=JSON.parse(zlib.gunzipSync(fs.readFileSync(checkpoint)));
    if(saved.tool_sha256===toolHash&&saved.graph_sha256===graphHash) {
      for(const detail of saved.product.details){const text=source(detail.path,detail.sha256);assert(text.slice(detail.offset,detail.end)===detail.source,'Changed retained detail '+detail.id);}
      if(process.argv.includes('--check'))validateProduct(saved.product);
      retain(saved.product,checkpoint);console.log('Product '+product.product_id+': source-verified detail checkpoint');continue;
    }
  }
  const details=new Map(),pages=[];
  for(const page of product.pages) {
    const refs=page.components.map(component=>{const id=`${component.path}@${component.offset}:${component.end}`;if(!details.has(id))details.set(id,extract(component));return id;});
    pages.push({page_id:page.page_id,page_key:page.page_key,display_mode:page.display_mode,primary:page.primary,navigation:page.navigation,
      component_detail_ids:refs,graph_edges:page.edges,root_component_detail_ids:[...new Set(page.edges.filter(e=>e.from===null).map(e=>refs[e.to]))],graph_unresolved:page.unresolved,graph_truncated:page.truncated,
      semantic_review:'not_completed_by_syntax_details'});
  }
  const record={product_id:product.product_id,name:product.name,categories:product.categories,pages,details:[...details.values()]};
  validateProduct(record);
  fs.mkdirSync(path.dirname(checkpoint),{recursive:true});fs.writeFileSync(checkpoint,zlib.gzipSync(JSON.stringify({tool_sha256:toolHash,graph_sha256:graphHash,product:record}),{level:9}));
  retain(record,checkpoint);
  console.log(`Product ${product.product_id}: ${pages.length} pages / ${details.size} source ranges / ${record.details.reduce((n,d)=>n+d.ui_elements.length,0)} UI calls`);
}
const stats={...statistics[0]};for(const stat of statistics.slice(1))for(const [name,value]of Object.entries(stat))if(typeof value==='number')stats[name]+=value;
const limitations=['UI calls are recognized by JSX/jsxs/createElement syntax; the graph retains module proofs, while this document does not recompute every call provider or execute React.',
  'Lexical containment is not runtime DOM parentage. Element children retain source order/conditions/iterations; component expansion needs passed props, HOC behavior and state observations.',
  'Every original prop and spread is preserved by range. Props supplied as arbitrary expressions/helper calls remain opaque; object arguments are candidates, not proved merge/override order.',
  'Local callback references and this-member assignments are candidates within each receipt, not all module/global declarations or guaranteed control-flow values.',
  'Text props and literals include localization expressions without guessing localized output. Full locale/import binding and rendered values remain separate work.',
  'Definitions/state/conditions/loops are original syntax; API names do not prove DLL read/write meaning, and handlers are not executed.',
  'Only the prior bounded graph is expanded; unresolved graph targets, dynamic imports and unreferenced/unsupported modules are not silently covered.',
  'No app, vendor JS, DLL, build or tests executed. UI operations remain in scope; DLL mutation/write-back remains deferred.'];
const reviews=semanticReviews();
const output={schema_version:1,tool_sha256:toolHash,page_graph_path:graphPath,page_graph_sha256:graphHash,source_date:'2026-10-02',offsets:'UTF-16, zero-based offset inclusive / end exclusive. Nested spans inherit path/hash and raw source from their component detail.',summary:stats,limitations,semantic_review_contracts:reviews};
async function archive(file) {
  fs.mkdirSync(path.dirname(file),{recursive:true});
  const gzip=zlib.createGzip({level:9}),stream=fs.createWriteStream(file),rawHash=crypto.createHash('sha256'),compressedHash=crypto.createHash('sha256');let rawSize=0;
  gzip.on('data',buffer=>compressedHash.update(buffer));gzip.pipe(stream);
  const done=once(stream,'close');
  async function append(text){const buffer=Buffer.from(text);rawSize+=buffer.length;rawHash.update(buffer);if(!gzip.write(buffer))await once(gzip,'drain');}
  await append(JSON.stringify(output).slice(0,-1)+',"products":[\n');
  for(const [index,checkpoint]of checkpoints.entries()){const saved=JSON.parse(zlib.gunzipSync(fs.readFileSync(checkpoint)));await append((index?',\n':'')+JSON.stringify(saved.product));}
  await append('\n]}\n');gzip.end();await done;
  return {data_sha256:compressedHash.digest('hex'),uncompressed_sha256:rawHash.digest('hex'),uncompressed_bytes:rawSize};
}
if(minimum||maximum) {
  const file=path.join(root,`.work/product-ui-details-range-${minimum?.split('=')[1]??'start'}-${maximum?.split('=')[1]??'end'}.json.gz`);await archive(file);
}else {
  const target=path.join(root,'docs/re/product-ui-details-current.json'),checking=process.argv.includes('--check'),archivePath=checking?path.join(root,'.work/product-ui-details-check.json.gz'):target+'.gz';
  const archiveFacts=await archive(archivePath),index={...output,data_file:'product-ui-details-current.json.gz',...archiveFacts,products};
  const json=JSON.stringify(index,null,2)+'\n';
  if(process.argv.includes('--check')) {
    assert(sha(read('docs/re/product-ui-details-current.json.gz'))===archiveFacts.data_sha256,'UI detail archive drift');
    assert(read('docs/re/product-ui-details-current.json').toString('utf8')===json,'UI detail index drift');
  }else fs.writeFileSync(target,json);
  const md=renderMarkdown(index),mdPath='docs/re/product-ui-details-current.md';
  if(checking)assert(read(mdPath).toString('utf8')===md,'UI detail markdown drift');else fs.writeFileSync(path.join(root,mdPath),md);
}
console.log(JSON.stringify(stats));
}
function validateProduct(product) {
  const details=new Map(product.details.map(d=>[d.id,d]));
  for(const page of product.pages){for(const id of page.component_detail_ids)assert(details.has(id),'Missing detail '+id);for(const edge of page.graph_edges)assert((edge.from===null||(edge.from>=0&&edge.from<page.component_detail_ids.length))&&edge.to>=0&&edge.to<page.component_detail_ids.length,'Invalid page edge '+page.page_id);}
  for(const detail of product.details) {
    const text=source(detail.path,detail.sha256);assert(text.slice(detail.offset,detail.end)===detail.source,'Changed detail source '+detail.id);
    function verify(value) {
      if(!value||typeof value!=='object')return;
      if('offset' in value&&'end' in value)assert(value.offset>=detail.offset&&value.end<=detail.end&&value.end>value.offset,'Invalid nested detail range '+detail.id+' '+JSON.stringify(value));
      if(value.node_type&&'offset' in value)assert(typeof value.node_type==='string','Invalid node type');
      for(const child of Object.values(value))if(Array.isArray(child))child.forEach(verify);else if(child&&typeof child==='object')verify(child);
    }
    verify(detail);
    const elementIDs=new Set(detail.ui_elements.map(e=>e.id));
    for(const element of detail.ui_elements){assert(elementIDs.has(element.id),'Element id');if(element.lexical_parent_element)assert(elementIDs.has(element.lexical_parent_element),'Missing lexical element parent');}
  }
}
function semanticReviews() {
  const mouse='.ref/devices/226/static/js/8355.3d5e573e.chunk.js',keyboard='.ref/devices/691/static/js/OLED.b7b95581.chunk.js',audio='.ref/devices/1383/static/js/6141.5d00192e.chunk.js',receiver='.ref/devices/164/static/js/main.458d4103.js';
  const contracts=[
    {id:'mouse-performance-columns',product_id:226,path:mouse,offset:183668,end:183979,title:'鼠标 Performance 根布局与 BLE 门控',facts:['BodyWidget 的 left 先挂 ys；right 顺序为 e&&Xs、yi、ei。e 等于 !isBle || DeviceInfo.supportBluetoothPollingRate===true，因此 BLE 支持条件只影响这处 Xs 插槽。','这里只证明原组件排列和条件；左右列最终宽度、边距、子组件含义必须继续沿组件证据与 CSS 链核对。']},
    {id:'mouse-dpi-controls',product_id:226,path:mouse,offset:149688,end:154848,title:'鼠标 DPI 切换、阶段与双轴联动',facts:['Widget 子元素按说明、双向 tab 或 stage-control、动态 disabled 主体排列；twoway-lighting no-inner-border 追加动态 class，普通 stage-control 原样带 zIndex:2。','缺少或空 dpiStages 时返回 null。阶段网格还依赖 selectedProfile；常规 maxDPI/minDPI/step 分别存在 16000/100/50 的回退，singleDPI 使用 minDPIConfig/maxDPIConfig。','关闭独立轴时 y 复制 x；changeX 在未独立时同步 y。changeX/changeY 接收的第三参数会抑制 dispatch，不能省略该条件。','禁用当前阶段会选下一可见阶段或首个可见阶段；updateStages 会拒绝非数组及空洞并维护可见回退。阶段关闭和 two-way 分支还含夹取及 updateDPIStage。','componentDidMount 关闭 configure sensitivity 状态；enableStages 变化会命令式修改 .stage input.slider。不能只把 JSX 描述当作全部交互。','setActiveDPI/活动索引必须保留原表达式，本审阅不擅自纠正看似异常的索引。']},
    {id:'mouse-sensitivity-matcher',product_id:226,path:mouse,offset:176795,end:182999,title:'Sensitivity Matcher 状态、弹层及消息边界',facts:['打开会登记 window click；外部点击根据 popup 和 ref containment 判定并发消息、移除监听。关闭时按 calibrationState.state 决定 cancel/reset；卸载也清除监听。','READY、CALIBRATING、ERROR、COMPLETED 分支决定内容顺序；进度宽度为 process%；完成表格按 currentDpi、newDpi 排列，reset 可点击。','原弹层 minHeight:569px；动作区 marginTop:120px；Apply 在非 COMPLETED 时追加 disabled 类。ERROR 还挂 retry/cancel dialog。','cancel 方法比较整个 this.props.calibrationState 与 CALIBRATING，其他位置比较 .state；这是原文差异，不能改写成统一正确行为。','Apply 关闭并发送消息，不证明设备保存成功。消息常量 ui/mi/Si/vi/gi/Ci 尚须沿模块定义解析，不在这里猜测 DLL 命令。']},
    {id:'keyboard-oled-root',product_id:691,path:keyboard,offset:98640,end:99355,title:'键盘 OLED 根布局及编辑门控',facts:['根组件保存 popup=false 和 selectedEdit=null。BLE 只阻断 animation/image/emote/banner 编辑入口；progress 取 oledLoadingReducer.oledLoading.type。','外层先 Wi、ta；内层先 preview di，再 left[hi,wi]、right[xi,Gi,ea]；可选 pi 弹层收到 selectedEdit/show/onClose，关闭同时清理两个本地状态。','页面的 main 异步根仍负责加载/失败分支、result.default 与清理。此记录不运行 lazy loader。']},
    {id:'keyboard-oled-home-preview',product_id:691,path:keyboard,offset:79229,end:86040,title:'键盘 OLED 首页预览、轮播与应用',facts:['读取 home enabled/selected 及 animation/image/emote/banner/media/system/OLEDlang；slides 仅保留 left||right。数量变化重置索引，触发轮播按 (idx+1)%R 与 timeBetweenSlides*1000 更新，清理 interval。R 为 0 的余数问题只记录为原表达式，未作运行结论。','首页 tile 顺序是 animation(0)、image(1)、emote(4)、banner(2)、media(5)、system(6)、keyboard/headset(3)。media/system 需要 Synapse 且 BLE 禁用；产品专用 tile 不可编辑。','home 禁用时叠加 ci；loading 改变外层 disabled class。巨型 base64 产品图仍保留在完整原文范围，MD 不复印图像编码。','type 6 的 Apply 克隆 system；OLEDlang===1 时用 getTextItem 的 _lang:"zh-CN" 写 deviceLabel，否则取信息标签并 dispatch m；其他类型 dispatch d({enabled:x,selected:e})。这些是 UI action，无 DLL 写回成功结论。']},
    {id:'keyboard-oled-dim',product_id:691,path:keyboard,offset:95389,end:96010,title:'键盘 OLED Dim Display',facts:['选项来自 fi.OLED_DIM_DISPLAY_VALUES||Ji；点击发送 SET_OLED_TIME_TO_DIM，payload 为当前 enabled 和新 value。','按钮容器 marginTop:"20px"；!enabled 时叠加遮罩。active 的 concat(e===s&&"active") 可保留 false 字符串，不能静默规范化成空串。','Ji 的已确认数组回退是 [1,3,5,10,15]，并不证明每个产品配置都采用该值或时间单位。']},
    {id:'audio-sound-root',product_id:1383,path:audio,offset:415171,end:416558,title:'耳机 Sound 根布局与状态同步',facts:['kr mount 调用 getSettings；THX 改变时同步本地 isTHX；切换 setTHXSpatialEnable 的参数为 !this.state.isTHX。','根顺序为 customize-audio flex 容器(Pr)、chart-eq(Wo,In)、eo.A 两列。left 为 mr，right 为 Ao(showGameChatBalance:true)、jo。','Wo 固定 isStereo:true；In 收到 disableRequiredSynapseForCustom:true、secondTooltip:true、compactMode:true 和 te 中 bandDataConfig/eqChartOpt/yAxisTitle。不能以普通双列控件替代顶部两块。','外围 connector 把 spatial/eq/customize 状态与 bindActionCreators 注入 kr；这不证明每次调用都实际到达 DLL。']},
    {id:'audio-volume',product_id:1383,path:audio,offset:284339,end:285904,title:'耳机音量本地状态及禁用条件',facts:['local volume 初始化自 props；toggle 的 setState 回调调用 setValues→props.setVolume；changeValue 以 !!value 更新 enabled，0 会关闭 enabled；props 变化时 componentDidUpdate 同步。','仅 inputSource 属于 aux/bluetooth/optical 且 canDisabledVolume 时 disabled。Widget 使用 hasSwitch、local active、Slider 0..100/step 1/prop debounceTime；customTips 存在回退。','externalSoundProperties 点击调用 j.MN()；showGameChatBalance 条件挂 wo。changeVolume 的存在不证明该处理器被某个控件实际使用。']},
    {id:'audio-game-chat',product_id:1383,path:audio,offset:282848,end:284155,title:'耳机 Game/Chat Balance',facts:['本地 balanceMode 初始化自 props；changeValue 在 setState 回调调用 setGameChatBalance；props 变化同步；tutorial 使用 showGameChatTutorial:true。','标题行 style 为 marginTop:"10px"、display:"flex"、alignItems:"center"、zIndex:0、gap:"6px"；标题 uppercase，其后 help icon/tip。','Slider min:0/max:config.max||20/step:1。教程链接 marginTop:"45px"、textDecoration:"underline"。V.* 文案键尚不当作最终翻译。']},
    {id:'receiver-pairing-frame',product_id:164,path:receiver,offset:4582679,end:4585320,title:'接收器/底座多设备配对 iframe 边界',facts:['eD props 含 widgetHeight(默认"100%")、iframeId、deviceInfo、deviceName、allMasters(默认[])；QC.Zv 归一 product/category/boolean flags/string dongleId，JC.eA 归一 masters。','同源 URL /synapse/multipairing/ 带 displayMode=multiDevicePairing、可选 containerId、productId/pid、category、配对/productivity flags、deviceName、serialNumber、lang、allMasters JSON。','发送 multiDevicePairingInit payload={deviceInfo:r,deviceName:t,allMasters:T}。Ready listener 同时检查 event.source===iframe.contentWindow、event.origin===targetOrigin 和 eventName===multiDevicePairingReady。注册/清理 effect、另一个 init effect 及 onLoad 都保留。','wrapper position:"relative"、width:"100%"、height:E；iframe id 回退 iframeMultiDevicePairingWidget/frameBorder:0，并以 {...$C,zIndex:1} 定位。$C 的原静态对象单列证据。','这里是嵌入入口和消息握手，不证明 multipairing 应用内部流程、无线查询或硬件配对成功。']},
    {id:'receiver-pairing-route',product_id:164,path:receiver,offset:4587157,end:4587565,title:'接收器配对导航分支',facts:['renderView 以 active_view 查找导航 name；Q.d9b 分支按 href 缓存 allMasters，随后挂 ED，传 deviceInfo/deviceName/allMasters；不匹配返回 null。','ED=P().memo(eD) 为独立包装证据。配对 UI 与硬件操作不能因路由存在就标成完成。']}
  ];
  for(const c of contracts){const text=source(c.path);c.sha256=hashes.get(c.path);c.source=text.slice(c.offset,c.end);const ast=acorn.parseExpressionAt('('+c.source+')',0,{ecmaVersion:'latest'});assert(ast.end===c.source.length+1,'Review expression '+c.id);}
  for(const [id,product_id,file,anchor,startOffset,title]of [
    ['keyboard-oled-dim-fallback',691,keyboard,'Ji=',95000,'Dim Display 默认数组'],
    ['receiver-frame-style',164,receiver,'$C=',4582000,'iframe 静态定位对象'],
    ['receiver-memo-wrapper',164,receiver,'P().memo(eD)',4585000,'iframe memo 包装']]) {
    const text=source(file),position=text.indexOf(anchor,startOffset);assert(position>=0,'Missing semantic anchor '+id);
    const offset=position+(anchor.endsWith('=')?anchor.length:0),ast=acorn.parseExpressionAt(text,offset,{ecmaVersion:'latest'});
    contracts.push({id,product_id,path:file,offset,end:ast.end,sha256:hashes.get(file),title,facts:['独立模块邻接声明的原表达式；只用于上述对应契约，不泛化到其他命名空间。'],source:text.slice(offset,ast.end)});
  }
  return contracts;
}
function renderMarkdown(index) {
  const s=index.summary,rows=index.products.map(p=>`| ${p.product_id} | ${String(p.name).replace(/\|/g,'\\|')} | ${p.pages.length} | ${p.unique_component_ranges} | ${p.details.reduce((n,d)=>n+d.ui_calls,0)} |`).join('\n');
  const reviews=index.semantic_review_contracts.map(c=>`### ${c.product_id} · ${c.title}\n\n证据 ID：\`${c.id}\`；\`${c.path}\`，UTF-16 \`[${c.offset},${c.end})\`；SHA-256 \`${c.sha256}\`。原片段完整保存在 JSON 的 \`semantic_review_contracts[].source\`。\n\n${c.facts.map(f=>'- '+f).join('\n')}\n`).join('\n');
  return `# 当前全部产品 UI 原文细节与代表页语义核对\n\n当前源快照为 2026-10-02；此文档由维护工具静态解析当前产品包，不执行原 JavaScript。它补全 [全产品页面调用链](all-product-page-chains-current.md) 的组件内部结构证据，和 [布局/样式索引](all-product-layout-chains-current.md)、[实际样式加载与字体](ui-style-sources-current.md)、[共享控件审阅](shared-ui-controls-current.md) 配合使用。\n\n## 覆盖与完成口径\n\n已保留 ${s.products} 个产品 / ${s.pages} 个页面 / ${s.component_references} 个页面组件引用；产品内去重后 ${s.unique_component_ranges} 个原组件范围。共 ${s.ui_call_receipts} 个 UI 调用、${s.prop_receipts} 个直接对象属性、${s.helper_object_prop_receipts} 个 helper 对象属性候选、${s.event_prop_receipts} 个事件属性、${s.class_style_receipts} 个 class/style 属性、${s.text_candidate_receipts} 个文案候选、${s.function_definitions} 个函数定义、${s.state_call_receipts} 个状态/订阅调用、${s.conditions} 个条件节点、${s.loop_receipts} 个循环/迭代节点。\n\n**这些数值是原文范围及语法证据覆盖，不是所有页面已经语义逆向完毕。semantic_complete_pages_claimed=${s.semantic_complete_pages_claimed}。** 本节后的 ${index.semantic_review_contracts.length} 个原文契约只覆盖 226 鼠标、691 键盘、1383 耳机和 164 底座的明确分支。未宣称当前 Rust UI 达到原版或 DLL 调用成功；未运行应用、DLL、构建或测试。\n\n## 可回查的数据结构\n\n- [轻量索引](product-ui-details-current.json)：产品/页面/组件 IDs、页面根、原调用链 edges、unknowns、统计与语义契约；范围按 UTF-16 零起点左闭右开。\n- [完整 gzip JSON](product-ui-details-current.json.gz)：每个组件的原文 source、路径、字节 SHA-256，以及每项嵌套证据的 offset/end/node_type。每行一个产品记录，适合流式读取；解压后 ${index.uncompressed_bytes} 字节，不应一次加载全部到内存。\n- ui_elements 按原调用出现次序记录 callee、组件表达式、完整 ordered props/spreads、jsx 第三 key 参数、原 children 的数组/分支/迭代顺序、事件、class/style 和文案表达式。createElement 的 children 来自第三及后续参数。\n- lexical_parent_element 仅是 AST 的嵌套关系；opaque props/helper_object_arguments/helper_children_candidates 不推断合并顺序或最终 props。静态 literal 使用 static_value，不覆盖原 value 范围。\n- function_definitions/local_definitions/methods/this_assignments 保存组件内声明与回调候选；state_calls/conditions/loops 保留状态及条件原式。函数 returns 排除嵌套函数，不把回调 return 当父函数 return。\n- pages 的 component_detail_ids 对接原 graph_edges；root_component_detail_ids 只从 from=null 的真实根取得。页面 shared 范围在每个产品内去重，不丢失页面引用。\n\n完整数据字节 SHA-256：\`${index.data_sha256}\`；解压内容 SHA-256：\`${index.uncompressed_sha256}\`；输入调用链 SHA-256：\`${index.page_graph_sha256}\`。\n\n## 明确未解决的边界\n\n${index.limitations.map(f=>'- '+f).join('\n')}\n\n模块外变量、传入 props、HOC、动态 selector 数据、computed 名称、CSS cascade 和 localization 输出都不能只凭本索引认定已闭环。所有 graph unresolved 保留；包括 React Fragment 条件赋值不能假设 Symbol 环境选分支。原 callbacks 不等于实际用户动作成功；UI Apply/Save 和本地 draft 操作仍在范围，DLL mutation/write-back 仍留待协调集成。\n\n## 代表页原代码语义契约\n\n${reviews}\n## 全产品索引\n\n| 产品 ID | 当前源名称 | 页面 | 原组件范围 | UI 调用 |\n|---|---|---:|---:|---:|\n${rows}\n\n## 维护及静态校验\n\n运行 \`node tools/extract-product-ui-details-current.cjs\` 重建；\`--check\` 对全部源字节 SHA、原文切片、所有嵌套范围、页面引用/edge 和 lexical parent 做验证，并重建归档比对压缩内容、轻量 JSON 与此 MD。仅执行自有静态解析工具，不执行被审阅的包。\n\n检查点保存在 \`.work/product-ui-details-cache/\`，只有工具及调用链 SHA 都一致且当前源重新核对通过才复用。缓存未代表额外语义验证。此文档与数据不可作为“整个产品包所有逻辑已还原”的证明；下一步须逐个页面沿参数、共享组件、事件/action、host/service/DLL 与样式文案绑定继续闭环。\n`;
}
main().catch(error=>{console.error(error.stack);process.exitCode=1;});
