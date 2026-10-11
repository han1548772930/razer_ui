// Re-audit 515 and 521 from current bytes before sharing the ordinary renderer.
const fs=require('fs'),crypto=require('crypto'),acorn=require('acorn');
const hash=s=>crypto.createHash('sha256').update(s).digest('hex');
const key=n=>n?.name??n?.value;
const records=[];
function normalize(root){const ids=new Map();function visit(n,p,field){if(Array.isArray(n))return n.map(c=>visit(c,p,field));if(!n||typeof n!=='object')return n;if(n.type==='Identifier'){const property=p?.type==='MemberExpression'&&field==='property'&&!p.computed||p?.type==='Property'&&field==='key'&&!p.computed;let name=n.name;if(!property){if(!ids.has(name))ids.set(name,'v'+ids.size);name=ids.get(name);}return{type:n.type,name};}return Object.fromEntries(Object.keys(n).filter(k=>!['start','end','raw'].includes(k)).map(k=>[k,visit(n[k],n,k)]));}return JSON.stringify(visit(root));}
for(const pid of [515,521]){
 const root=`local-ui-reverse/source/official/apps.razer.com/synapse/products/${pid}/ui/static/`,path=root+'js/'+fs.readdirSync(root+'js').find(f=>f.startsWith('main.'));
 const bytes=fs.readFileSync(path),s=bytes.toString('utf8'),tree=acorn.parse(s,{ecmaVersion:'latest'}),nodes=[],parents=new WeakMap();
 function walk(n){if(!n?.type)return;nodes.push(n);for(const v of Object.values(n)){for(const c of Array.isArray(v)?v:v?.type?[v]:[]){if(c?.type){parents.set(c,n);walk(c);}}}}walk(tree);
 const owner=n=>{for(let p=parents.get(n);p;p=parents.get(p))if(p.type==='VariableDeclarator'&&p.id.type==='Identifier'||p.type==='ClassDeclaration')return p;};
 const findMarker=marker=>owner(nodes.find(n=>n.type==='Literal'&&n.value===marker));
 const wrapper=findMarker('snap-tap-widget'),editor=findMarker('snap-tap-wrapper');
 const rows=nodes.filter(n=>n.type==='Property'&&key(n.key)==='keyGroup'&&n.start>=editor.start&&n.end<=editor.end).map(n=>parents.get(n)).map(n=>parents.get(n)).filter(n=>n?.type==='CallExpression').map(n=>n.arguments[0].name);
 const row=nodes.filter(n=>n.type==='VariableDeclarator'&&rows.includes(n.id?.name)&&n.start<editor.start).at(-1);
 const reducer=nodes.find(n=>n.type==='Property'&&key(n.key)==='snapTapReducer'&&n.value.type==='FunctionExpression');
 const defaults=nodes.find(n=>n.type==='VariableDeclarator'&&n.init?.type==='ObjectExpression'&&['isEnabled','keyList','pressedKeys'].every(k=>n.init.properties.some(p=>key(p.key)===k)));
 const forbidden=nodes.find(n=>n.type==='VariableDeclarator'&&n.init?.type==='ArrayExpression'&&n.init.elements.map(n=>n?.value).join(',')==='KEY_APPLICATION,KEY_LEFT_GUI,KEY_FN,DKM_F6,DKM_D2');
 const inputHead=/(?:^|[,\{])46114:/.exec(s);let inputModule=acorn.parseExpressionAt(s,inputHead.index+inputHead[0].length,{ecmaVersion:'latest'});if(inputModule.type==='SequenceExpression')inputModule=inputModule.expressions[0];
 const layoutHead=/(?:^|[,\{])90857:/.exec(s);let layoutModule=acorn.parseExpressionAt(s,layoutHead.index+layoutHead[0].length,{ecmaVersion:'latest'});if(layoutModule.type==='SequenceExpression')layoutModule=layoutModule.expressions[0];
 const fragments={wrapper:wrapper.init,editor:editor.init,row:row.init,reducer:reducer.value,defaults:defaults.init,forbidden:forbidden.init,inputModule,layoutModule};
 const colMarker=nodes.find(n=>n.type==='Literal'&&n.value==='widget-col col-');
 let column=parents.get(colMarker);while(column&&column.type!=='ClassDeclaration')column=parents.get(column);
 if(!column)throw Error('Unresolved actual source column wrapper '+pid);
 fragments.columnWrapper=column;
 const helpers={captureState:'EA',messageState:pid===515?'bl':'Kl',razerKeys:pid===515?'wc':'Kc',isRazer:pid===515?'zc':'kc',razerName:pid===515?'kc':'zc',duplicate:pid===515?'jl':'$l',keyEvent:'jc',beginCapture:'Zc',endCapture:'qc',textRecorder:'tA',keypadForbidden:pid===515?'Xl':'Jl'};
 for(const [name,symbol] of Object.entries(helpers)){
  const node=nodes.filter(n=>['VariableDeclarator','FunctionDeclaration'].includes(n.type)&&n.id?.name===symbol&&n.start<editor.start).at(-1);
  if(!node)throw Error('Unresolved source helper '+pid+' '+name);
  fragments[name]=node.init??node;
 }
 const record={product_id:pid,path,sha256:hash(bytes),fragments:Object.fromEntries(Object.entries(fragments).map(([name,node])=>[name,{byte_offset:Buffer.byteLength(s.slice(0,node.start)),source:s.slice(node.start,node.end),normalized_ast_sha256:hash(normalize(node))}]))};
 // Keep the exact JSX mount and its direct source column wrapper.
 const symbol=wrapper.id.name,pattern=new RegExp('\\(0,[\\w$]+\\.(?:jsx|jsxs)\\)\\('+symbol+'[,]','g');
 record.mounts=[...s.matchAll(pattern)].filter(m=>m.index<wrapper.start||m.index>wrapper.end).map(m=>({byte_offset:Buffer.byteLength(s.slice(0,m.index)),source:s.slice(m.index-320,m.index+170)}));
 records.push(record);
}
const compared=Object.keys(records[0].fragments).map(name=>({name,equal:records[0].fragments[name].normalized_ast_sha256===records[1].fragments[name].normalized_ast_sha256}));
const output={method:'Direct current main files; static AST alpha-renaming preserves property names, literals, identifier correlations and all syntax. Shared renderer comparison is not runtime/device acceptance.',compared,records};
fs.writeFileSync('docs/re/keyboard-521-snap-reuse-current-source.json',JSON.stringify(output,null,2)+'\n');
if(compared.some(row=>!row.equal))throw Error('Source renderer sharing needs branch inspection: '+JSON.stringify(compared));
console.log(JSON.stringify(compared));
