// Extract the Dashboard's window-open contract without evaluating JavaScript.
// The host opens product roots in named, reusable windows: each displayMode
// window is created through `_P(url, name, ...flags)` with the flag enum below.
// Any second-window implementation must follow the same names and flags.
const fs=require('fs'),path=require('path'),crypto=require('crypto'),acorn=require('acorn');
const root=path.resolve(__dirname,'..');
const hash=value=>crypto.createHash('sha256').update(value).digest('hex');
const read=relative=>fs.readFileSync(path.join(root,relative));
const app='.ref/applications/synapse/dashboard/static/js/App.72827d47.chunk.js';
const pairing='.ref/applications/synapse/dashboard/static/js/7861.1b0e99a4.chunk.js';
const appBytes=read(app),appSource=appBytes.toString('utf8');
const pairingBytes=read(pairing),pairingSource=pairingBytes.toString('utf8');
const receipt=(file,bytes,node)=>({path:file,sha256:hash(bytes),offset:node.start,end:node.end,source:appSource.slice(node.start,node.end)});
const ast=acorn.parse(appSource,{ecmaVersion:'latest'});
let moduleBody=null;
const walk=(node,visit)=>{
 if(!node?.type)return;
 visit(node);
 for(const value of Object.values(node)){
  if(Array.isArray(value))value.forEach(entry=>walk(entry,visit));
  else if(value?.type)walk(value,visit);
 }
};
walk(ast,node=>{
 if(moduleBody)return;
 if(node.type==='Property'&&(node.key.value??node.key.name)===84058&&/Function/.test(node.value.type))moduleBody=node.value;
});
if(!moduleBody)throw Error('Missing Dashboard window module 84058');
// Read only the module's own top-level statements: minified inner scopes reuse
// the same single-letter names.
const statements=moduleBody.body.type==='BlockStatement'?moduleBody.body.body:[];
const declared=new Map();
for(const statement of statements){
 if(statement.type==='VariableDeclaration')for(const declarator of statement.declarations)
  if(declarator.id.type==='Identifier')declared.set(declarator.id.name,declarator.init);
 if(statement.type==='FunctionDeclaration'&&statement.id)declared.set(statement.id.name,statement);
}
const flagsNode=declared.get('i');
if(flagsNode?.type!=='ObjectExpression')throw Error('Missing window flag enum');
const flags={};
for(const property of flagsNode.properties){
 if(property.type!=='Property'||property.value.type!=='Literal')throw Error('Changed window flag enum shape');
 flags[property.key.name??property.key.value]=property.value.value;
}
for(const expected of ['sameWindow','diffWindow','diffWindowSingleProcess','tabVisible','tabInvisible','windowVisible','windowInvisible','autoFocus','chromaIcon','synapseIcon'])
 if(!(expected in flags))throw Error(`Missing window flag ${expected}`);
const openNode=declared.get('o'),nameNode=declared.get('n');
if(!/Function/.test(openNode?.type??''))throw Error('Missing window open function');
if(!/Function/.test(nameNode?.type??''))throw Error('Missing window name function');
const nameSource=appSource.slice(nameNode.start,nameNode.end);
const openSource=appSource.slice(openNode.start,openNode.end);
// Window names are literal templates or hyphenated strings in the name
// function; `typeof x === "string"` guards in the same body are not names.
const names=[...nameSource.matchAll(/`([^`]+)`|"([\w-]*)"|'([\w-]*)'/g)]
 .map(match=>match[1]??match[2]??match[3])
 .filter(name=>name&&(name.includes('${')||name.includes('-')));
const pairingOpen=/new URL\(`([^`]+)`[^)]*\)([\s\S]{0,700}?)\)\}\}/.exec(pairingSource);
if(!pairingOpen)throw Error('Missing multi-device pairing window construction');
// Minified calls separate statements with commas, so the argument is read with
// a depth- and string-aware scan instead of a closing-`)` regex.
function argumentAfter(text,start){
 let depth=0;
 for(let index=start;index<text.length;index++){
  const character=text[index];
  if(character==='"'||character==="'"||character==='`'){
   const quote=character;index++;
   while(index<text.length&&text[index]!==quote){if(text[index]==='\\')index++;index++;}
   continue;
  }
  if(character==='('||character==='['||character==='{')depth++;
  else if(character===')'||character===']'||character==='}'){
   if(depth===0)return {value:text.slice(start,index).trim(),end:index};
   depth--;
  }
 }
 throw Error('Unterminated window argument');
}
const pairingParams=[];
for(const match of pairingOpen[2].matchAll(/searchParams\.set\("([\w-]+)",/g))
 pairingParams.push({name:match[1],value:argumentAfter(pairingOpen[2],match.index+match[0].length).value});
for(const expected of ['containerId','displayMode','allMasters'])
 if(!pairingParams.some(entry=>entry.name===expected))throw Error(`Missing pairing parameter ${expected}`);
const pairingRoute=/new URL\(`([^`]+)`/.exec(pairingSource.slice(pairingOpen.index,pairingOpen.index+220));

// --- Named application windows (Dashboard module 54420) ----------------------
// Minified single-letter symbols are reused by unrelated modules, so every
// `alias.SYM` reference is resolved inside the module that exports it: module
// 69937 (window names) and 54693 (module-box names), both in the main chunk.
const main_chunk='.ref/applications/synapse/dashboard/static/js/main.01550b17.js';
const mainBytes=read(main_chunk),mainSource=mainBytes.toString('utf8');
const moduleCache=new Map();
function findModule(id){
 if(moduleCache.has(id))return moduleCache.get(id);
 for(const [file,source,bytes] of [[app,appSource,appBytes],[main_chunk,mainSource,mainBytes]]){
  let body=null;
  walk(acorn.parse(source,{ecmaVersion:'latest'}),node=>{
   if(body)return;
   if(node.type==='Property'&&String(node.key.value??node.key.name)===String(id)&&/Function/.test(node.value.type))body=node.value;
  });
  if(!body)continue;
  const statements=body.body.type==='BlockStatement'?body.body.body:[];
  const aliases={};
  for(const statement of statements){
   if(statement.type!=='VariableDeclaration')continue;
   for(const declarator of statement.declarations){
    const call=declarator.init;
    if(call?.type!=='CallExpression'||call.arguments.length!==1)continue;
    if(call.arguments[0].type!=='Literal')continue;
    aliases[declarator.id.name]={module:String(call.arguments[0].value)};
   }
  }
  const result={id:String(id),file,bytes,source,start:body.start,end:body.end,statements,aliases};
  moduleCache.set(id,result);
  return result;
 }
 throw Error(`Missing module ${id} in the Dashboard chunks`);
}
function exportsOf(module){const map=new Map();
 for(const statement of module.statements){
  if(statement.type!=='ExpressionStatement')continue;
  for(const argument of statement.expression.arguments??[]){
   if(argument?.type!=='ObjectExpression')continue;
   for(const property of argument.properties??[]){
    if(property.type!=='Property')continue;
    const value=property.value;
    if(value?.type==='ArrowFunctionExpression'&&value.body?.type==='Identifier')
     map.set(String(property.key.name??property.key.value),value.body.name);
   }
  }
 }
 return map;
}
// A string constant is either a module-level `NAME="value"` or an object field
// (`hz={CWCCW:12}` style tables are not window names, so only strings count).
function stringConstant(module,local){
 for(const statement of module.statements){
  if(statement.type!=='VariableDeclaration')continue;
  for(const declarator of statement.declarations){
   if(declarator.id?.name!==local)continue;
   if(declarator.init?.type==='Literal'&&typeof declarator.init.value==='string')
    return {value:declarator.init.value,offset:declarator.init.start,end:declarator.init.end};
   if(declarator.init?.type==='ObjectExpression'){
    const fields={};
    for(const property of declarator.init.properties??[])
     if(property.value?.type==='Literal'&&typeof property.value.value==='string')
      fields[String(property.key.name??property.key.value)]={value:property.value.value,offset:property.value.start,end:property.value.end};
    if(Object.keys(fields).length)return {object:declarator.id.name,fields};
   }
  }
 }
 return null;
}
function exportedText(moduleId,symbol){
 const module=findModule(moduleId);
 const local=exportsOf(module).get(symbol);
 if(!local)throw Error(`Module ${moduleId} does not export ${symbol}`);
 const constant=stringConstant(module,local);
 if(!constant)throw Error(`Symbol ${symbol} in module ${moduleId} is not a string`);
 if(constant.object)return {value:constant.object,symbol,local,module:String(moduleId),path:module.file,object:true,
  fields:Object.fromEntries(Object.entries(constant.fields).map(([name,field])=>[name,field.value]))};
 return {value:constant.value,symbol,local,module:String(moduleId),path:module.file,
  offset:constant.offset,end:constant.end,source:module.source.slice(constant.offset,constant.end)};
}
function referenceText(source,alias,expression){
 const member=/^([\w$]+)\.([\w$]+)$/.exec(expression);
 if(!member||member[1]!==alias)throw Error(`Unsupported window reference ${expression}`);
 return exportedText(69937,member[2]);
}
function literalOrReference(node,source,alias){
 if(node.type==='Literal')return {value:String(node.value),literal:true,offset:node.start,end:node.end};
 if(node.type==='TemplateLiteral'&&node.expressions.length===0)return {value:node.quasis[0].value.cooked,literal:true,offset:node.start,end:node.end};
 const text=source.slice(node.start,node.end);
 const resolved=referenceText(source,alias,text);
 return {value:resolved.value,literal:false,symbol:text,offset:node.start,end:node.end};
}
const registry=findModule(54420);
const registryAlias=Object.entries(registry.aliases).find(([,entry])=>entry.module==='69937')?.[0];
if(!registryAlias)throw Error('Module 54420 does not import the window-name module');
const windows=new Map();
for(const statement of registry.statements){
 if(statement.type!=='VariableDeclaration')continue;
 for(const declarator of statement.declarations){
  const init=declarator.init;
  if(init?.type!=='ObjectExpression')continue;
  const fields={};
  for(const property of init.properties??[]){
   const name=String(property.key.name??property.key.value);
   if(!['windowName','url','openParam'].includes(name))continue;
   if(name==='windowName')fields.windowName=literalOrReference(property.value,appSource,registryAlias);
   else if(property.value.type==='Literal')fields[name]={value:String(property.value.value),literal:true,offset:property.value.start,end:property.value.end};
   else if(property.value.type==='BinaryExpression'&&property.value.left.type==='Literal')
    fields[name]={value:`${property.value.left.value}+window.location.pathname`,literal:false,offset:property.value.start,end:property.value.end};
   else fields[name]={value:appSource.slice(property.value.start,property.value.end),literal:false,offset:property.value.start,end:property.value.end};
  }
  if(!fields.windowName||!fields.url||!fields.openParam)continue;
  windows.set(declarator.id.name,{symbol:declarator.id.name,window_name:fields.windowName,url:fields.url.value,
   open_param:fields.openParam.value,
   receipt:{path:app,sha256:hash(appBytes),offset:init.start,end:init.end,source:appSource.slice(init.start,init.end)}});
 }
}
if(windows.size<8)throw Error(`Expected the Dashboard window registry, found ${windows.size}`);
// Grouped lists name the hidden windows, the module windows and every window.
// A list entry is either a window variable, another list (spread) or an inline
// object literal; lists may reference each other, so resolution recurses.
function listEntries(source,node,alias){
 if(node.type==='Identifier')return [windows.has(node.name)?{window:node.name}:{group:node.name}];
 if(node.type==='ArrayExpression')return node.elements.flatMap(element=>listEntries(source,element,alias));
 if(node.type==='SpreadElement')return listEntries(source,node.argument,alias);
 if(node.type==='ObjectExpression'){
  const field=node.properties.find(property=>String(property.key.name??property.key.value)==='windowName');
  return field?[{inline:literalOrReference(field.value,source,alias).value}]:[];
 }
 return [];
}
const rawGroups={};
for(const statement of registry.statements){
 if(statement.type!=='VariableDeclaration')continue;
 for(const declarator of statement.declarations){
  const init=declarator.init;
  if(init?.type!=='ArrayExpression')continue;
  rawGroups[declarator.id.name]=listEntries(appSource,init,registryAlias);
 }
}
function resolveList(name,seen=[]){
 if(seen.includes(name))throw Error(`Window list cycle at ${name}`);
 const entries=rawGroups[name];
 if(!entries)throw Error(`Missing window list ${name}`);
 const names=[];
 for(const entry of entries){
  if(entry.window)names.push(windows.get(entry.window).window_name.value);
  else if(entry.inline)names.push(entry.inline);
  else names.push(...resolveList(entry.group,[...seen,name]));
 }
 return names;
}
const groups={};
for(const name of Object.keys(rawGroups))groups[name]=resolveList(name);
// Module boxes focus an existing window: box names come from module 54693.
const showModulesAt=pairingSource.indexOf('this.showModules=()=>{switch(this.props.box.name){');
if(showModulesAt<0)throw Error('Missing module box switch');
const showModules=pairingSource.slice(showModulesAt,showModulesAt+520);
const boxFace=pairingSource.slice(0,showModulesAt);
const boxAlias=Object.entries((()=>{const found={};
 for(const match of boxFace.matchAll(/(?:^|[,;])([\w$]+)=([\w$]+)\((\d+)\)/g))if(!(match[1] in found))found[match[1]]=match[3];
 return found;})()).find(([,id])=>id==='54693')?.[0];
const windowAlias=Object.entries((()=>{const found={};
 for(const match of boxFace.matchAll(/(?:^|[,;])([\w$]+)=([\w$]+)\((\d+)\)/g))if(!(match[1] in found))found[match[1]]=match[3];
 return found;})()).find(([,id])=>id==='69937')?.[0];
if(!boxAlias||!windowAlias)throw Error('Module box switch does not import the name modules');
// Fall-through labels share one `focusTab` target, so each label is read with
// the text up to the next label and the pending label list is carried forward.
const boxFocus=[];
const switchBody=showModules.slice(showModules.indexOf('switch'));
const labels=[...switchBody.matchAll(/case\s*(?:"([\w$]+)"|([\w$]+)\.([\w$]+)):/g)];
let pending=[];
for(let index=0;index<labels.length;index++){
 const label=labels[index];
 const chunk=switchBody.slice(label.index+label[0].length,labels[index+1]?.index??switchBody.length);
 const boxName=label[1]??exportedText(54693,label[3]).value;
 const boxSymbol=label[1]?`"${label[1]}"`:`f.${label[3]}`;
 const target=/return this\.focusTab\((?:(`\$\{([\w$]+)\.([\w$]+)\}`)|"([\w$-]+)")\)/.exec(chunk);
 if(!target){pending.push({boxSymbol,boxName});continue;}
 const windowName=target[3]?exportedText(69937,target[3]).value:target[4];
 const windowSymbol=target[3]?`${target[2]}.${target[3]}`:`"${target[4]}"`;
 for(const entry of [...pending,{boxSymbol,boxName}]){
  const resolved=entry.boxSymbol.startsWith('f.')?exportedText(54693,entry.boxSymbol.slice(2)).value:entry.boxName;
  boxFocus.push({box_symbol:entry.boxSymbol,box_name:resolved,window_symbol:windowSymbol,window_name:windowName});
 }
 pending=[];
}
if(boxFocus.length<6)throw Error(`Expected module box focus mapping, found ${boxFocus.length}`);
const namedWindows={registry_module:'54420',window_module:'69937',box_name_module:'54693',
 entries:[...windows.values()],groups,
 hidden_windows:groups.h??[],module_windows:groups.S??[],all_windows:groups.v??[],
 box_focus:boxFocus,
 name_receipts:{window_names:exportedText(69937,'BDG'),introduction:{value:exportedText(69937,'SCf').value},
  box_names:boxFocus.map(entry=>entry.box_name)}};
const result={schema_version:1,generator_sha256:hash(fs.readFileSync(__filename)),
 method:'Acorn receipts for Dashboard module 84058 (flags, window name, open function) and a text receipt for the multi-device pairing URL construction. No downloaded JavaScript is executed.',
 window_flags:flags,
 window_name:{patterns:names,source:{path:app,sha256:hash(appBytes),offset:nameNode.start,end:nameNode.end,source:nameSource}},
 window_open:{reuses_existing:openSource.includes('yH'),visibility_flag:'browser_visible',source:{path:app,sha256:hash(appBytes),offset:openNode.start,end:openNode.end,source:openSource}},
 multi_device_pairing:{route:pairingRoute?.[1]??null,parameters:pairingParams,
  source:{path:pairing,sha256:hash(pairingBytes),offset:pairingOpen.index,end:pairingOpen.index+pairingOpen[0].length,source:pairingOpen[0]}},
 display_modes:{note:'Branch names and per-mode URL parameters come from tools/audit-display-modes.cjs.',
  parameters:{macro:'macro=<id>',chromaApp:'serialNumber=<serial>',armory:'serialNumber=<serial>',multiDevicePairing:'containerId, allMasters, productId/pid, category, canPairTwoDevices'}},
 named_windows:namedWindows};
const json=JSON.stringify(result,null,2)+'\n';
const lines=['# 当前窗口打开契约','',
 'Dashboard 通过 `window.open` 打开具名页面；宿主根据策略选择已有窗口内的页签或新的系统窗口。不能仅根据 `windowName` 推断它是第二个系统窗口。下表来自当前 Dashboard 包（`'+app+'`，SHA-256 `'+hash(appBytes).slice(0,16)+'…`），由 `tools/extract-window-contract.cjs` 静态提取，没有执行下载的 JavaScript。策略的实际执行链见 [宿主策略审计](host-window-policy-current-audit.md)。','',
 '## 窗口标志','','| 标志 | 值 | 含义 |','| --- | --- | --- |'];
const meaning={sameWindow:'复用当前窗口',diffWindow:'独立窗口',diffWindowSingleProcess:'独立窗口（单进程）',tabVisible:'标签可见',tabInvisible:'标签隐藏',windowVisible:'窗口可见',windowInvisible:'窗口隐藏',autoFocus:'打开后聚焦',chromaIcon:'Chroma 窗口图标',synapseIcon:'Synapse 窗口图标',commonIcon:'通用图标',streamerCompanionIcon:'Streamer Companion 图标',virtualRingIcon:'虚拟环形灯图标'};
for(const [flag,value] of Object.entries(flags))lines.push(`| \`${flag}\` | \`${value}\` | ${meaning[flag]??''} |`);
lines.push('','## 窗口名','',
 '窗口名由容器或产品与序列号决定，同一设备重复打开会命中同一个窗口：','',
 ...names.map(pattern=>`- \`${pattern}\``),'',
 '打开函数先查询该名字的窗口是否已存在：存在则复用并调整可见性／子页，否则 `window.open(url, name, flags)`。','',
 '## 多设备配对窗口','',
 `路由 \`${result.multi_device_pairing.route??''}\`，参数：`,'',
 '| 参数 | 取值表达式 |','| --- | --- |',
 ...pairingParams.map(entry=>`| \`${entry.name}\` | \`${entry.value}\` |`),'',
 '## 具名应用窗口','',
 'Dashboard 模块 54420 用同一张表登记所有具名窗口（窗口名符号在模块 69937 里定义，模块盒名在模块 54693 里定义）：','',
 '| 变量 | 窗口名 | 地址 | 打开标志 |','| --- | --- | --- | --- |',
 ...namedWindows.entries.map(entry=>`| \`${entry.symbol}\` | \`${entry.window_name.value}\` | \`${entry.url}\` | \`${entry.open_param}\` |`),'',
 `隐藏窗口（\`browser_visible=0\`）：${namedWindows.hidden_windows.map(name=>`\`${name}\``).join('、')}。`,'',
 `模块窗口（安装后才打开的独立应用窗口）：${namedWindows.module_windows.map(name=>`\`${name}\``).join('、')}。`,'',
 '## 模块盒点击去向','',
 '模块列表里每个盒子点击后聚焦同名窗口；未安装时先打开安装器，再自动打开窗口：','',
 '| 盒名 | 窗口名 |','| --- | --- |',
 ...namedWindows.box_focus.map(entry=>`| \`${entry.box_name}\` | \`${entry.window_name}\` |`),'',
 '## 各 displayMode 的参数','','| 模式 | 参数 |','| --- | --- |',
 ...Object.entries(result.display_modes.parameters).map(([mode,value])=>`| \`${mode}\` | ${value} |`),'',
 '## 对本仓库的要求','',
 '- `policy=3` 模块使用宿主具名页签；确需第二窗口的策略使用系统窗口。两种情形均先按名字查找并聚焦已有页面。多设备配对保留历史明确要求的第二窗口，这是偏离其源 `policy=3` 调用的本地例外。',
 '- 窗口标志决定可见性与聚焦：`sameWindow` 表示复用当前窗口，`diffWindow` 才开新窗口。',
 '- 图标按模式区分（`chromaApp` 用 Chroma 图标），标题与 favicon 由该根自己设置。',
 '- 关闭语义也来自根：例如多设备配对窗口会在配对对象窗口关闭后自行关闭。','',
 '逐字段收据见 [机器可读契约](display-window-contract.json)。','',
 '重新生成：`node tools/extract-window-contract.cjs`；校验：`node tools/extract-window-contract.cjs --check`。','');
const md=lines.join('\n');
const jsonTarget=path.join(root,'docs/re/display-window-contract.json'),mdTarget=path.join(root,'docs/re/display-window-contract.md');
if(process.argv.includes('--check')){
 if(fs.readFileSync(jsonTarget,'utf8')!==json)throw Error('Stale window contract JSON');
 if(fs.readFileSync(mdTarget,'utf8')!==md)throw Error('Stale window contract report');
}else{
 fs.writeFileSync(jsonTarget,json);
 fs.writeFileSync(mdTarget,md);
}
console.log(`Extracted ${Object.keys(flags).length} window flags, ${names.length} name patterns, ${pairingParams.length} pairing parameters.`);
