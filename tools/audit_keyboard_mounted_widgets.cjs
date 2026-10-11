// Fresh AST caller audit. Parse only: never evaluate product JavaScript.
const fs=require('fs'),crypto=require('crypto'),acorn=require('acorn');
const products=JSON.parse(fs.readFileSync('crates/razer-pages/src/features/keyboard_products_data.json','utf8'));
const args=process.argv.slice(2).map(Number).filter(Number.isFinite);
const results=[];
const key=node=>node?.name??node?.value;
function persist(records){
 const properties=JSON.parse(fs.readFileSync('crates/razer-pages/src/features/keyboard_properties_data.json','utf8'));
 const gaming=JSON.parse(fs.readFileSync('crates/razer-pages/src/features/keyboard_gaming_review_data.json','utf8'));
 const native=fs.readFileSync('crates/razer-pages/src/features/keyboard_products.rs','utf8');
 const sha=s=>crypto.createHash('sha256').update(s).digest('hex');
 const compact=records.map(row=>{
  const s=fs.readFileSync(row.path,'utf8');if(sha(s)!==row.sha256)throw Error('Source changed since AST audit '+row.product_id);
  return {product_id:row.product_id,path:row.path,sha256:row.sha256,native:{properties:properties.find(p=>p.product_id===row.product_id)??null,gaming:gaming.find(p=>p.product_id===row.product_id)??null,source_layout:products.find(p=>p.product_id===row.product_id)?.source_layout??null},widgets:Object.fromEntries(Object.entries(row.widgets).map(([kind,defs])=>[kind,defs.map(def=>({symbol:def.symbol,aliases:def.aliases,definition:def.definition,callers:def.callers.map(call=>({call:call.call,owner_symbol:call.owner_symbol,owner:call.owner?{byte_offset:call.owner.byte_offset,char_range:call.owner.char_range,sha256:sha(call.owner.source)}:null,container:call.container,context:{byte_offset:Buffer.byteLength(s.slice(0,Math.max(0,call.call.char_range[0]-500))),source:s.slice(Math.max(0,call.call.char_range[0]-500),call.call.char_range[1]+400)}}))}))]))};
 });
 const noMainCall=kind=>compact.filter(row=>!row.widgets[kind].some(def=>def.callers.length)).map(row=>row.product_id);
 const output={method:'Fresh current direct main bytes parsed with Acorn; leaf component, aliases and actual JSX call/container retained. A direct call is not a complete page/navigation acceptance claim. Products without main callers require their current lazy chunk and route audit; absence in main is not vendor absence. UTF-8 byte offsets.',date:'2026-10-11',native_predicates:[...native.matchAll(/[^\r\n]*(?:product_id|source_layout|source_mod_tap|source_keyboard_top_padding_percent)[^\r\n]*/g)].map(m=>({line:native.slice(0,m.index).split('\n').length,source:m[0]})),summary:{products:compact.length,snap_main_callers:compact.length-noMainCall('snap').length,gaming_main_callers:compact.length-noMainCall('gaming').length,properties_main_callers:compact.length-noMainCall('properties').length,without_main_snap_caller:noMainCall('snap'),without_main_gaming_caller:noMainCall('gaming'),without_main_properties_caller:noMainCall('properties')},products:compact};
 fs.writeFileSync('docs/re/keyboard-product-predicates-current-source.json',JSON.stringify(output,null,2)+'\n');
}
if(process.argv.includes('--summarize')){persist(JSON.parse(fs.readFileSync('.work/keyboard-mounted-widgets/receipts.json','utf8')));process.exit(0);}
function children(node){return Object.values(node).flatMap(v=>Array.isArray(v)?v:v?.type?[v]:[]).filter(v=>v?.type);}
for(const product of products.filter(p=>!args.length||args.includes(p.product_id))){
 const pid=product.product_id,dir=`local-ui-reverse/source/official/apps.razer.com/synapse/products/${pid}/ui/static/js/`;
 const path=dir+fs.readdirSync(dir).find(p=>p.startsWith('main.')),bytes=fs.readFileSync(path),source=bytes.toString('utf8');
 const tree=acorn.parse(source,{ecmaVersion:'latest'}),parent=new WeakMap(),nodes=[];
 function walk(node){nodes.push(node);for(const child of children(node)){parent.set(child,node);walk(child);}}walk(tree);
 const slice=n=>source.slice(n.start,n.end);
 const receipt=n=>({byte_offset:Buffer.byteLength(source.slice(0,n.start)),char_range:[n.start,n.end],source:slice(n)});
 const ancestors=n=>{const list=[];for(let p=parent.get(n);p;p=parent.get(p))list.push(p);return list;};
 const owner=n=>ancestors(n).find(p=>p.type==='ClassDeclaration'||p.type==='FunctionDeclaration'||p.type==='VariableDeclarator'&&p.id.type==='Identifier');
 const symbol=n=>n?.id?.name;
 function aliases(original){const set=new Set([original]);let changed=true;while(changed){changed=false;for(const node of nodes){if(node.type!=='VariableDeclarator'||node.id.type!=='Identifier'||set.has(node.id.name)||!node.init)continue;const init=node.init;
   // Direct aliases and react-redux connect(...)(Component), with optional transform wrapping.
   const ref=init.type==='Identifier'?init.name:init.type==='CallExpression'&&init.arguments.length===1&&init.arguments[0].type==='Identifier'?init.arguments[0].name:null;
   if(set.has(ref)){set.add(node.id.name);changed=true;}
 }}return [...set];}
 const jsx=nodes.filter(n=>n.type==='CallExpression'&&(n.callee.type==='SequenceExpression'?n.callee.expressions.at(-1):n.callee)?.type==='MemberExpression'&&['jsx','jsxs'].includes(key((n.callee.type==='SequenceExpression'?n.callee.expressions.at(-1):n.callee).property)));
 const definitions={snap:[],gaming:[],properties:[]};
 for(const node of nodes){
  if(node.type==='Property'&&key(node.key)==='extraClass'&&node.value.type==='Literal'&&node.value.value==='snap-tap-widget')definitions.snap.push(owner(node));
  if(node.type==='Property'&&key(node.key)==='id'&&node.value.type==='Literal'&&node.value.value==='winKey')definitions.gaming.push(owner(node));
  if(node.type==='CallExpression'&&node.callee.type==='MemberExpression'&&key(node.callee.property)==='OpenKeyboardProperties'&&ancestors(node).some(n=>n.type==='Property'&&key(n.key)==='onClick'||n.type==='AssignmentExpression'&&key(n.left.property)==='openKeyboardProperties'))definitions.properties.push(owner(node));
 }
 const row={product_id:pid,path,sha256:crypto.createHash('sha256').update(bytes).digest('hex'),widgets:{}};
 for(const [kind,definitionsForKind] of Object.entries(definitions)){
  row.widgets[kind]=[...new Set(definitionsForKind.filter(Boolean))].map(def=>{
   const names=aliases(symbol(def));
   const callers=jsx.filter(n=>n.arguments[0]?.type==='Identifier'&&names.includes(n.arguments[0].name)&&!(n.start>=def.start&&n.end<=def.end));
   return {symbol:symbol(def),aliases:names,definition:receipt(def),callers:callers.map(call=>{
    const chain=ancestors(call),container=chain.find(n=>jsx.includes(n)),callerOwner=owner(call);
    return {call:receipt(call),owner_symbol:symbol(callerOwner),owner:callerOwner?receipt(callerOwner):null,container:container?receipt(container):null,containers:chain.filter(n=>jsx.includes(n)).slice(0,5).map(receipt)};
   })};
  });
 }
 results.push(row);
 console.log(JSON.stringify({pid,widgets:Object.fromEntries(Object.entries(row.widgets).map(([kind,defs])=>[kind,defs.map(def=>({symbol:def.symbol,aliases:def.aliases,callers:def.callers.map(call=>({byte_offset:call.call.byte_offset,owner_symbol:call.owner_symbol,container:call.container?.source.slice(0,140)}))}))]))}));
}
fs.mkdirSync('.work/keyboard-mounted-widgets',{recursive:true});
fs.writeFileSync('.work/keyboard-mounted-widgets/receipts.json',JSON.stringify(results,null,2));
persist(results);
