// Parse current reference JavaScript as data; never execute vendor modules.
const fs = require('fs'), path = require('path');
const {Source, hash, walk, key} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..'), source = new Source('synapse/dashboard');
const check = process.argv.includes('--check');
const write = (file, text) => {
  const target = path.join(root, file);
  if (check) { if (fs.readFileSync(target, 'utf8') !== text) throw Error(`Stale ${file}`); }
  else fs.writeFileSync(target, text);
};
const paths = source.literal(96689, source.binding(96689, 's'));
const transforms = source.literal(96689, source.binding(96689, 'i'));
const categories = paths.filter(p => transforms[p.name]).map(p => ({...p, ...transforms[p.name]}));
const entries = [];
for (const item of categories) {
  const file = `assets/synapse/service-category-${item.name.toLowerCase()}.svg`;
  const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40" fill="#ccc" viewBox="0 0 20 20"><g transform="translate(0 0)"><path d="${item.d}" transform="translate(${item.transformX} ${item.transformY})"/></g></svg>\n`;
  write(file, svg);
  entries.push({asset: file.slice(7), source: source.module(96689).file, source_sha256: hash(source.text(source.module(96689).file)), output_sha256: hash(svg), category: item.name});
}
let warningPath;
walk(source.module(27875).fn, node => {
  if (node.type !== 'ObjectExpression') return;
  const d = node.properties.find(p => key(p.key) === 'd');
  const fill = node.properties.find(p => key(p.key) === 'fill');
  if (d && fill) warningPath = {d:source.literal(27875,d.value),fill:source.literal(27875,fill.value)};
});
if (!warningPath) throw Error('Missing current firmware warning path');
const warningSvg = `<svg xmlns="http://www.w3.org/2000/svg" width="20" height="27" viewBox="0 0 20 20"><path fill="${warningPath.fill}" d="${warningPath.d}"/></svg>\n`;
write('assets/synapse/service-firmware-warning.svg',warningSvg);
entries.push({asset:'synapse/service-firmware-warning.svg',source:source.module(27875).file,source_sha256:hash(source.text(source.module(27875).file)),output_sha256:hash(warningSvg)});
// Dedicated include keeps independent resource writers from overwriting each other.
write('assets/synapse/module-service-embedded.rs', '&[\n' + entries.map(e => `    ("${e.asset}", include_bytes!("${path.basename(e.asset)}") as &[u8]),`).join('\n') + '\n]\n');
const data = {categories: categories.map(p => p.name), subcategory_parents: Object.keys(source.literal(29228, source.exported(29228, 'Su')))};
write('src/shell/module_service_source.json', JSON.stringify(data, null, 2)+'\n');
const cssFiles = ['6505.9782778c.chunk.css', '55.4e8559cb.chunk.css'];
const css = cssFiles.map(name => {
  const file = `${source.directory}/static/css/${name}`, text = fs.readFileSync(path.join(root, file), 'utf8');
  return {path:file, sha256:hash(text), rules:text.split('}').filter(r=>/items|firmware|profile-del|progress-bar|spinner-razer|\.loader|\.thx-btn|\.body-text/.test(r)).map(r=>r+'}')};
});
const contracts = [[44442,'ae'],[44442,'w'],[44442,'O'],[44442,'L'],[44442,'W'],[44442,'z'],[44442,'ne'],[88466,'n'],[96689,'c'],[96689,'d']]
  .map(([id,name])=>({module:id,symbol:name,...source.receipt(id,source.binding(id,name))}));
contracts.push({module:15597,symbol:'module',...source.receipt(15597,source.module(15597).fn)});
write('docs/re/module-service-current-evidence.json', JSON.stringify({method:'Acorn AST and CSS only; source-record projection is local and not a native service connection.', contracts, css, entries, data}, null, 2)+'\n');
console.log(`Current Devices & Modules service contracts: ${contracts.length} AST, ${entries.length} category SVGs`);
