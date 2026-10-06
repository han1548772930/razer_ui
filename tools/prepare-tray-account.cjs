// Parse the current systray bundle as data. Never import or execute vendor JS.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const root = path.resolve(__dirname, '..'), base = '.ref/applications/systray/systrayv2';
const check = process.argv.includes('--check');
const hash = s => crypto.createHash('sha256').update(s).digest('hex');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
function write(p, s) {
  if (check) { if (read(p) !== s) throw Error('Stale ' + p); }
  else fs.writeFileSync(path.join(root, p), s);
}
const manifest = JSON.parse(read(base + '/asset-manifest.json'));
function declared(suffix) {
  const matches = Object.values(manifest.files).filter(x => x.endsWith(suffix));
  if (matches.length !== 1) throw Error('Ambiguous manifest entry ' + suffix);
  return base + '/' + matches[0].replace(/^\.\//, '');
}
const js = declared('554.2573b048.chunk.js'), css = declared('554.7cdbd936.chunk.css');
const source = read(js), styles = read(css), ast = acorn.parse(source, {ecmaVersion:'latest'});
const wanted = new Set(['oe','se','he','Oe','_e','Re','ce','be','W','F']);
const nodes = [];
function walk(n) {
  if (!n?.type) return;
  if (['VariableDeclarator','FunctionDeclaration'].includes(n.type) && wanted.has(n.id?.name)
      && (n.id.name !== 'W' || source.slice(n.start,n.end).includes('hasItems:!1'))
      && (n.id.name !== 'F' || source.slice(n.start,n.end).includes('case N.OC:')))
    nodes.push({symbol:n.id.name, start:n.start, end:n.end, source:source.slice(n.start,n.end)});
  for (const v of Object.values(n)) if (Array.isArray(v)) v.forEach(walk); else if(v?.type) walk(v);
}
walk(ast);
for (const key of wanted) if (nodes.filter(n=>n.symbol===key).length!==1) throw Error('Missing/ambiguous ' + key);
const gear = nodes.find(n=>n.symbol==='ce');
const gearAst = acorn.parse(gear.source, {ecmaVersion:'latest'});
const attrs = new Map();
function literals(n) {
  if (!n?.type) return;
  if(n.type==='Property' && n.value.type==='Literal') attrs.set(n.key.name??n.key.value,n.value.value);
  for(const v of Object.values(n)) if(Array.isArray(v))v.forEach(literals);else if(v?.type)literals(v);
}
literals(gearAst);
for(const key of ['xmlns','width','height','viewBox','d','transform'])if(!attrs.has(key))throw Error('Missing gear '+key);
const esc = s => String(s).replaceAll('&','&amp;').replaceAll('"','&quot;').replaceAll('<','&lt;');
const svg = `<svg xmlns="${esc(attrs.get('xmlns'))}" width="${esc(attrs.get('width'))}" height="${esc(attrs.get('height'))}" viewBox="${esc(attrs.get('viewBox'))}"><path d="${esc(attrs.get('d'))}" transform="${esc(attrs.get('transform'))}"/></svg>\n`;
write('assets/synapse/tray-settings-current.svg',svg);
const rules = styles.split('}').filter(r=>/\.systray>\.header|\.navbar|\.notifications|\.btn-text|\.spinner-razer|^\.btn/.test(r)).map(r=>r+'}');
for(const fact of ['padding:9px 20px 8px','opacity:0','transition:opacity .1s ease-out','height:31px'])if(!rules.some(r=>r.includes(fact)))throw Error('Changed CSS '+fact);
const main = declared('main.9579c403.js'), mainSource=read(main), geometry=[];
function findGeometry(n){if(!n?.type)return;if(n.type==='FunctionDeclaration'&&n.id.name==='c'&&mainSource.slice(n.start,n.end).includes('dpiScaleY'))geometry.push({start:n.start,end:n.end,source:mainSource.slice(n.start,n.end)});for(const v of Object.values(n))if(Array.isArray(v))v.forEach(findGeometry);else if(v?.type)findGeometry(v);}
findGeometry(acorn.parse(mainSource,{ecmaVersion:'latest'}));
if(geometry.length!==1)throw Error('Missing geometry');
const report = {schema:1, manifest:{path:base+'/asset-manifest.json',sha256:hash(read(base+'/asset-manifest.json'))},js:{path:js,sha256:hash(source)},css:{path:css,sha256:hash(styles)},nodes,rules,geometry:{path:main,sha256:hash(mainSource),...geometry[0]},gear:{path:'assets/synapse/tray-settings-current.svg',sha256:hash(svg)},scope:'Account header, navigation and unloaded/empty notification branch. Populated notifications, widgets, real account transport and dynamic placement remain incomplete.'};
write('docs/re/tray-account-current-evidence.json',JSON.stringify(report,null,2)+'\n');
console.log('Current tray: 10 AST slices, CSS, gear and placement source validated.');
