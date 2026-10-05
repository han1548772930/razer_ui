// Current Synapse/Chroma banner JSX, text aliases and matching CSS; data only.
const fs = require('fs'), path = require('path');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const records = [];
for (const [route, module, name] of [['synapse/dashboard',22534,'ji'], ['chroma-app/dashboard',62296,'Pn']]) {
  const source = new Source(route);
  if (!source.files.length) {
    const manifest = JSON.parse(fs.readFileSync(path.join(root, source.directory, 'asset-manifest.json'), 'utf8'));
    source.files = [...new Set(Object.values(manifest.files))].filter(f => f.startsWith('/chroma-app/dashboard/static/js/') && f.endsWith('.js')).map(f => `.ref/applications${f}`);
  }
  const node = source.binding(module, name), labels = {};
  walk(node, child => {
    if (child.type === 'Property' && key(child.key) === 'text' && child.value.type === 'MemberExpression') {
      const literal = source.literal(module, child.value);
      labels[source.snippet(module, child.value)] = literal;
    }
  });
  const cssDirectory = `${source.directory}/static/css`;
  const sheets = fs.readdirSync(path.join(root, cssDirectory)).filter(f => f.endsWith('.css')).map(f => {
    const file = `${cssDirectory}/${f}`, text = fs.readFileSync(path.join(root,file),'utf8');
    return {path:file, sha256:hash(text), rules:parseCSS(text).filter(r => r.selector.startsWith('.introduction-banner-container'))};
  }).filter(s => s.rules.length);
  records.push({route, component:source.receipt(module,node), labels, css:sheets});
}
const normalized = record => record.css.flatMap(s => s.rules.map(r => [r.selector,r.conditions,
  r.declarations.replaceAll('url(../../static/media/', 'url(ASSET/').replaceAll('url(/chroma-app/dashboard/static/media/', 'url(ASSET/')]));
if (JSON.stringify(normalized(records[0])) !== JSON.stringify(normalized(records[1]))) {
  const [a,b] = records.map(r => new Map(normalized(r).map(([s,c,d]) => [s+' '+JSON.stringify(c), d])));
  for (const selector of new Set([...a.keys(),...b.keys()])) if (a.get(selector)!==b.get(selector)) console.log(selector, a.get(selector)?.slice(0,250), b.get(selector)?.slice(0,250));
  throw Error('Banner CSS differs between apps; cannot share presentation');
}
if (JSON.stringify(Object.values(records[0].labels)) !== JSON.stringify(Object.values(records[1].labels))) throw Error('Banner text keys differ');
const manifest = JSON.parse(fs.readFileSync(path.join(root,'assets/synapse/manifest.json'),'utf8'));
const assets = ['chroma-big_synapse_4.svg','chroma-split_arrow.svg','chroma-introduction_background.png','chroma-introduction-logo.png','mapping-close.svg'].map(name => {
  const entry = manifest.entries.find(e => e.output === `assets/synapse/${name}`);
  if (!entry) throw Error(`Unprepared banner resource ${name}`);
  const outputHash = hash(fs.readFileSync(path.join(root,entry.output)));
  if (entry.sha256 !== outputHash) throw Error(`Banner asset drifted ${name}`);
  return entry;
});
const receipt = JSON.stringify({products:records, shared_css:true, shared_text_keys:true, assets},null,2)+'\n';
const destination = path.join(root,'docs/re/app-introduction-banner-current-evidence.json');
if (process.argv.includes('--check')) {
  if (fs.readFileSync(destination,'utf8') !== receipt) throw Error('Banner receipt drifted');
} else fs.writeFileSync(destination,receipt);
console.log(JSON.stringify({shared_css:true, labels:records[0].labels, assets:assets.length}));
