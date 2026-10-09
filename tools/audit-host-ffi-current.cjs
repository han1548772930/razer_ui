// Static AST receipts only. Never import or execute the reference host/dependencies.
const fs = require('fs'), path = require('path'), assert = require('assert');
const acorn = require('acorn');
const {hash, key} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const host = '.ref/host-4.0.827/';
const paths = {
  preload: 'electron/preload.js', main: 'electron/main.js',
  Main: 'electron/modules/ffi/FFIPreloadMain.js', Legacy: 'electron/modules/ffi/ffiMain.js',
  Manager: 'electron/modules/ffi_subprocess/index.js',
  Process: 'electron/modules/ffi_subprocess/FFIProcess.js',
  Sub: 'electron/modules/ffi_subprocess/FFIPreloadSubProcess.js',
  child: 'electron/modules/ffi_subprocess/_ffiprocess.js',
  route: 'electron/modules/ffi_subprocess/_ffiProcessRoute.js',
  Registry: 'electron/modules/dll_registry/index.js',
  Common: 'electron/lib/common.js',
  SysPlatform: 'electron/modules/sysutil/index.js',
  SysWin: 'electron/modules/sysutil/win/index.js',
  SysMac: 'electron/modules/sysutil/mac/index.js',
  Entry: 'node_modules/ffi-napi-rz/lib/ffi.js',
  FreeLibrary: 'node_modules/ffi-napi-rz/lib/freelibrary.js',
  Variadic: 'node_modules/ffi-napi-rz/lib/foreign_function_var.js',
  CIFVar: 'node_modules/ffi-napi-rz/lib/cif_var.js',
  Library: 'node_modules/ffi-napi-rz/lib/library.js',
  DynamicLibrary: 'node_modules/ffi-napi-rz/lib/dynamic_library.js',
  ForeignFunction: 'node_modules/ffi-napi-rz/lib/foreign_function.js',
  ForeignProxy: 'node_modules/ffi-napi-rz/lib/_foreign_function.js',
  Callback: 'node_modules/ffi-napi-rz/lib/callback.js',
  CIF: 'node_modules/ffi-napi-rz/lib/cif.js',
  Type: 'node_modules/ffi-napi-rz/lib/type.js',
  Bindings: 'node_modules/ffi-napi-rz/lib/bindings.js'
};
const files = [], anchors = [], cases = [], ipc = [], imports = [];
const sysutilDeclarationGroups = [];
const texts = new Map();
const bridgeNames = ['doDLLAction','doDLLActionAsync','doDLLMainAction',
  'doDLLMainActionAsync','doDLLSubProcessActionAsync'];
const channels = ['ffiPreload','ffiPreloadAsync','ffiSubPreloadAsync'];
for (const [alias, relative] of Object.entries(paths)) {
  const file = host + relative, bytes = fs.readFileSync(path.join(root, file));
  const source = bytes.toString('utf8'), sha256 = hash(bytes);
  texts.set(alias, source);
  const ast = acorn.parse(source, {ecmaVersion: 'latest', sourceType: 'script'});
  files.push({alias, path: file, sha256, bytes: bytes.length});
  const receipt = (name, node) => {
    const id = `${alias}:${name}:${node.start}`;
    if (!anchors.some(a => a.id === id)) anchors.push({id, alias, name,
      path: file, sha256, offset: node.start, end: node.end,
      source: source.slice(node.start, node.end)});
    return id;
  };
  function scan(node, parents = [], member = null) {
    if (!node?.type) return;
    if (alias === 'SysWin' && node.type === 'ArrayExpression' && node.elements.length > 1
        && node.elements.every(n => n?.type === 'ObjectExpression' && n.properties.every(p =>
          p.type === 'Property' && p.value.type === 'ArrayExpression'
          && typeof p.value.elements[0]?.value === 'string'
          && p.value.elements[1]?.type === 'ArrayExpression'))) {
      assert.equal(sysutilDeclarationGroups.length,0,'Ambiguous SysUtils fallback groups');
      node.elements.forEach((group,index) => sysutilDeclarationGroups.push({index,
        anchor:receipt(`declaration-group:${index}`,group),
        declarations:group.properties.map(p=>({name:key(p.key),return_type:p.value.elements[0].value,
          parameters:p.value.elements[1].elements.map(e=>e.value)}))}));
    }
    let owner = member;
    if (['MethodDefinition','PropertyDefinition'].includes(node.type)) {
      owner = key(node.key);
      receipt(owner, node);
    }
    if (!['main','preload','Common'].includes(alias)) {
      if (node.type === 'FunctionDeclaration') receipt(node.id.name, node);
      if (node.type === 'VariableDeclarator' && parents.length <= 2)
        receipt(source.slice(node.id.start, node.id.end), node);
      if (node.type === 'AssignmentExpression' &&
          (['FunctionExpression','ArrowFunctionExpression'].includes(node.right.type)
            || parents.length <= 2)) receipt(source.slice(node.left.start, node.left.end), node);
      if (node.type === 'SwitchCase') cases.push({alias, owner, action: node.test?.value ?? null,
        anchor: receipt(`case:${owner || 'nested'}:${node.test ? source.slice(node.test.start,node.test.end) : 'default'}`, node)});
    }
    if (alias === 'preload' && node.type === 'Property' && bridgeNames.includes(key(node.key)))
      receipt(key(node.key), node);
    if(alias === 'Common' && node.type === 'AssignmentExpression' && key(node.left.property)==='sleepWithWhileLoop')
      receipt('sleepWithWhileLoop',node);
    if (alias === 'main' && node.type === 'NewExpression' && node.arguments.some(n => n.type === 'Identifier' && n.name === 'Ve'))
      receipt('legacy-ffi-sysutil-constructor',node);
    if (node.type === 'CallExpression') {
      if (node.callee.type === 'Identifier' && node.callee.name === 'require') {
        const value = node.arguments[0]?.value;
        imports.push({alias, offset: node.start, end: node.end,
          target: typeof value === 'string' ? value : null,
          source: source.slice(node.start, node.end)});
        if (alias === 'main' && /ffi|dll_registry/.test(value || '')) {
          const decl = [...parents].reverse().find(p => p.type === 'VariableDeclarator');
          receipt(`load:${value}`, decl || node);
        }
      }
      if (['main','preload'].includes(alias) && channels.includes(node.arguments[0]?.value)) {
        const entry = {alias, channel: node.arguments[0].value,
          operation: key(node.callee.property), anchor: receipt(`ipc:${node.arguments[0].value}`,node)};
        ipc.push(entry);
      }
      if(alias === 'main' && node.callee.object?.name === 'io'
          && ['initDll','callDLL','callDLLSync','callDLLAsync'].includes(key(node.callee.property)))
        receipt(`sysutil-call:${key(node.callee.property)}:${node.start}`,node);
      if (alias === 'child' && key(node.callee.property) === 'on' && node.arguments[0]?.value === 'message')
        receipt(key(node.callee.object?.property) === 'parentPort' ? 'parent-message-handler' : 'port-message-handler', node);
      if (alias === 'child' && key(node.callee.property) === 'setEventCallback') receipt('event-forwarder', node);
    }
    for (const value of Object.values(node)) {
      if (Array.isArray(value)) value.forEach(child => scan(child, [...parents,node],owner));
      else if (value?.type) scan(value, [...parents,node],owner);
    }
  }
  scan(ast);
}
const find = (alias, name) => {
  const hits = anchors.filter(a => a.alias === alias && a.name === name);
  assert.equal(hits.length, 1, `Ambiguous/missing ${alias}.${name}`);
  return hits[0];
};
const contracts = [];
function contract(id, description, refs, probes = []) {
  const receipts = refs.map(([alias,name]) => find(alias,name));
  for (const [index, regex] of probes) assert(regex.test(receipts[index].source), `${id}: ${regex}`);
  contracts.push({id, description, anchors: receipts.map(a => a.id)});
}
contract('bridge-routing','Three current preload APIs invoke separate main/main-async/subprocess IPC handlers; deprecated APIs only log.',
  bridgeNames.map(n => ['preload',n]), [[0,/deprecated/],[1,/deprecated/],[2,/invoke\("ffiPreload"/],
    [3,/invoke\("ffiPreloadAsync"/],[4,/invoke\("ffiSubPreloadAsync"/]]);
assert.equal(ipc.filter(i => i.alias === 'main' && i.operation === 'handle').length,3);
for (const alias of ['Main','Sub']) {
  contract(`${alias}-channel-reuse`,'ConfigureFFI reuses an existing channel and appends URL; it does not compare a replacement DLL path or declaration.',
    [[alias,'_handleAction_ConfigureFFI']], [[0,/ffiLibMap.get\(/],[0,/dll already loaded/],[0,/\.Library\(/]]);
  contract(`${alias}-args-pointer-release`,'Absent/array/scalar actionArgs dispatch with zero/spread/one arguments; pointer-like results use readCString then optional FreeMalloc.',
    [[alias,'_handleAction_Default'],[alias,'_callDLLFunctionAsync']],
    [[0,/Array.isArray/],[0,/readCString/],[0,/finally/],[0,/FreeMalloc/],[1,/\.async\(/]]);
  contract(`${alias}-callback-pin`,'SetNodeFFIEvent retains one callback per channel and forwards parsed event/events to registered URLs.',
    [[alias,'_handleAction_SetNodeFFIEvent']], [[0,/ffiCallbackMap/],[0,/Callback\("void",\["string"\]/],[0,/JSON.parse/]]);
  contract(`${alias}-free-noop`,'FreeFFI special dispatch returns true while the named handler is empty; no explicit dlclose occurs here.',
    [[alias,'_handleAction_FreeFFI'],[alias,'callDLLAsync']], [[0,/=>\{\}/],[1,/case"FreeFFI":return!0/]]);
  contract(`${alias}-lifecycle-channel-key`,'Lifecycle keys concatenate channel/product/container but execution takes split("-")[0]; preserve this source behavior.',
    [[alias,'generateKey_APIToCallWithPID'],[alias,'callApiWhenExitDevice'],[alias,'callApiWhenSuspend']],
    [[0,/productId/],[0,/deviceContainerId/],[1,/split\("-"\)/],[2,/split\("-"\)/]]);
}
contract('main-mutex','Both current main wrappers acquire a per-channel mutex and release it in finally; lifecycle no-async calls bypass this acquisition.',
  [['Main','callDLL'],['Main','callDLLAsync'],['Main','_callDLLNoAsync']],
  [[0,/\.acquire\(/],[0,/finally/],[1,/\.acquire\(/]]);
assert(!find('Main','_callDLLNoAsync').source.includes('.acquire('));
contract('lifecycle-delay-unit','sleepWithWhileLoop busy-waits against performance.now(); lifecycle delay defaults to 2 milliseconds, not seconds.',
  [['Common','sleepWithWhileLoop']], [[0,/performance.now\(\)/],[0,/r=o\+e/],[0,/ms/]]);
contract('isolation-group','Subprocess manager uses ffiGroup; exit batch is a special path. Crash removes group/registry record and broadcasts to original URLs.',
  [['Manager','callDLLAsync'],['Manager','createFFIProcess'],['Manager','onCrashed'],['Manager','onNotify'],['Manager','batchCallFFIProcesses']],
  [[0,/ffiGroup/],[1,/pidReady/],[2,/\.delete\(/],[3,/getURL\(/]]);
contract('subprocess-start','A running process returns before addDLLPath; route forks the current unpacked shim. Readiness waits for ready/crash without a timer in _ensureReady.',
  [['Process','constructor'],['Process','startSubProcess'],['Process','createSubProcess'],['Process','_ensureReady']],
  [[0,/_ffiProcessRoute.js/],[1,/this._process/],[2,/\.fork\(/],[3,/subprocess-ready/]]);
assert(!find('Process','_ensureReady').source.includes('setTimeout'));
contract('subprocess-task-results','postMessage precedes pending registration; response error is logged but result resolves. Action timeout is 10000ms and does not cancel a native call.',
  [['Process','handleAction'],['Process','getResult'],['Process','handleResponse'],['Process','callDLLAsync']],
  [[0,/postMessage\(o\),await this.getResult/],[1,/1e4/],[2,/\.resolve\(/],[3,/crypto.randomUUID/]]);
contract('subprocess-exit','Termination rejects pending tasks and resets ports/readiness/URLs; normal exit and crash use different manager notification paths.',
  [['Process','onExit'],['Process','terminate'],['Process','handleFailure']],
  [[0,/0===/],[1,/\.reject\(/],[2,/subProcessCrashed/]]);
contract('child-readiness','Child signals message readiness before configuring any DLL; callback forwards ffi-event and response links taskId.',
  [['child','parent-message-handler'],['child','event-forwarder'],['route','e']],
  [[0,/subprocess-ready/],[0,/ffi-response/],[1,/ffi-event/],[2,/_ffiprocess.js/]]);
contract('eager-binding','Library eagerly resolves every declared symbol using DynamicLibrary.get; ABI/async/varargs come from tuple options.',
  [['Library','Library'],['DynamicLibrary','DynamicLibrary'],['DynamicLibrary','DynamicLibrary.prototype.get'],['DynamicLibrary','DynamicLibrary.prototype.close']],
  [[0,/_dl.get\(func\)/],[0,/async \? ff.async : ff/],[1,/dlopen\(/],[2,/dlsym\(/],[3,/dlclose\(/]]);
contract('library-entry-unload','Shipped entry exports Library and FreeLibrary; explicit FreeLibrary closes dllObj and clears it. CanUseAnneCallback returns true in this shipped JS.',
  [['Entry','exports.Library'],['Entry','exports.FreeLibrary'],['Entry','exports.CanUseAnneCallback'],['FreeLibrary','FreeLibrary']],
  [[0,/require\('\.\/library'\)/],[1,/freelibrary/],[2,/return true/],[3,/\['dllObj'\].close\(/]]);
contract('variadic-marshalling','Varargs declarations expose a type generator caching marshalled signatures; CIF_var enforces fixed count and default ABI before native preparation.',
  [['Variadic','VariadicForeignFunction'],['CIFVar','CIF_var'],['Type','Type']],
  [[0,/CIF_var\(/],[1,/ffi_prep_cif_var\(/],[1,/abi = FFI_DEFAULT_ABI/],[2,/type.indirection/]]);
contract('marshal-abi','ForeignFunction normalizes types then CIF; unspecified ABI is bindings.FFI_DEFAULT_ABI. Proxy enforces arity and marshals buffers to ffi_call/ffi_call_async.',
  [['ForeignFunction','ForeignFunction'],['CIF','CIF'],['ForeignProxy','ForeignFunction']],
  [[0,/ref.coerceType/],[1,/abi = FFI_DEFAULT_ABI/],[2,/arguments.length !== numArgs/],
    [2,/argc !== numArgs \+ 1/],[2,/bindings.ffi_call\(/],[2,/bindings.ffi_call_async\(/]]);
contract('callback-native-boundary','Callback supports Anne boolean overload, retains CIF and dereferences conventional callback pointers. bindings.js initializes the shipped native addon.',
  [['Callback','Callback'],['Bindings','bindings'],['Bindings','module.exports']],
  [[0,/typeof isAnneCallback !== 'boolean'/],[0,/callback._cif = cif/],
    [1,/node-gyp-build/],[2,/initializeBindings\(ref.instance\)/]]);
contract('registry-diagnostic','Registry records first main channel path/first subprocess PID entry; it is diagnostic metadata, not proof every loaded DLL is listed.',
  [['Registry','registerMainDLL'],['Registry','registerSubprocessDLL'],['Registry','getSubprocessLabel']],
  [[0,/!this._mainDLLs.has/],[1,/this._subprocessDLLs.has/]]);
contract('legacy-sysutil-reachable','main injects the legacy ffiMain binding into FFISysUtils; Windows SysUtils calls its callDLLMain and callDLLMainAsync, independently of renderer FFI maps.',
  [['main','legacy-ffi-sysutil-constructor'],['SysWin','constructor'],['SysWin','initDll'],['SysWin','callDLLAsync']],
  [[0,/new no\(Ve\)/],[1,/this.ffi=e/],[2,/this.ffi.callDLLMain/],[3,/callDLLMainAsync/]]);
contract('sysutil-version-fallback','Windows SysUtils eagerly declares groups and retries up to three progressively shorter prefixes; successful binding reads version and installs a callback.',
  [['SysWin','initDll']], [[0,/e.length-3>=n/],[0,/GetDLLVersion/],[0,/this.handleFFIEvent/]]);
contract('sysutil-events','Windows SysUtils filters foreground/keyboard subscriptions by URL and emits system power, suspend/resume and network events into host orchestration.',
  [['SysWin','handleFFIEvent'],['SysWin','callDLL']],
  [[0,/n.events.emit\("suspend"\)/],[0,/n.events.emit\("resume"\)/],
    [1,/keyboardLayoutChangeEventList=this.foregroundEventList.filter/]]);
const mainAst = acorn.parse(texts.get('main'),{ecmaVersion:'latest'});
const legacyRefs = [];
function identifiers(node, parents=[]) {
  if (!node?.type) return;
  if (node.type === 'Identifier' && node.name === 'Ve') legacyRefs.push({offset:node.start,end:node.end,
    parent:parents.at(-1)?.type,source:texts.get('main').slice(node.start,node.end)});
  for(const value of Object.values(node)) {
    if(Array.isArray(value)) value.forEach(n=>identifiers(n,[...parents,node]));
    else if(value?.type) identifiers(value,[...parents,node]);
  }
}
identifiers(mainAst);
assert.equal(legacyRefs.length,2,'Re-audit ffiMain reachability if main injection changes');
assert(sysutilDeclarationGroups.length>3);
const packagePath=host+'node_modules/ffi-napi-rz/package.json';
const packageBytes=fs.readFileSync(path.join(root,packagePath));
const packageData=JSON.parse(packageBytes.toString('utf8'));
assert.equal(packageData.main,'./lib/ffi');
const packageReceipt={path:packagePath,sha256:hash(packageBytes),bytes:packageBytes.length,
  name:packageData.name,version:packageData.version,main:packageData.main,dependencies:packageData.dependencies};
const result = {schema_version:1, source_version_date:'2026-10-02',
  scope:'Current host FFI transport/loader/lifecycle and shipped ffi-napi-rz JS, statically parsed only',
  scanner_sha256:hash(fs.readFileSync(__filename)), files, anchors, action_cases:cases, ipc, imports,
  contracts, package:packageReceipt, sysutil_declaration_groups:sysutilDeclarationGroups,
  legacy_main_binding_references:legacyRefs,
  limitations:['Ranges are UTF-16 code units, not byte offsets.',
    'Product-specific DLL paths, declarations, initialization callers and return consumers require their own scoped evidence.',
    'Native bindings/libffi machine bodies, actual calling convention values and DLL allocation ownership are not proved by JS wrappers.',
    'No runtime race, callback delivery, unloading, timeout cancellation or device readiness is asserted.',
    'Legacy ffiMain is injected into SysUtils. Other dynamic callers are not exhaustively established here.'],
  runtime_validation:'not_run', native_body_recovery_claimed:false};
const encoded = JSON.stringify(result,null,2)+'\n';
const output = path.join(root,'docs/re/host-ffi-current-evidence.json');
if(process.argv.includes('--check')) assert.equal(hash(fs.readFileSync(output)),hash(Buffer.from(encoded)),'Stale host FFI evidence');
else fs.writeFileSync(output,encoded);
console.log(JSON.stringify({files:files.length,anchors:anchors.length,contracts:contracts.length,
  action_cases:cases.length,ipc:ipc.length,sysutil_groups:sysutilDeclarationGroups.length,runtime_validation:'not_run'}));
