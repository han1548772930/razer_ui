// One current product per process. Parse syntax; never evaluate vendor code.
const fs = require('node:fs');
const path = require('node:path');
const {ProductSource, property, member} = require('./audit-source-profile-menu.cjs');
const {walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const pid = Number(process.argv[2]);
if (!Number.isInteger(pid) || pid <= 0) throw Error('Provide one product ID');
const directory = `local-ui-reverse/source/official/apps.razer.com/synapse/products/${pid}/ui`;
const manifest = JSON.parse(fs.readFileSync(path.join(root, directory, 'asset-manifest.json'), 'utf8'));
const file = `${directory}/${manifest.files['main.js'].replace(/^\.\//, '')}`;
const source = new ProductSource(pid, file);
const mounts = source.calls.filter(call => property(call.arguments[1], 'isEnableProfileBar'));
if (mounts.length !== 1) throw Error(`Expected one mounted profile bar, got ${mounts.length}`);
const mount = mounts[0];
let bar = source.component(mount.arguments[0]);
if (!bar) throw Error('Unresolved mounted profile bar component');
const wrappers = [];
let barIcon;
const visited = new Set();
const jsxCall = call => {
  const callee = call.callee.type === 'SequenceExpression' ? call.callee.expressions.at(-1) : call.callee;
  return member(callee, 'jsx') || member(callee, 'jsxs');
};
while (!(barIcon = source.assignment(bar.body?.type === 'ClassBody' ? bar.body : bar, 'renderProfileBarIcon')[0])) {
  if (visited.has(bar)) throw Error('Cyclic profile bar wrapper');
  visited.add(bar);
  const children = source.calls.filter(call => call.start > bar.start && call.end < bar.end &&
    jsxCall(call) && source.component(call.arguments[0]));
  if (children.length !== 1) throw Error(`Unknown profile bar wrapper: ${children.length} component children`);
  wrappers.push({component:source.receipt(bar),child:source.receipt(children[0])});
  bar = source.component(children[0].arguments[0]);
}
const mountedRoot = source.owners.get(mount);
const ancestorConditions = [];
for (let node = source.parents.get(mount); node && node !== mountedRoot; node = source.parents.get(node)) {
  if (node.type === 'ConditionalExpression') ancestorConditions.push(source.receipt(node.test));
  if (node.type === 'LogicalExpression') ancestorConditions.push(source.receipt(node.left));
}
const exclusions = [];
walk(mountedRoot, node => {
  if (node.type !== 'Property' || key(node.key) !== 'displayProfileBar') return;
  const value = node.value;
  if (value.type !== 'UnaryExpression' || value.operator !== '!' || !member(value.argument.callee, 'includes')) {
    throw Error('Unknown profile sync predicate');
  }
  exclusions.push({expression:source.receipt(node), pages:source.literal(value.argument.callee.object),
    list:source.receipt(source.resolve(value.argument.callee.object))});
});
if (!exclusions.length) throw Error('No root profile sync predicates');
if (exclusions.some(entry => JSON.stringify(entry.pages) !== JSON.stringify(exclusions[0].pages))) {
  throw Error('Profile sync predicates differ within root');
}
const helpOwners = [...new Set(source.assignments.filter(node => key(node.left.property) === 'copyDeviceSerial').map(node => source.owners.get(node)))];
const helps = helpOwners.map(owner => {
  const component = owner.type === 'ClassBody' ? source.parents.get(owner) : owner;
  const calls = source.calls.filter(call => jsxCall(call) && source.component(call.arguments[0]) === component);
  const layoutComponents = new Map();
  for (const call of source.calls) {
    if (call.start < owner.start || call.end > owner.end || !jsxCall(call)) continue;
    const props = call.arguments[1];
    if (!property(props, 'direction') && !property(props, 'title') && !property(props, 'children')) continue;
    const child = source.component(call.arguments[0]);
    if (child && child.end - child.start < 4000) layoutComponents.set(child,source.receipt(child));
  }
  const defaults = source.assignments.filter(node => key(node.left.property) === 'defaultProps' && source.resolve(node.left.object) === component);
  if (defaults.length !== 1) throw Error('Unresolved Help default props');
  return {component:source.receipt(owner), mounts:calls.map(call => source.receipt(call)),
    default_props:{expression:source.receipt(defaults[0]),value:source.literal(defaults[0].right)},
    layout_components:[...layoutComponents.values()]};
});
let deviceInfo;
walk(mountedRoot, node => {
  if (deviceInfo || !member(node, 'DeviceInfo')) return;
  try {
    const value = source.literal(node);
    if (value?.productId !== pid) return;
    deviceInfo = {expression:source.receipt(node), value};
  } catch (_) {}
});
if (!deviceInfo) throw Error('Unresolved root DeviceInfo');
const cssFile = `${directory}/${manifest.files['main.css'].replace(/^\.\//, '')}`;
const css = fs.readFileSync(path.join(root, cssFile), 'utf8');
const cssRules = parseCSS(css).filter(rule => /help|support|widget|panel|button|link|profile-bar|\.mt[12]0|div\.flex/.test(rule.selector));
const evidence = {product_id:pid, offset_unit:'UTF-16 code units in the original JS string',source:{path:file,sha256:hash(source.source)},device_info:deviceInfo,
  profile_bar:{mount:source.receipt(mount),wrappers,ancestor_conditions:ancestorConditions,icon_consumer:source.receipt(barIcon),sync_exclusions:exclusions},
  help:helps, css:{path:cssFile,sha256:hash(css),rules:cssRules}};
const output = path.join(root, '.work/product-shell-current');
fs.mkdirSync(output, {recursive:true});
fs.writeFileSync(path.join(output, `${pid}-evidence.json`), JSON.stringify(evidence,null,2)+'\n');
console.log(JSON.stringify({product_id:pid,profile_mount_conditions:ancestorConditions.length,
  sync_excluded_pages:exclusions[0].pages,help_components:helps.length,output}));
