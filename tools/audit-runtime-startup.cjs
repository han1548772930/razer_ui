// Current-source receipts only. Never require/evaluate a reference module or DLL.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {Source, hash, walk, key} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const receipts = [];
function parse(file) {
  const source = fs.readFileSync(path.join(root, file), 'utf8');
  return {file, source, ast:acorn.parse(source, {ecmaVersion:'latest'})};
}
function receipt(parsed, node, label) {
  receipts.push({label, path:parsed.file, sha256:hash(parsed.source),
    offset:node.start, end:node.end, source:parsed.source.slice(node.start,node.end)});
}
const base = '.ref/host-4.0.827/electron/';
for (const [module, methods] of [
  ['simple_service', ['simpleGetVersionInfo','simpleEnumerateAudioDevices']],
  ['mapping_engine', ['getGlobalMode','getGlobalShortcuts']],
]) {
  const p = parse(`${base}modules/${module}/win/index.js`);
  for (const name of methods) {
    let count = 0;
    walk(p.ast, node => {
      if (['Property','PropertyDefinition','MethodDefinition'].includes(node.type)
          && key(node.key) === name) {
        receipt(p,node,`${module}:${name}:${node.type}`); count++;
      }
    });
    if (count < 2) throw Error(`Missing API and implementation for ${name}`);
  }
}
const main = parse(base+'main.js');
let startup;
walk(main.ast, node => {
  if (node.type === 'IfStatement' && main.source.slice(node.test.start,node.test.end).endsWith('.isDebugCommonDLL')) startup = node;
});
if (!startup) throw Error('Missing current host core DLL selection');
receipt(main,startup,'debug-versus-packaged-core-DLL-selection');
const usb = parse(base+'UsbRzDeviceAction.js');
walk(usb.ast, node => {
  if (node.type === 'SwitchCase' && node.test?.value === 'usb.getDevices') receipt(usb,node,'usb.getDevices');
});
const dashboard = new Source('synapse/dashboard');
for (const name of ['z','G','V','K']) receipts.push({label:`Dashboard22534:${name}`,module:22534,
  ...dashboard.receipt(22534,dashboard.binding(22534,name))});
const result = {method:'Static Acorn AST, UTF-16 half-open offsets; no code execution',receipts};
const output = path.join(root,'docs/re/runtime-startup-current-evidence.json');
const text = JSON.stringify(result,null,2)+'\n';
if (process.argv.includes('--check')) {
  if (fs.readFileSync(output,'utf8') !== text) throw Error('Stale startup evidence');
} else fs.writeFileSync(output,text);
console.log(`Runtime startup: ${receipts.length} current-source receipts`);
