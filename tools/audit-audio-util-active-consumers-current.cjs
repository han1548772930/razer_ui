// Inspect current product boot registrations and exported feature classes only.
// Acorn parses original bytes as data; no vendor JavaScript is executed.
const fs = require('fs'), path = require('path'), assert = require('assert');
const acorn = require('acorn');
const {CurrentMiddlewareSource} = require('./current-middleware-source.cjs');
const {walk, key, hash} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const products = [];
const modules = new Map([[1422, 80168], [1446, 45871]]);
for (const product of [1398, 1422, 1427, 1446, 2638, 2641, 4124, 4126]) {
  const s = new CurrentMiddlewareSource(product);
  const text = s.text(s.mainFile), registrations = [], names = [];
  walk(acorn.parse(text, {ecmaVersion: 'latest'}), node => {
    if (node.type !== 'CallExpression' || node.callee.type !== 'MemberExpression'
        || key(node.callee.property) !== 'useFeature'
        || node.arguments[0]?.value !== 'genericFeature') return;
    const config = node.arguments[2];
    assert.equal(config?.type, 'ObjectExpression');
    const features = config.properties.find(p => key(p.key) === 'features')?.value;
    assert.equal(features?.type, 'ArrayExpression');
    for (const feature of features.elements) {
      const name = feature.properties.find(p => key(p.key) === 'featureKey')?.value?.value;
      assert.equal(typeof name, 'string'); names.push(name);
      if (['audio_streamMixer', 'audioUtil'].includes(name)) registrations.push({
        path: s.mainFile, sha256: hash(text), offset: feature.start, end: feature.end,
        source: text.slice(feature.start, feature.end),
      });
    }
  });
  assert.equal(registrations.length, 1, `Ambiguous boot registration: ${product}`);
  const active = names.includes('audio_streamMixer');
  assert.equal(active, modules.has(product));
  const id = active ? modules.get(product) : 9915;
  let klass = s.exported(id, active ? 'Audio_StreamMixer' : 'AudioUtil');
  for (let count = 0; klass.type === 'Identifier'; count++) {
    assert(count < 8); klass = s.binding(id, klass.name);
  }
  assert(['ClassDeclaration', 'ClassExpression'].includes(klass.type));
  const methods = klass.body.body.filter(m => m.type === 'MethodDefinition');
  const wanted = active
    ? ['init', 'enableAudioDeviceChangeNotification', 'getWindowsPlaybackDevices',
       'getWindowsRecordDevices', 'getAudioDevicesList', 'getStreamMixerDeviceName',
       'enableRouting', 'routeExternalDevice', 'setStreamMixerEnable', 'setSamplingRate']
    : ['init', 'setAudioUtilCommand'];
  const receipts = wanted.map(name => {
    const found = methods.filter(m => key(m.key) === name);
    assert.equal(found.length, 1, `${product} ${name}`);
    return {name, module: id, ...s.receipt(id, found[0])};
  });
  const init = receipts.find(r => r.name === 'init').source;
  assert.equal(init.includes('enableAudioDeviceChangeNotification(!0)'), active);
  if (!active) assert(!init.includes('EnableNotification') && !init.includes('AudioRouter'));
  products.push({product_id: product, feature_names: names, registrations,
    active_feature: active ? 'Audio_StreamMixer' : 'AudioUtil',
    automatic_audio_notifications: active, receipts, acquisition: s.acquisition});
}
const output = 'docs/re/audio-util-active-consumers-current.json';
const value = {method: 'Current HTTP-verified manifest, boot AST and exported class methods; no target execution',
  offset_unit: 'JavaScript UTF-16 code units',
  automatic_notification_products: products.filter(p => p.automatic_audio_notifications).map(p => p.product_id),
  products,
  boundary: 'A bundled Audio_StreamMixer method is not a startup caller. Generic AudioUtil may dispatch explicit UI commands, but does not automatically enable AudioEnumerator notifications.'};
const bytes = JSON.stringify(value, null, 2) + '\n';
if (process.argv.includes('--check')) assert.equal(fs.readFileSync(path.join(root, output), 'utf8'), bytes, 'Stale ' + output);
else fs.writeFileSync(path.join(root, output), bytes);
console.log(`AudioUtil activation: ${products.length} boot roots, ${value.automatic_notification_products.length} automatic notification owners; vendor code not executed`);
