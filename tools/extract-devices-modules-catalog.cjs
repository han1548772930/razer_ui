// Current 6505/44442 catalogue data. Parse literals with Acorn; never execute
// application JavaScript or resolve service-dependent expressions by guessing.
const fs = require('fs'), path = require('path');
const {Source, key, hash} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const source = new Source('synapse/dashboard');
const names = source.binding(44442, 'ee');
const titles = {};
for (const item of names.properties) {
  const title = {}, beta = {};
  for (const language of item.value.properties) {
    const value = language.value;
    if (value.type === 'ConditionalExpression') {
      if (source.snippet(44442, value.test) !== 'X.Vb') throw Error('Unresolved title condition');
      beta[key(language.key)] = source.literal(44442, value.consequent);
      title[key(language.key)] = source.literal(44442, value.alternate);
    } else title[key(language.key)] = source.literal(44442, value);
  }
  titles[key(item.key)] = title;
  if (Object.keys(beta).length) titles[`${key(item.key)}_beta`] = beta;
}
const catalogue = source.binding(44442, 'ne');
const ids = catalogue.properties.map(property => key(property.key));
if (JSON.stringify(ids) !== JSON.stringify(['alexa', 'macro', 'linkedGames', 'feedback', 'armory']))
  throw Error('Current catalogue membership changed');
const details = {};
for (const item of catalogue.properties) {
  const detail = item.value.properties.find(property => key(property.key) === 'detail')?.value;
  if (!detail) continue;
  details[key(item.key)] = {};
  for (const property of detail.properties) if (['size', 'learnMoreURL'].includes(key(property.key)))
    details[key(item.key)][key(property.key)] = source.literal(44442, property.value);
}
const data = {ids, titles, descriptions: source.literal(44442, source.binding(44442, 'te')), details};
const evidence = {
  source_date: '2026-10-05',
  method: 'Current manifest-listed 6505/44442 declarations parsed as Acorn data. Feedback host-beta branches retained separately; Armory feature condition remains runtime data.',
  generator_sha256: hash(fs.readFileSync(__filename)),
  contracts: ['ae', 'H', 'w', 'O', 'L', 'ne', 'te', 'ee', 'ie', 'u'].map(symbol => ({symbol, ...source.receipt(44442, source.binding(44442, symbol))})),
  armory_features: source.receipt(77989, source.binding(77989, 'i')),
  data,
};
for (const [file, value] of [
  ['crates/razer-app-pages/src/devices_modules_catalog.json', data],
  ['docs/re/devices-modules-current-catalog.json', evidence],
]) {
  const text = JSON.stringify(value, null, 2) + '\n';
  if (process.argv.includes('--check')) {
    if (fs.readFileSync(path.join(root, file), 'utf8') !== text) throw Error(`${file} is stale`);
  } else fs.writeFileSync(path.join(root, file), text);
}
console.log(`Current Devices & Modules: ${ids.join(', ')}; ${Object.keys(titles.macro).length} locales; source detail sizes retained`);
