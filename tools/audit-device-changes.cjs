// Parse current vendor JavaScript as data. Never load the reference modules.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {hash, walk, key} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const receipts = [];
for (const file of [
  '.ref/host-4.0.827/electron/UsbRzDeviceAction.js',
  '.ref/host-4.0.827/source-evidence/background-current-source.js',
]) {
  const source = fs.readFileSync(path.join(root, file), 'utf8');
  const ast = acorn.parse(source, {ecmaVersion: 'latest', sourceType: 'module'});
  walk(ast, node => {
    let label;
    if (node.type === 'MethodDefinition' && key(node.key) === 'initUSB') {
      label = 'host-startMonitoring-and-add-remove-publish';
    } else if (node.type === 'CallExpression'
      && node.callee.type === 'MemberExpression'
      && key(node.callee.property) === 'response'
      && ['nodeUSBEvent-add', 'nodeUSBEvent-remove'].includes(node.arguments[0]?.value)) {
      label = `background-consume-${node.arguments[0].value}`;
    }
    if (label) receipts.push({label, path: file, sha256: hash(source),
      offset: node.start, end: node.end, source: source.slice(node.start, node.end)});
  });
}
for (const label of ['host-startMonitoring-and-add-remove-publish',
  'background-consume-nodeUSBEvent-add', 'background-consume-nodeUSBEvent-remove']) {
  if (!receipts.some(receipt => receipt.label === label)) throw Error(`Missing ${label}`);
}
const native = JSON.parse(fs.readFileSync(path.join(root, 'docs/re/usb-native-current-evidence.json'), 'utf8'));
const result = {
  method: 'Static Acorn AST; UTF-16 half-open offsets. No vendor JavaScript or DLL execution.',
  receipts,
  usbClass: {guid: native.usb_device_guid, path: native.path, sha256: native.sha256},
  nativeAdapter: {
    path: 'crates/razer-platform/src/platform/windows/device_changes.rs',
    facade: 'crates/razer-platform/src/device_changes.rs',
    mechanism: 'Windows CM_Register_Notification for USB_DEVICE and GUID_DEVINTERFACE_HID; signals existing discovery only',
    lifecycle: 'Owned thread registers before Ready; callback enqueues only; owner drop ends thread, unregisters then releases context',
    boundary: 'Native Windows adaptation, not an assertion of a callable vendor C export. Does not observe radio-only peer changes or read mouse configuration.',
  },
};
const output = path.join(root, 'docs/re/device-changes-current-evidence.json');
const text = JSON.stringify(result, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (fs.readFileSync(output, 'utf8') !== text) throw Error('Stale device change evidence');
} else fs.writeFileSync(output, text);
console.log(`Device changes: ${receipts.length} current-source receipts; native adapter boundary recorded`);
