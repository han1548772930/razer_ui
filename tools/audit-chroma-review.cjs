// Current Chroma review receipts; source files are parsed as data, never executed.
const fs = require('fs');
const {Source, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const s = new Source('chroma-app/dashboard');
const manifest = JSON.parse(fs.readFileSync(`${s.directory}/asset-manifest.json`, 'utf8'));
s.files = [...new Set(Object.values(manifest.files))]
  .filter(v => /^\/chroma-app\/dashboard\/static\/js\/[^/]+\.js$/.test(v))
  .map(v => `.ref/applications${v}`);
const contracts = [];
for (const [id, names] of [[23322, ['Ks','nt']], [62296, ['Wn','Pn','Ss','Cs','bs','Kt','ue','Bi','us','ls']],
    [40554, ['K','Z','F','k']], [65596, ['j']]]) {
  for (const name of names) contracts.push({module_id:id,symbol:name,...s.receipt(id,s.binding(id,name))});
}
const routes = {};
for (const name of ['A8','Q4','QI']) {
  const node = s.binding(60336, s.exported(60336, name).name);
  routes[name] = {value:s.literal(60336,node),...s.receipt(60336,node)};
}
const css = [...new Set(Object.values(manifest.files))]
  .filter(v => /^\/chroma-app\/dashboard\/static\/css\/(?:332|9700|6570)\.[a-f0-9]+\.chunk\.css$/.test(v))
  .map(relative => {
    const path = `.ref/applications${relative}`, text = fs.readFileSync(path,'utf8');
    return {path,sha256:hash(text),rules:parseCSS(text).filter(r =>
      /nav-tabs|apply-effect|box-group|chroma-app-dashboard-wrapper|introduction-banner|^\.items/.test(r.selector))};
  });
// A missing original remains missing: do not generate substitute timing/graphics.
const requiredMedia = [...new Set(css.flatMap(c => c.rules.flatMap(r =>
  [...r.declarations.matchAll(/url\((\/chroma-app\/dashboard\/static\/media\/dashboard_[^)]*(?:gray|animated)\.[^)]*\.svg)\)/g)]
    .map(m => m[1]))))];
const missingMedia = requiredMedia.map(url => ({url:`https://apps.razer.com${url}`,
  local:`.ref/applications${url}`, available:fs.existsSync(`.ref/applications${url}`)}));
const resources = JSON.parse(fs.readFileSync('assets/synapse/chroma-app-manifest.json','utf8'));
for (const resource of resources) {
  if (hash(fs.readFileSync(resource.source)) !== resource.source_sha256
      || hash(fs.readFileSync(resource.output)) !== resource.output_sha256) {
    throw Error(`Chroma resource hash differs: ${resource.output}`);
  }
}
const result = {date:'2026-10-05',verification:'Static AST/CSS parsing and source/output resource hashes only.',
  contracts,routes,css,missing_media:missingMedia,validated_resources:resources.length};
fs.writeFileSync('docs/re/chroma-review-fixes-2026-10-05-evidence.json', JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({contracts:contracts.length, routes:routes,
  css_rules:css.reduce((n,c)=>n+c.rules.length,0),missing_media:missingMedia.filter(x=>!x.available).length,
  validated_resources:resources.length}));
