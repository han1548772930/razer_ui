// Static semantic receipts: parse current vendor source as data; never execute it.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), tray = new Source('systray/systrayv2');
const read = file => fs.readFileSync(path.join(root,file),'utf8');
const digest = file => hash(fs.readFileSync(path.join(root,file)));
const receipts = [], inputs = new Map();
const assert = (value, message) => {if(!value) throw Error(message);};
function record(id,file,node,purpose) {
  assert(!receipts.some(r=>r.id===id),`Duplicate receipt ${id}`);
  inputs.set(file,digest(file));
  receipts.push({id,path:file,sha256:digest(file),start:node.start,end:node.end,
    source:read(file).slice(node.start,node.end),purpose});
  return receipts[receipts.length-1].source;
}
function binding(module,name,purpose) {
  return record(`tray:${module}:${name}`,tray.module(module).file,tray.binding(module,name),purpose);
}
function select(ast,predicate,label) {
  const found=[];walk(ast,n=>{if(predicate(n))found.push(n);});
  assert(found.length===1,`${label}: expected one node, got ${found.length}`);return found[0];
}
for(const [module,names,purpose] of [
  [2554,['C','T','W','F','ae','oe','se','he','Re','Oe','me','_e','Ne','Se','Je','Ze','qe','Qe','et'],'Account, tabs, catalog gate, notification state, host events and session effects'],
  [597,['a','c','s','d'],'Monitor alignment, resize requests, actual no-op _f and body lookup st'],
  [9001,['h','f','v','S','g'],'App eligibility, catalog refresh, launcher projection, launch and close'],
  [7660,['o'],'er: parses getWindowServiceClients JSON once and returns an array'],
  [5492,['j','x','Y','b','y','f','D'],'Connected device query/dedup/filter, preferences, subscription, profiles and battery DOM'],
  [219,['f','l','c','i'],'Surround visible branches, broadcast callbacks and thunk creators'],
  [3190,['x','u','m','g','f','v'],'Spatial visible branches, dropdown, broadcasts and thunk creators'],
  [2215,['y','m','p','b'],'Chroma visible branches, hidden widgets, running/hibernated and launch skeleton'],
  [8017,['w'],'Chroma Studio per-user profiles and local currentProfile update'],
  [4182,['y','g','v'],'Visualizer per-user state, menu labels and local state update'],
  [5586,['p','u','c'],'Studio/Visualizer installed-module launch hook and Chroma sub-tab map'],
  [1899,['g','p'],'Chroma Connect memory query, subscription, actual devices-only DOM and action queue'],
  [2033,['E','h','x'],'Cortex games/cover fallback, boost/restore, hidden widgets and thunk creators'],
  [3482,['c'],'Gold/silver hidden fields, pending balance and online account click']
]) for(const name of names)binding(module,name,purpose);
const bridgeNames=['callElectronAction','setBrowserVisible','getWindowStatus','activateWindowServiceClient',
  'getWindowStorageItem','setWindowStorageItem','registerWindowStorageEvent','unRegisterWindowStorageEvent',
  'getMemoryStorageItem','setMemoryStorageItem','registerMemoryStorageEvent','unRegisterMemoryStorageEvent'];
for(const name of bridgeNames) {
  const node=select(tray.module(7217).fn,n=>n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'
    &&key(n.left.property)===name,`bridge ${name}`);
  record(`bridge:${name}`,tray.module(7217).file,node,'Current Electron/fallback branch; original API name and payload');
}
for(const name of ['getUserNotifications','updateUserNotificationStatus','clearUserNotifications','launchSettings','broadcastMessage']) {
  const scope=tray.module(3839),node=select(scope.fn,n=>(n.type==='CallExpression'&&n.arguments.length===3
    &&n.arguments[1]?.value===name)||(n.type==='ObjectExpression'
      &&n.properties.some(p=>key(p.key)==='key'&&p.value.value===name)
      &&n.properties.some(p=>key(p.key)==='value')),`RazerApp ${name}`);
  record(`RazerApp:${name}`,scope.file,node,'Public method delegation only; not proof of downstream device success');
}
const host='.ref/host-4.0.827/electron/',mainFile=host+'main.js',mainAst=acorn.parse(read(mainFile),{ecmaVersion:'latest'});
for(const name of ['zn','qn','Hn','xn']) {
  const node=select(mainAst,n=>(n.type==='FunctionDeclaration'&&n.id.name===name)
    ||(n.type==='VariableDeclarator'&&n.id.name===name),`host ${name}`);
  record(`host:${name}`,mainFile,node,'Current right-menu creation / login / logout / named app activation');
}
for(const action of ['SET_BOUNDS','SET_BROWSER_VISIBLE','GET_WINDOW_SERVICE_CLIENTS','ACTIVE_WINDOW_SERVICE_CLIENT',
  'getWindowStatus','getWindowStorageItem','setWindowStorageItem',
  'registerWindowStorageEvent','unRegisterWindowStorageEvent','getMemoryStorageItem','setMemoryStorageItem',
  'registerMemoryStorageEvent','unRegisterMemoryStorageEvent']) {
  let matches=[];
  walk(mainAst,n=>{if(n.type==='SwitchStatement')n.cases.forEach((c,index)=>{
    if((c.test?.value??key(c.test?.property))===action)matches.push({node:c,delegate:c.consequent.length?null:n.cases.slice(index+1).find(c=>c.consequent.length)});
  });});
  assert(matches.length===1,`Host action ${action}: ${matches.length}`);
  record(`host:action:${action}`,mainFile,matches[0].node,'IPC dispatch label; stacked labels include the exact shared delegate');
  if(matches[0].delegate)record(`host:delegate:${action}`,mainFile,matches[0].delegate,'First following nonempty consequent; exact storage/focus implementation');
}
const constantsFile=host+'constants.js',constantsAst=acorn.parse(read(constantsFile),{ecmaVersion:'latest'});
for(const name of ['ACTIVE_WINDOW_SERVICE_CLIENT','GET_WINDOW_SERVICE_CLIENTS','SET_BOUNDS','SET_BROWSER_VISIBLE']) {
  const node=select(constantsAst,n=>n.type==='Property'&&key(n.key)===name,`Host action constant ${name}`);
  record(`host:constant:${name}`,constantsFile,node,'Exact bridge string to host enum mapping; not an inferred normalization layer');
}
const leftFile=host+'components/Tab/LeftSystray.js',leftAst=acorn.parse(read(leftFile),{ecmaVersion:'latest'});
const left=select(leftAst,n=>n.type==='ClassExpression','LeftSystray class');
for(const node of left.body.body)record(`LeftSystray:${key(node.key)}`,leftFile,node,
  'Actual host window lifecycle, click timers, alignment, load failure and login fallback');
const preloadFile=host+'preload.js',preloadAst=acorn.parse(read(preloadFile),{ecmaVersion:'latest'});
for(const name of ['doElectronAction','getWindowServiceClients','activateWindowServiceClient','setBounds','setBrowserVisible']) {
  const node=select(preloadAst,n=>n.type==='Property'&&key(n.key)===name,`preload ${name}`);
  record(`preload:${name}`,preloadFile,node,'Current preload IPC wrapper; getWindowServiceClients serializes the result');
}
for(const name of ['window_storage','memory_storage']) {
  const file=host+`modules/${name}/index.js`,text=read(file);
  record(`host:${name}`,file,{start:0,end:text.length},'Current storage scope, envelope, subscriptions and event delivery; not a native DLL');
}
const manifestFile=tray.directory+'/asset-manifest.json',manifest=JSON.parse(read(manifestFile));inputs.set(manifestFile,digest(manifestFile));
const css=[];
for(const rel of [...new Set(Object.values(manifest.files))].filter(p=>/\.css$/.test(p))) {
  const file=tray.directory+'/'+rel.replace(/^\.\//,''),text=read(file);inputs.set(file,digest(file));
  const rules=parseCSS(text).filter(r=>/systray|navbar|tabs|notifications|launcher|app-section|app-not-running|app-version|surround-sound|spatial-audio|synapse|devices|battery|chroma|visualizer|cortex|gold-and-silver|body|html|tooltip|dropdown|switch|btn|hover|disabled/.test(r.selector)
    ||r.selector==='body'||r.selector==='html'||r.selector==='*');
  css.push({path:file,sha256:digest(file),rules,
    font_faces:[...text.matchAll(/@font-face\{[^}]+\}/g)].map(m=>({start:m.index,end:m.index+m[0].length,source:m[0]})),
    interpretation:'Static selector/declaration receipts. Nested selectors, specificity, inherited values, invalid CSS and actual browser layout are not computed.'});
}
const lookup=id=>{const r=receipts.find(r=>r.id===id);assert(r,`Missing ${id}`);return r.source;};
assert(lookup('tray:2554:Re').includes('visibility:a?"visible":"none"'),'visibility anomaly changed');
assert(lookup('tray:2554:Oe').includes('launchSettings({section:"notifications"})')&&lookup('tray:2554:Oe').includes('payload:"widgets"'),'Empty Widgets command changed');
assert(lookup('tray:2554:me').includes('apps-${n.length}')&&lookup('tray:2554:me').includes('children:a.map'),'Filtered launcher count semantics changed');
assert(lookup('tray:2554:Ne').includes('isSame(i,"m")')&&lookup('tray:2554:Ne').includes('HH:DDA'),'Notification formatting changed');
assert(lookup('tray:7660:o').includes('e(JSON.parse(t))')&&lookup('tray:5492:y').includes('JSON.parse(e).find'),'Device activation parse boundary changed');
assert(lookup('tray:5492:x').includes('showBatteryWidget:!C.includes')&&!lookup('tray:5492:y').includes('showBatteryWidget'),'Battery-hidden preference changed');
assert(lookup('tray:5492:y').includes('battery-${D(r.level)} ${'),'Paused battery class changed');
assert(lookup('tray:219:l').includes('=>n=>n(')&&lookup('tray:219:f').includes('case"surroundSoundOnChange":l(a)'),'Surround thunk callback changed');
assert(lookup('tray:3190:u').includes('=>t=>t(')&&lookup('tray:3190:x').includes('case"spatialAudioOnChange":u(i)'),'Spatial thunk callback changed');
assert(lookup('tray:2033:h').includes('=>async t=>')&&lookup('tray:2033:E').includes('case"cortexGamesChange":h(a)'),'Cortex thunk callback changed');
const contracts=[
  {id:'host-lifecycle',receipts:['LeftSystray:createSystrayWindow','LeftSystray:addHandler','LeftSystray:sendClickEvent','LeftSystray:sendDoubleClickEvent','LeftSystray:handleBlur','LeftSystray:createLoginPage','LeftSystray:calculateWindowPosition','tray:2554:Je','tray:2554:Ze','tray:597:c','tray:597:a'],
    conclusion:'Windows host creates an initially invisible 300×200 policy-6 panel; 200ms single-click waits, double-click cancels both timers, blur hides after 300ms. Renderer queries actual window status and toggles it, requests installed engine state and independently aligns/resizes. Current resize selector .app.list-unstyled does not match rendered .apps. Account body height is children total +153 clamped 400–700; no-account request is 60. Load failure opens the official login URL, not a fake guest.'},
  {id:'root-state',receipts:['tray:2554:C','tray:2554:T','tray:2554:W','tray:2554:F','tray:2554:ae','tray:2554:oe','tray:2554:se','tray:2554:he','tray:2554:Re','tray:2554:Oe','tray:2554:me'],
    conclusion:'Empty initial account/catalog. Account id gates tabs/body. Guest/header click logs out; authenticated click views online profile with busy state. Tabs show total notification count, not unread count. Launcher count styling/title/tooltip uses unfiltered count. Empty Widgets invokes notifications section then widgets scroll broadcast. visibility none is invalid CSS and the constant Widgets branch unconditionally snapshots body height.'},
  {id:'catalog-launch',receipts:['tray:9001:h','tray:9001:f','tray:9001:v','tray:9001:S','tray:9001:g','tray:2554:qe','tray:2554:Qe','tray:2554:Je'],
    conclusion:'installedModules plus isAppInstalled gates app catalog; source sort/hidden settings project widget entries, running-client objects and hibernation. Launchers use saved array or five title-sorted app defaults. Launch hides panel, distinguishes website auth, running restore, compatibility and browser-window creation, and queues behind login. Double-click launch requires logged-in state and absent login window. Session/notification/balance observations are original host/account paths, not device DLL reads.'},
  {id:'notifications',receipts:['tray:2554:_e','tray:2554:Ne','tray:2554:Se','tray:2554:F','RazerApp:getUserNotifications','RazerApp:updateUserNotificationStatus','RazerApp:clearUserNotifications'],
    conclusion:'Fetch rejection logs and keeps pending state; success sorts newest startDate first. Card opens URL before awaiting a mark-read success. Clear does not await the remote call before local items reset. Empty and pending render separate branches. Date source uses HH:DDA and moment minute-unit m; do not rewrite as a conventional month formatter.'},
  {id:'widget-gates',receipts:['tray:2554:Oe','tray:219:f','tray:3190:x','tray:5492:Y','tray:2215:y','tray:2215:m','tray:2215:p','tray:2033:E','tray:3482:c'],
    conclusion:'Six app widget branches: Surround, Spatial, Synapse, Chroma, Cortex, Gold. Spatial suppresses Surround. Synapse/Chroma additionally require installed engine entries. Guest suppresses Gold. Running/hibernated checks differ per widget; preferences can hide whole widgets or particular fields. Unstarted widget skeletons are source launch prompts, not device observations.'},
  {id:'synapse-data',receipts:['tray:5492:j','tray:5492:x','tray:5492:Y','tray:5492:b','bridge:getWindowStorageItem','bridge:registerWindowStorageEvent','bridge:unRegisterWindowStorageEvent','host:window_storage'],
    conclusion:'connectedDevices window-storage envelopes contain JSON values. Current code suppresses same-PID off duplicates, deduplicates productId/primaryPid, retains serial/primary replacement rules, requires ready+serial and removes missing-power duplicates. Hidden/container filtering precedes subdevice flattening. dashboardDevices further filters current observations. Sort/battery preference events reproject cached observations. Row height counts before mixer-failure filtering. These receipts prove the consumer and host store, not every publisher or DLL query.'},
  {id:'synapse-actions',receipts:['tray:5492:y','tray:7660:o','bridge:activateWindowServiceClient','host:action:ACTIVE_WINDOW_SERVICE_CLIENT','host:delegate:ACTIVE_WINDOW_SERVICE_CLIENT'],
    conclusion:'Rows activate named UI or hibernated Synapse, broadcast device focus with monitor/IoT aliases, and send two profile switch messages over MWWindowName BroadcastChannel. No acknowledgement is awaited. Dynamic Loupedeck profiles disable selector; Auto profile click bubbles to row. Non-hibernated pairing-window cleanup parses an already parsed client array, so successful cleanup is not established.'},
  {id:'battery',receipts:['tray:5492:x','tray:5492:y','tray:5492:D'],
    conclusion:'hasBattery, defined level and isBatterySupported gate battery DOM. Incorrect/external/showBatteryValue=false hides label only; non-off start and off-to-on wait 2000ms with -%. <=10 non-Charging is low-battery, Charging 100 has separate icon. showBatteryWidget from hidden prefs is not consumed by row. Paused class inserts a space before -paused and does not match concatenated CSS paused variants.'},
  {id:'surround-spatial-cortex-observation',receipts:['tray:219:f','tray:219:l','tray:219:c','tray:219:i','tray:3190:x','tray:3190:u','tray:3190:m','tray:3190:g','tray:3190:f','tray:3190:v','tray:2033:E','tray:2033:h','tray:2033:x'],
    conclusion:'Mount/appLaunch sends original state queries and registers named broadcast responses. Original callbacks call thunk creators without dispatching or invoking the returned thunk. Connected action props are not used there; this source alone does not prove Redux state changes on broadcast or Cortex image-error callback. Toggle/profile/game/boost commands are broadcasts, with source visibility gates retained.'},
  {id:'chroma',receipts:['tray:2215:m','tray:8017:w','tray:4182:y','tray:4182:g','tray:5586:p','tray:5586:u','tray:5586:c','tray:1899:g','tray:1899:p','host:memory_storage'],
    conclusion:'Studio selects current user profiles and updates local currentProfile; Visualizer selects current user visualizerState and updates renderer storage. Their launch hook reads synapseInstalledModules, focuses an existing named module or Chroma sub-tab and optionally closes pairing windows using an already parsed array correctly. Connect queries both devices/apps memory values but renders only connectedDevices, hiding named rows and mapping enable/running to off/disconnected/connected. Its switch submits setAppEnabled JSON to the respective memory action key. Consumer-to-DLL write acknowledgement is not proved here.'},
  {id:'right-menu',receipts:['host:zn','host:qn','host:Hn','host:xn'],
    conclusion:'Current host enumerates recognized User Data/Apps directories and excludes pending failed launches; original icons, label translations/beta fallback and separators retained. Account label uses logged-in non-Guest while handler uses logged-in flag. Exit choices depend on running known apps, with settings omitted from per-app Exit. Native menu creation and global login/logout broadcasts are separate from left renderer.'}
];
for(const c of contracts)for(const id of c.receipts)lookup(id);
const result={schema_version:1,scope:'Current left/right tray semantic read-through only; structural page graphs and local UI parity are separate.',
  method:'Acorn AST, CSS static parser and exact byte hashes; reference JS, apps, DLLs, builds and tests never executed.',
  offsets:'UTF-16 string indices, zero-based start inclusive/end exclusive; SHA-256 over file bytes.',
  source_inputs:Object.fromEntries([...inputs].sort(([a],[b])=>a.localeCompare(b))),receipts,css,semantic_contracts:contracts,
  unresolved:['All connectedDevices publisher-to-device query branches are not re-proved by this tray consumer audit.',
    'Audio/Cortex broadcast recipient implementations and Chroma memory-action consumers require their own complete downstream audit.',
    'Generic RazerApp notification method delegation is recorded; online service responses and account authenticity are not verified by static parsing.',
    'Styles are original source declarations, not a computed cascade, rendered viewport, font availability or screenshot proof.',
    'Current GPUI tray readback/session/catalog/actions must be independently compared; original-source anomalies are not local implementation fixes.']};
const output=path.join(root,'docs/re/tray-semantic-current-evidence.json'),json=JSON.stringify(result,null,2)+'\n';
if(process.argv.includes('--check'))assert(read('docs/re/tray-semantic-current-evidence.json')===json,'Tray semantic evidence drift');
else fs.writeFileSync(output,json);
console.log(JSON.stringify({receipts:receipts.length,contracts:contracts.length,css_files:css.length,css_rules:css.reduce((n,c)=>n+c.rules.length,0),inputs:inputs.size}));
