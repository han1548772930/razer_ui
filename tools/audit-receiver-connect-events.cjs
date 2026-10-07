// Current hardware notification -> middleware broadcast -> parent loading state.
// Parse source bytes only; do not load vendor modules or call native libraries.
const fs = require('fs');
const path = require('path');
const { MiddlewareSource } = require('./middleware-source.cjs');
const { Source, walk, hash } = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const mw = new MiddlewareSource(179);
const receipts = [];
const receipt = (source, module, binding, node) => {
  const value = { module, binding, ...source.receipt(module, node) };
  receipts.push(value);
  return value.source;
};
for (const name of ['he', 'Ne', 'oe', 'De', 'ye']) receipt(mw, 34340, name, mw.binding(34340, name));
if (mw.snippet(34340, mw.module(34340).exports.get('kO')) !== 'oe' ||
    mw.snippet(78548, mw.binding(78548, 'f')) !== 'n(34340)') throw Error('Changed slave event export/import');
const branches = [];
walk(mw.binding(78548, 'Be'), node => {
  if (node.type === 'IfStatement' && node.test.type === 'MemberExpression' &&
      node.test.object.name === 'at' && node.test.property.name === 'isDualLink') branches.push(node);
});
if (branches.length !== 1) throw Error('Changed dual-link hardware notification branch');
const branch = receipt(mw, 78548, 'Be.isDualLink', branches[0]);
for (const expression of ['ke===o.ZK.NOTIFICATION_EVENTS', 'ke===o.ZK.DONGLE_SECONDARY_DEVICE_NOTIFICATION_EVENTS',
  'at.productId===at.dualLinkPrimaryPId', 'xe===o.E3.CONNECT_EVENT', '(0,f.kO)', 'je!==T.DeviceInfo.category']) {
  if (!branch.includes(expression)) throw Error('Changed connection event condition: ' + expression);
}
const wireless = [];
walk(mw.binding(78548, 'Be'), node => {
  if (node.type === 'VariableDeclarator' && node.id.name === 'st') wireless.push(node.init);
});
if (wireless.length !== 1) throw Error('Changed wireless event classification');
receipt(mw, 78548, 'Be.wirelessEvent', wireless[0]);
const enums = Object.fromEntries(['o', 'p', 'y'].map(name => {
  receipt(mw, 50652, name, mw.binding(50652, name));
  return [name, mw.literal(50652, mw.binding(50652, name))];
}));
if (enums.o.NOTIFICATION_EVENTS !== 5 || enums.o.DONGLE_SECONDARY_DEVICE_NOTIFICATION_EVENTS !== 9 ||
    enums.p.WIRELESS_CHANGE_EVENT !== 9 || enums.p.WIRELESS_CHANGE_EVENT_V2 !== 53 ||
    enums.y.CONNECT_EVENT !== 3) throw Error('Changed hardware notification enum values');

const ui = Object.create(Source.prototype);
ui.directory = '.ref/devices/179';
const manifestPath = ui.directory + '/asset-manifest.json';
const manifest = JSON.parse(read(manifestPath));
ui.files = [...new Set(Object.values(manifest.files))]
  .filter(file => /^\.\/static\/js\/[^/]+\.js$/.test(file))
  .map(file => ui.directory + '/' + file.slice(2));
ui.modules = new Map(); ui.texts = new Map(); ui.parsed = new Set();
const consumers = [];
walk(ui.binding(9473, 'Te'), node => {
  if (node.type === 'LogicalExpression' && node.operator === '&&' &&
      node.left.type === 'BinaryExpression' && node.left.left.value === 'SLAVE_CONNECT_EVENT') consumers.push(node);
});
if (consumers.length !== 1) throw Error('Changed Te connection event consumer');
const consumer = receipt(ui, 9473, 'Te.SLAVE_CONNECT_EVENT', consumers[0]);
if (!consumer.includes('f(!0),setTimeout(()=>{f(!1)},2e4)')) throw Error('Changed connection loading delay');
const output = {
  method: 'Current manifest-owned middleware and UI AST parsing; no vendor JavaScript, app or DLL execution.',
  generator_sha256: hash(fs.readFileSync(__filename)),
  receipts,
  contract: {
    producer: '78548/Be hardware event -> 34340/kO=oe -> MW_RESPONSE_UI SLAVE_CONNECT_EVENT',
    prerequisite: 'isDualLink; different category branch; notification record 5 or 9; wireless event 9 or 53; own PID equals dualLinkPrimaryPId; state equals 3',
    consumer: '9473/Te turns loading on then off after 20000ms; this broadcast itself supplies no binding payload',
    query_separation: '34340/he/Ne queries use status=1 for online, not hardware eventValue.state=3; a query result is not a notification',
    side_effects: 'oe also calls ye for unregistered slaves; ye writes duallink-devices and registers runtime listeners. Do not reuse that function as a read-only native callback',
    native_boundary: 'No actual hardware notification publisher is wired to this UI. Do not manufacture SLAVE_CONNECT_EVENT from connection query results',
  },
};
const target = 'docs/re/receiver-connect-events-current-evidence.json';
const serialized = JSON.stringify(output, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (read(target) !== serialized) throw Error('Stale receiver connection event evidence');
} else fs.writeFileSync(path.join(root, target), serialized);
console.log(`Receiver connection events: ${receipts.length} current AST receipts; notification/query states remain separate.`);
