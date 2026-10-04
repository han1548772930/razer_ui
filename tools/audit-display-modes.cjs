// Audit every device root's displayMode branches without evaluating JavaScript.
// The current product bundles read `searchParams.get("displayMode")` and choose
// a root component from it; each branch is recorded with its own text window so
// the finding can be re-checked by hand. Nothing here runs downloaded code.
const fs=require('fs'),path=require('path'),crypto=require('crypto');
const root=path.resolve(__dirname,'..');
const hash=value=>crypto.createHash('sha256').update(value).digest('hex');
const read=relative=>fs.readFileSync(path.join(root,relative));
const registration=JSON.parse(read('docs/re/product-registration-audit.json').toString('utf8'));
const WINDOW=2600;
const escape=name=>name.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
// A branch is only accepted when the literal is compared against the variable
// that received the displayMode value, or against the getter itself.
function branchesFor(source,index){
 const start=Math.max(0,index-160),end=Math.min(source.length,index+WINDOW),window=source.slice(start,end);
 const variables=[...window.matchAll(/([A-Za-z_$][\w$]*)\s*=\s*[\w$.]*searchParams\.get\("displayMode"\)/g)].map(m=>m[1]);
 const branches=new Map();
 const record=(name,offset,kind)=>{
  const existing=branches.get(name);
  if(!existing||kind==='root')branches.set(name,{name,kind,offset:start+offset});
 };
 for(const variable of variables){
  const pattern=new RegExp(`"([A-Za-z][\\w]*)"\\s*===\\s*${escape(variable)}\\b|\\b${escape(variable)}\\s*===\\s*"([A-Za-z][\\w]*)"`,'g');
  for(const match of window.matchAll(pattern)){
   const name=match[1]??match[2],at=match.index;
   // Root branches select a component in the same ternary chain (`?...?(0,x.jsx)(`)
   // or return one directly (`if(...)return(0,x.jsx)(...)`); helper checks
   // (e.g. `isMacroDisplayMode`) only toggle a CSS class and stay in page.
   const tail=window.slice(at,at+300);
   const selects=/\?\s*\(\s*0\s*,\s*[\w$.]+\.(?:jsx|createElement)\)\(/.test(tail)
    ||/\?\s*(?:[\w$.]+\.)?(?:jsx|createElement)\(/.test(tail)
    ||/return\s*\(\s*0\s*,\s*[\w$.]+\.(?:jsx|createElement)\)\(/.test(tail);
   record(name,at,selects?'root':'inline');
  }
 }
 for(const match of window.matchAll(/"([A-Za-z][\w]*)"\s*===\s*[\w$.()"'\s]*searchParams\.get\("displayMode"\)/g)){
  record(match[1],match.index,'inline');
 }
 const params=[...new Set([...window.matchAll(/searchParams\.get\("([\w-]+)"\)/g)].map(m=>m[1]))].sort();
 const components=[...new Set([...window.matchAll(/\.jsx\)\(([A-Za-z_$][\w$]*)\s*,\s*\{/g)].map(m=>m[1]))].sort();
 return {start,end,variables,params,components,branches:[...branches.values()].sort((a,b)=>a.offset-b.offset)};
}
const products=[];
const rootCounts=new Map(),allCounts=new Map();
for(const product of registration.products){
 const navigation=product.navigation.find(entry=>entry.primary);
 const file=navigation.source,bytes=read(file),source=bytes.toString('utf8');
 const occurrences=[];
 for(const match of source.matchAll(/searchParams\.get\("displayMode"\)/g))occurrences.push({offset:match.index,...branchesFor(source,match.index)});
 const modes=new Map();
 for(const occurrence of occurrences)for(const branch of occurrence.branches){
  const current=modes.get(branch.name);
  if(!current||(current.kind!=='root'&&branch.kind==='root'))modes.set(branch.name,{mode:branch.name,kind:branch.kind,offset:branch.offset});
 }
 const listed=[...modes.values()].sort((a,b)=>a.mode.localeCompare(b.mode));
 for(const mode of listed){
  allCounts.set(mode.mode,(allCounts.get(mode.mode)??0)+1);
  if(mode.kind==='root')rootCounts.set(mode.mode,(rootCounts.get(mode.mode)??0)+1);
 }
 products.push({product_id:product.product_id,name:product.name,source:file,sha256:hash(bytes),
  display_mode_reads:occurrences.length,modes:listed,occurrences});
}
const registered={};
for(const product of registration.products)for(const navigation of product.navigation){
 if(navigation.primary)continue;
 const mode=navigation.display_mode;
 (registered[mode]??=[]).push(product.product_id);
}
const found=new Map(products.flatMap(product=>product.modes.map(mode=>[mode.mode,(mode.mode)])));
const registeredSummary={};
for(const [mode,ids] of Object.entries(registered)){
 const set=new Set(ids);
 const audited=products.filter(product=>product.modes.some(entry=>entry.mode===mode&&entry.kind==='root')).map(product=>product.product_id);
 registeredSummary[mode]={registered_navigations:ids.length,root_branches_audited:audited.length,
  registered_without_root_branch:[...set].filter(id=>!audited.includes(id)),
  root_branch_without_registration:audited.filter(id=>!set.has(id))};
}
const counts=object=>Object.fromEntries([...object.entries()].sort((a,b)=>b[1]-a[1]||a[0].localeCompare(b[0])));
// Who opens each mode in the current source. Negative results are recorded as
// such: `chromaApp` has no Dashboard opener at all, because that window belongs
// to the separate Chroma application.
const dashboardDir='.ref/applications/synapse/dashboard/static/js';
const dashboardFiles=fs.readdirSync(path.join(root,dashboardDir)).filter(name=>name.endsWith('.js'));
const dashboardBundles=dashboardFiles.map(name=>({name,source:fs.readFileSync(path.join(root,dashboardDir,name),'utf8')}));
const literalReceipt=(relative,literal)=>{
 const source=fs.readFileSync(path.join(root,relative),'utf8');
 const offset=source.indexOf(literal);
 if(offset<0)throw Error(`Missing consumer literal ${literal} in ${relative}`);
 return {path:relative,offset,end:offset+literal.length,sha256:hash(fs.readFileSync(path.join(root,relative))),excerpt:literal};
};
const countInDashboard=literal=>dashboardBundles.reduce((total,file)=>total+file.source.split(literal).length-1,0);
const macroChunk='.ref/applications/synapse/macro/static/js/1700.a2780a35.chunk.js';
const macroPopup=fs.readFileSync(path.join(root,macroChunk),'utf8');
const macroProductUrl='displayMode=macro&macro=${h}&containerId=${t.deviceContainerId}&deviceEditionInfo=${t.deviceEditionInfo}&serialNumber=${t.serialNumber}';
const hostBackground='.ref/host-4.0.827/source-evidence/background-current-source.js';
const consumers={
 macro:{opened_by:'macro application window',window_name:'macro',url:'/synapse/macro/',
  product_embed:'iframe in the keybind popup (title MouseBind)',
  evidence:[literalReceipt('.ref/applications/synapse/dashboard/static/js/App.72827d47.chunk.js','{windowName:s.BDG,url:"/synapse/macro/",openParam:"policy=3,tab_visible=1"}'),
   (()=>{const offset=macroPopup.indexOf(macroProductUrl);if(offset<0)throw Error('Missing macro product URL');return {path:macroChunk,offset,end:offset+macroProductUrl.length,sha256:hash(fs.readFileSync(path.join(root,macroChunk))),excerpt:macroProductUrl};})()]},
 armory:{opened_by:'armory application window',window_name:'armory',url:'/synapse/armory/',
  evidence:[literalReceipt('.ref/applications/synapse/dashboard/static/js/App.72827d47.chunk.js','{windowName:"armory",url:"/synapse/armory/",openParam:"policy=3,tab_visible=1"}')]},
 chromaApp:{opened_by:'Chroma application window (separate app)',window_name:'chroma-app',url:'/chroma-app/dashboard/',
  dashboard_opener_literal_count:countInDashboard('chromaApp'),
  evidence:[literalReceipt(hostBackground,'appName:"chroma-app"')]},
 multiDevicePairing:{opened_by:'Dashboard device box and the product-side pairing helper',window_name:'multi-device-pairing',
  url:'/synapse/products/<pid>/ui/index.html',
  evidence:[literalReceipt('.ref/applications/synapse/dashboard/static/js/7861.1b0e99a4.chunk.js','i.searchParams.set("displayMode","multiDevicePairing")')]}};
const result={schema_version:1,generator_sha256:hash(fs.readFileSync(__filename)),
 method:'Static text windows around `searchParams.get("displayMode")`; branch names are comparisons against the variable that received the value or against the getter itself. Root branches select a component in the same ternary chain. No downloaded JavaScript is executed.',
 window_chars:WINDOW,summary:{products_scanned:products.length,
  products_reading_display_mode:products.filter(product=>product.display_mode_reads>0).length,
  products_with_root_branches:products.filter(product=>product.modes.some(mode=>mode.kind==='root')).length,
  root_branch_counts:counts(rootCounts),all_branch_counts:counts(allCounts),
  registered_non_primary:registeredSummary,consumers},products};
const json=JSON.stringify(result,null,2)+'\n';
const lines=['# 当前 displayMode 分支审计','',
 '本报告按静态文本窗口统计每个设备根读取 `displayMode` 后选择的分支，用于确定独立根的真实范围。`root` 表示该分支在同一三元链里直接选择组件，`inline` 表示只切换类名或隐藏元素。没有执行任何下载的 JavaScript。','',
 `扫描 ${products.length} 个产品包；${result.summary.products_reading_display_mode} 个读取 \`displayMode\`；${result.summary.products_with_root_branches} 个存在根级分支。`,'',
 '## 根级分支计数','','| 模式 | 产品数 |','| --- | ---: |'];
for(const [mode,count] of Object.entries(result.summary.root_branch_counts))lines.push(`| \`${mode}\` | ${count} |`);
lines.push('','## 注册导航与根分支的差异','','| 模式 | 注册的非主导航 | 审计到的根分支 | 注册但无根分支 | 有根分支但未注册 |','| --- | ---: | ---: | --- | --- |');
for(const [mode,entry] of Object.entries(registeredSummary))lines.push(`| \`${mode}\` | ${entry.registered_navigations} | ${entry.root_branches_audited} | ${entry.registered_without_root_branch.join(', ')||'—'} | ${entry.root_branch_without_registration.length} 个 |`);
lines.push('','## 说明','',
 '- 计数是产品包个数，不是独立界面数；同一产品的多个分支共用同一份样式与状态。',
 '- `inline` 分支（例如 `chromaApp` 时隐藏设备图）不产生独立界面，但会改变默认根的呈现，实现默认界面时需要一并处理。',
 `- 只扫描注册审计记录的主导航源文件；${result.summary.products_scanned-result.summary.products_reading_display_mode} 个产品在该文件里没有读取 \`displayMode\`（根选择器可能位于懒加载分块），这些产品的分支需要单独核对。`,
 '## 各模式由谁打开','',
 '| 模式 | 打开者 | 窗口名 | 地址 |','| --- | --- | --- | --- |',
 ...Object.entries(consumers).map(([mode,entry])=>`| \`${mode}\` | ${entry.opened_by} | \`${entry.window_name}\` | \`${entry.url}\` |`),'',
 '- `macro` 不是独立产品窗口：它是宏应用（`/synapse/macro/` 窗口）在「绑定到设备」弹层里嵌入的 iframe，参数 `displayMode=macro&macro=<id>&containerId=…&deviceEditionInfo=…&serialNumber=…`，因此产品包的 `macro` 分支只在那个 iframe 里出现，产品根本身不会自己开窗。',
 `- \`chromaApp\` 在当前 Dashboard 包里出现 \`${consumers.chromaApp.dashboard_opener_literal_count}\` 次：Synapse 不打开这个模式，它属于独立的 Chroma 应用窗口。`,'',
 '本地 `macro`、`armory`、`profiles`、`alexa` 和 `feedback-synapse` 通过宿主具名页签打开。当前 4.0.827 的 `Tab.js` 将 `policy=3` 分派到已有窗口的标签栏，同名再次打开时聚焦已有页签；旧版文档把它解释成第二个 gpui 窗口的结论已撤回。多设备配对仍按历史明确要求保留第二窗口，这是本地例外，其源调用同样传 `sameWindow`。详情见 [宿主策略审计](host-window-policy-current-audit.md)。页面与设备服务仍按各自边界记录。','',
 '- 窗口名、可见性与聚焦标志、每个模式的 URL 参数见 [窗口打开契约](display-window-contract.md)。',
 '- 逐产品的分支、参数、引用组件与文本窗口范围见 [机器可读清单](display-mode-audit.json)。','',
 '重新生成：`node tools/audit-display-modes.cjs`；校验：`node tools/audit-display-modes.cjs --check`。','');
const md=lines.join('\n');
const jsonTarget=path.join(root,'docs/re/display-mode-audit.json'),mdTarget=path.join(root,'docs/re/display-mode-audit.md');
if(process.argv.includes('--check')){
 if(fs.readFileSync(jsonTarget,'utf8')!==json)throw Error('Stale displayMode audit JSON');
 if(fs.readFileSync(mdTarget,'utf8')!==md)throw Error('Stale displayMode audit report');
}else{
 fs.writeFileSync(jsonTarget,json);
 fs.writeFileSync(mdTarget,md);
}
console.log(JSON.stringify(result.summary.root_branch_counts));
console.log(JSON.stringify(registeredSummary));
