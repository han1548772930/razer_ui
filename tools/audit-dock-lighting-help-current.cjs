// Static current-source subtree review; no reference JavaScript is evaluated.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {walk, hash} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..'), check = process.argv.includes('--check');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const files = new Map();
function source(file) {
  if (files.has(file)) return files.get(file);
  if (!/^\.ref\/devices\/(164|179|241)\//.test(file)) throw Error('Unexpected source ' + file);
  const text = read(file), ast = acorn.parse(text, {ecmaVersion:'latest'}), nodes = new Map(), scopes = new WeakMap();
  function visit(node, scope) {
    if (!node?.type) return;
    if (/^(FunctionDeclaration|ClassDeclaration)$/.test(node.type)) scope?.defs.set(node.id.name, node);
    if (/Function/.test(node.type) || node.type === 'Program') scope = {parent:scope, defs:new Map()};
    if (node.type === 'VariableDeclarator' && node.id.type === 'Identifier') scope.defs.set(node.id.name, node.init);
    scopes.set(node, scope); nodes.set(`${node.start}:${node.end}`, node);
    for (const value of Object.values(node)) {
      if (Array.isArray(value)) value.forEach(child => visit(child, scope));
      else if (value?.type) visit(value, scope);
    }
  }
  visit(ast, null);
  const result = {file, text, sha256:hash(text), nodes, scopes}; files.set(file,result); return result;
}
function receipt(file, start, end) {
  const s = source(file), node=s.nodes.get(`${start}:${end}`);
  if (!node) throw Error(`No exact AST node ${file}:${start}:${end}`);
  return {path:file, sha256:s.sha256, offset:start, end, source:s.text.slice(start,end)};
}
function binding(anchor, name) {
  const s=source(anchor.path), node=s.nodes.get(`${anchor.offset}:${anchor.end}`);
  let scope=s.scopes.get(node);
  while(scope) {
    if(scope.defs.has(name)){const n=scope.defs.get(name);return receipt(anchor.path,n.start,n.end);}
    scope=scope.parent;
  }
  throw Error('Missing binding '+name);
}
const products=[];
for(const [pid,file,bounds,names] of [
  [164,'.ref/devices/164/static/js/main.458d4103.js', [4421298,4421512], ['hr','Ur','Os','Is','Lo','fr']],
  [241,'.ref/devices/241/static/js/main.899712fe.js', [496322,496536], ['fn','Cn','mt','vt','L','Rn']],
]) {
  const mounted=receipt(file,...bounds), components={mounted};
  for(const name of names)components[name]=binding(mounted,name);
  if (!components[names[1]].source.includes('return this.props.portComponent?') ||
      !components[names[3]].source.includes('this.handleDefaultWaveDirection=') ||
      !mounted.source.includes(`jsx)(${names[0]},{})`)) throw Error('Lighting mount changed '+pid);
  products.push({product_id:pid,page:'TAB_LIGHTING',components,
    findings:['Corrected the native descriptor receipt: this page passes no portComponent and mounts the regular quick-effects class; the inactive port branch is not evidence for this control.',
      'Native Lighting now mounts original left brightness/idle and right effects columns, title-contained brightness switch, source slider geometry and source mouse-release submission. Actual brightness query/write connection is audited independently in receiver-brightness-page-current-evidence.json; complete effect settings, quick/advanced selection and sync remain missing.']});
}
// Retained current exact-node anchors avoid depending on the retired general
// Help inventory. Reparse and match every anchor against actual current bytes.
const help=JSON.parse(read('docs/re/dock-lighting-help-review-current-evidence.json'));
for(const pid of [164,179,241]) {
  const page=help.products.find(p=>p.product_id===pid&&p.page==='HELP');
  if(!page)throw Error('Missing retained current Help source anchor '+pid);
  const component=page.components.help;
  const current=receipt(component.path,component.offset,component.end);
  if(current.source!==component.source || !current.source.includes('type:"ON_RESET_DEVICE"') || !current.source.includes('this.confirmDel='))throw Error('Help reset contract changed '+pid);
  products.push({product_id:pid,page:'HELP',navigation_offset:page.navigation_offset,components:{help:current},
    findings:[pid===179?'Current 179 Help confirmation is acquired; its real reset branch remains unimplemented and confirmation stays disabled.':'Current 164/241 ordinary Help taskMakerResetOBM is connected to actual serial-owned source document producer, metadata/cache mutation, local persistence, conditional device brightness and true partial receipts. Existing profiles are retained. Original two-second button cooldown is separate from real completion; effects and mappings are not yet submitted.']});
}
const native=['crates/razer-pages/src/features/source_help.rs','crates/razer-pages/src/features/source_controls.rs','crates/razer-pages/src/features/accessory_controls_data.json','crates/razer-pages/src/features/source_controls/receiver_lighting_page.rs','crates/razer-pages/src/features/source_controls/receiver_brightness_ui.rs','crates/razer-shell/src/shell/receiver_reset.rs','crates/razer-shell/src/shell/receiver_brightness_page.rs','crates/razer-storage/src/receiver_reset.rs'];
const output='docs/re/dock-lighting-help-review-current-evidence.json';
const value={method:'Acorn exact nodes, lexical binding resolution, current normal lighting branch and Help reset callbacks; no vendor execution',generator_sha256:hash(read('tools/audit-dock-lighting-help-current.cjs')),products,native:native.map(path=>({path,sha256:hash(read(path))}))};
const serialized=JSON.stringify(value,null,2)+'\n';
if(check){if(read(output)!==serialized)throw Error('Stale '+output);}else fs.writeFileSync(path.join(root,output),serialized);
console.log(`Dock lighting/Help: ${products.length} product pages, ${products.reduce((n,p)=>n+Object.keys(p.components).length,0)} exact current AST nodes.`);
