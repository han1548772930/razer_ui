// Current Macro tree metadata: AST-only receipts, no evaluation of vendor JS.
const fs = require('node:fs');
const path = require('node:path');
const acorn = require('acorn');
const {Source, walk, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const source = new Source('synapse/macro');
const file = source.files.find(file => /\/main\.[a-f0-9]+\.js$/.test(file));
const text = source.text(file);
const ast = acorn.parse(text, {ecmaVersion:'latest'});
const scopes = [];
walk(ast, node => {
  if (!/Function/.test(node.type) || node.body?.type !== 'BlockStatement') return;
  const definitions = new Map();
  for (const statement of node.body.body) {
    if (statement.type === 'VariableDeclaration') {
      for (const declaration of statement.declarations) {
        if (declaration.id.type === 'Identifier') definitions.set(declaration.id.name, declaration.init);
      }
    } else if (statement.id?.name) definitions.set(statement.id.name, statement);
  }
  if (['an','un','dn','Ct','Ys','Zs'].every(name => definitions.has(name))) scopes.push(definitions);
});
if (scopes.length !== 1) throw Error('Ambiguous Macro business closure');
const scope = scopes[0];
const receipt = node => ({file, sha256:hash(text), offset:node.start, end:node.end, source:text.slice(node.start,node.end)});
const bindings = Object.fromEntries(['kt','an','un','dn','Ct','Ys','$s','Ws','Vs','zs','Mn'].map(name => [name,receipt(scope.get(name))]));
const cases = {};
walk(scope.get('Zs'), node => {
  if (node.type === 'SwitchCase' && ['h.$g','h.uZ','h.Yt','h.fR'].includes(text.slice(node.test?.start,node.test?.end))) {
    cases[text.slice(node.test.start,node.test.end)] = receipt(node);
  }
});
if (Object.keys(cases).length !== 4) throw Error('Missing metadata reducer cases');
const exported = {};
for (const [module,name] of [[25572,'OM'],[25572,'iV'],[15030,'eh']]) {
  let node=source.exported(module,name);
  if (node.type==='Identifier') node=source.binding(module,node.name);
  exported[`${module}.${name}`]=source.receipt(module,node);
}
exported['15030.u']=source.receipt(15030,source.binding(15030,'u'));
const manifest=JSON.parse(fs.readFileSync(path.join(root,source.directory,'asset-manifest.json'),'utf8'));
const css=Object.values(manifest.files).filter(file=>/^\.\/static\/css\/(main|8190)\./.test(file) && file.endsWith('.css')).map(file=>{
  const name=source.directory+'/'+file.slice(2),value=fs.readFileSync(path.join(root,name),'utf8');
  return {file:name,sha256:hash(value),rules:parseCSS(value).filter(rule=>/structureFolder|marcoDropdown|profile-act|profile-del|guide_firstTimeCreateDot|#drag-image|\.s3-dropdown|MacroContent_(onboarding|macro_content|step_)|\.wrapper/.test(rule.selector))};
});
const result={route:'synapse/macro',method:'Acorn scoped business closure and webpack export getters; CSS rules retain enclosing media conditions; reference code is data only',bindings,cases,exported,css};
const target=path.join(root,'docs/re/macro-metadata-source.json');
if (process.argv.includes('--check')) {
  if (JSON.stringify(JSON.parse(fs.readFileSync(target,'utf8'))) !== JSON.stringify(result)) throw Error('Stale Macro metadata receipts');
} else fs.writeFileSync(target,JSON.stringify(result,null,2)+'\n');
console.log('Macro metadata receipts: 11 scoped bindings, 4 reducer cases, 4 module exports; source SHA-256 '+hash(text));
