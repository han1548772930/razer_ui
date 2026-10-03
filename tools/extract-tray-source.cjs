// Current host and systray translations as literal Acorn data; no evaluation.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const folder = '.ref/applications/systray/systrayv2';
const receipts = [];
function read(file) {
  const text = fs.readFileSync(path.join(root, file), 'utf8');
  receipts.push({path: file, sha256: crypto.createHash('sha256').update(text).digest('hex')});
  return text;
}
function walk(n, f) {
  if (!n?.type) return;
  f(n);
  for (const v of Object.values(n)) {
    if (Array.isArray(v)) v.forEach(c => walk(c, f));
    else if (v?.type) walk(v, f);
  }
}
const key = n => n.name ?? n.value;
function literal(n) {
  if (n.type === 'Literal') return n.value;
  if (n.type === 'ObjectExpression') return Object.fromEntries(n.properties.map(p => {
    if (p.type !== 'Property' || p.computed) throw Error('Nonliteral key');
    return [key(p.key), literal(p.value)];
  }));
  throw Error(`Nonliteral ${n.type}`);
}
const host = acorn.parse(read('.ref/host-4.0.827/electron/constants.js'), {ecmaVersion:'latest'});
let hostStrings;
walk(host, n => { if (n.type === 'AssignmentExpression' && n.left.type === 'MemberExpression'
  && key(n.left.property) === 'string_translation') hostStrings = literal(n.right); });
if (!hostStrings?.en?.exit) throw Error('Missing host strings');
const manifest = JSON.parse(read(`${folder}/asset-manifest.json`));
const files = Object.values(manifest.files);
const chunk = id => `${folder}/${files.find(f => new RegExp(`/static/js/${id}\\.`).test('/'+f)).replace(/^\.\//,'')}`;
const mapSource = read(chunk(442));
const localeChunks = [...mapSource.matchAll(/(?:"([\w-]+)"|(\w+)):\(\)=>n\.e\((\d+)\)\.then\(n\.bind\(n,(\d+)\)\)/g)];
if (localeChunks.length !== 10) throw Error('Missing current locale map');
const popup = {};
for (const match of localeChunks) {
  const ast = acorn.parse(read(chunk(match[3])), {ecmaVersion:'latest'});
  let module;
  walk(ast, n => { if (n.type === 'Property' && key(n.key) === Number(match[4])) module = n.value; });
  if (!module) throw Error('Missing translation module');
  const vars = new Map(); let exports;
  walk(module, n => {
    if (n.type === 'VariableDeclarator' && n.id.type === 'Identifier') vars.set(n.id.name,n.init);
    if (n.type === 'CallExpression' && n.callee.type === 'MemberExpression' && key(n.callee.property)==='d') exports=n.arguments[1];
  });
  popup[(match[1] ?? match[2]).toLowerCase()] = Object.fromEntries(exports.properties.map(p => {
    const value = vars.get(p.value.body.name);
    if (value?.type !== 'Literal' || typeof value.value !== 'string') throw Error('Nonliteral translation');
    return [key(p.key),value.value];
  }));
}
const styles = read(`${folder}/static/css/554.7cdbd936.chunk.css`);
for (const fact of ['width:360px', 'height:60px', '.systray>.header-2', '.systray>.apps'])
  if (!styles.includes(fact)) throw Error(`Changed tray CSS: ${fact}`);
const viewSource = read(`${folder}/static/js/554.2573b048.chunk.js`);
const viewAst = acorn.parse(viewSource, {ecmaVersion:'latest'});
const branches = [];
walk(viewAst, node => {
  if(node.type !== 'FunctionDeclaration' || !['Re','oe','se','Oe'].includes(node.id?.name)) return;
  branches.push({symbol:node.id.name,offset:node.start,end:node.end,
    source:viewSource.slice(node.start,node.end)});
});
for(const symbol of ['Re','oe','se','Oe'])
  if(branches.filter(branch=>branch.symbol===symbol).length!==1) throw Error('Changed tray branch '+symbol);
const commonStyleFile = files.find(file=>/static\/css\/main\./.test(file));
const commonStyles = read(`${folder}/${commonStyleFile.replace(/^\.\//,'')}`);
if(!commonStyles.includes('body{color:#ccc;font-size:16px;line-height:1.22')) throw Error('Changed tray base font');
for(const fact of ['min-height:100%', '.systray>.apps>li:active>.icon{opacity:.3}',
  '.systray>.header-2{background-color:#222;justify-content:center;padding-bottom:1px'])
  if(!styles.includes(fact)) throw Error('Changed tray style '+fact);
read(`${folder}/static/js/main.9579c403.js`);
read('.ref/host-4.0.827/electron/lib/common.js');
read('.ref/host-4.0.827/electron/components/Tab/LeftSystray.js');
read('.ref/host-4.0.827/electron/main.js');
fs.writeFileSync(path.join(root,'src/shell/tray_strings.json'),JSON.stringify({host:hostStrings,popup},null,2)+'\n');
fs.writeFileSync(path.join(root,'docs/re/tray-source-receipts.json'),JSON.stringify({schema_version:1,receipts},null,2)+'\n');
fs.writeFileSync(path.join(root,'docs/re/tray-ui-current-evidence.json'),JSON.stringify({
  source:`${folder}/static/js/554.2573b048.chunk.js`,branches,
  status:'partial',pending:['guest/account session integration','widget and notification bodies',
    'multi-application launchers','non-Windows platform registration','runtime visual comparison']
},null,2)+'\n');
console.log(`Prepared ${Object.keys(popup).length} popup locales and current host translations.`);
