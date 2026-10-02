// Firmware copy is scoped independently of Dashboard; source JS is only parsed.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const folder = '.ref/applications/synapse/update-fw';
const check = process.argv.includes('--check');
const keys = `UPDATE RETRY NEXT SKIP UPDATE_REQUIRED NO_UPDATE_REQUIRED UPDATE_DONGLE
UPDATE_SUCCESSFUL_HEADER UPGRADE_MOUSE_COMPLETED_NOTE UPGRADE_KEYBOARD_COMPLETED_NOTE
UPDATE_IN_PROGRESS DEVICE_IS_DISCONNECTED CANCEL_UPDATE UPDATE_FAILED UPDATE_FAILED_DETAIL
SHOW_WIRELESS_CONNECTION_TIPS WAIT_FOR_OTHER_DEVICES CONTINUE_WITH_UPDATE DEVICE_IS_UPGRADING
DEVICE_IS_DISCONNECTED_DETAIL FW_UPDATER_UTILITY PLEASE_NOTE LAPTOP_NOTE CURRENT_VERSION
LATEST_VERSION PENDING UPDATING UPDATING_NOTE UPDATE_SUCCESSFUL MOUSE_LATEST KEYBOARD_LATEST
DONGLE_IS_LATEST NEXT_UPGRADE_MOUSE NEXT_UPGRADE_KEYBOARD NEXT_UPGRADE_DONGLE PLEASE_FOLLOW_STEPS
SWITCH_MOUSE_OFF SWITCH_KEYBOARD_OFF CONNECT_MOUSE_WITH_CABLE CONNECT_KEYBOARD_WITH_CABLE
PLUGGED_HYPERSPEED_WIRELESS_DONGLE SWITCH_MOUSE_CONNECT_TO_DONGLE SWITCH_KEYBOARD_CONNECT_TO_DONGLE
UPDATE_FAILED_HEADER UPDATE_FINISHED START_OVER CLOSE`.split(/\s+/);
const read = name => fs.readFileSync(path.join(root, name), 'utf8');
const hash = text => crypto.createHash('sha256').update(text).digest('hex');
const manifestText = read(`${folder}/asset-manifest.json`);
const sources = [...new Set(Object.values(JSON.parse(manifestText).files))]
  .filter(name => /\/trans-[^.]+\.[a-f0-9]+\.chunk\.js$/.test(name)).sort();
if (sources.length !== 10) throw Error(`Expected 10 locale bundles, found ${sources.length}`);
// Current Ee/Ue first falls back to English, then returns the original key.
// These are verified source omissions, not translations to invent locally.
const absentFromAll = new Set(['DEVICE_IS_UPGRADING']);
const englishOnly = new Set(['SWITCH_MOUSE_CONNECT_TO_DONGLE', 'SWITCH_KEYBOARD_CONNECT_TO_DONGLE']);
function walk(node, visit, ancestors = []) {
  if (!node || typeof node !== 'object') return;
  if (node.type) visit(node, ancestors);
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) value.forEach(child => walk(child, visit, [...ancestors, node]));
    else if (value && typeof value === 'object') walk(value, visit, [...ancestors, node]);
  }
}
const outputs = [];
const records = [];
for (const file of sources) {
  const locale = path.basename(file).match(/^trans-(.*?)\./)[1];
  const source = `${folder}/${file.replace(/^\.\//, '')}`;
  const text = read(source);
  const ast = acorn.parse(text, {ecmaVersion: 'latest'});
  let exports, body;
  walk(ast, (node, ancestors) => {
    if (node.type !== 'CallExpression' || node.callee.type !== 'MemberExpression'
        || node.callee.property.name !== 'd') return;
    const object = node.arguments[1];
    if (object?.type !== 'ObjectExpression' || object.properties.length < 1000) return;
    if (exports) throw Error(`Ambiguous translation exports: ${source}`);
    exports = object;
    body = ancestors.findLast(parent => /FunctionExpression$/.test(parent.type))?.body;
  });
  if (!exports || body?.type !== 'BlockStatement') throw Error(`Missing translations: ${source}`);
  const values = new Map();
  for (const statement of body.body) {
    if (statement.type !== 'VariableDeclaration') continue;
    for (const declaration of statement.declarations) {
      if (declaration.id.type === 'Identifier' && declaration.init) {
        if (values.has(declaration.id.name)) throw Error(`Duplicate binding: ${source}`);
        values.set(declaration.id.name, declaration.init);
      }
    }
  }
  function stringValue(name, seen = new Set()) {
    if (seen.has(name)) throw Error(`Cyclic translation: ${source}:${name}`);
    seen.add(name);
    const value = values.get(name);
    if (value?.type === 'Literal' && typeof value.value === 'string') return value.value;
    if (value?.type === 'Identifier') return stringValue(value.name, seen);
    throw Error(`Nonliteral translation: ${source}:${name}`);
  }
  const translations = {};
  const missing = [];
  for (const key of keys) {
    const property = exports.properties.find(property => (property.key.name ?? property.key.value) === key);
    if (!property && (absentFromAll.has(key) || locale !== 'en' && englishOnly.has(key))) {
      missing.push(key);
      continue;
    }
    if (!property || property.computed || property.value.type !== 'ArrowFunctionExpression'
        || property.value.params.length || property.value.body.type !== 'Identifier') {
      throw Error(`Missing literal export: ${source}:${key}`);
    }
    translations[key] = stringValue(property.value.body.name);
  }
  const target = `locales/${locale}.json`;
  const existing = JSON.parse(read(target));
  if (check && JSON.stringify(existing.FIRMWARE_SOURCE) !== JSON.stringify(translations)) {
    throw Error(`Stale firmware copy: ${target}`);
  }
  outputs.push([target, JSON.stringify({...existing, FIRMWARE_SOURCE: translations}, null, 2) + '\n']);
  records.push({locale, source, source_sha256: hash(text), keys: Object.keys(translations).length,
    missing_exports: missing});
  console.log(`${locale}: ${Object.keys(translations).length} firmware strings; ${missing.length} source omissions`);
}
const mainSource = `${folder}/${JSON.parse(manifestText).files['main.js'].replace(/^\.\//, '')}`;
const receipt = {
  manifest: `${folder}/asset-manifest.json`, manifest_sha256: hash(manifestText),
  namespace: 'FIRMWARE_SOURCE',
  method: 'Acorn AST: module-local string literals, aliases and explicit export getters; no execution',
  fallback: {source: mainSource, source_sha256: hash(read(mainSource)),
    behavior: 'Ee/Ue: selected locale, then en, then the unqualified source key',
    absent_from_all: [...absentFromAll], english_only: [...englishOnly]},
  keys, locales: records,
};
const receiptPath = 'docs/re/firmware-locale-source.json';
if (check) {
  if (JSON.stringify(JSON.parse(read(receiptPath))) !== JSON.stringify(receipt)) {
    throw Error('Stale firmware locale source receipt');
  }
} else {
  for (const [target, content] of outputs) fs.writeFileSync(path.join(root, target), content);
  fs.writeFileSync(path.join(root, receiptPath), JSON.stringify(receipt, null, 2) + '\n');
}
