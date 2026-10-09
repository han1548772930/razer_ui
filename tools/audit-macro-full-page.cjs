// Static current-source review. Never evaluates downloaded JavaScript.
const fs=require('node:fs'),path=require('node:path'),acorn=require('acorn');
const {Source,walk,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),source=new Source('synapse/macro');
const file=source.files.find(file=>/\/main\.[a-f0-9]+\.js$/.test(file));
const text=source.text(file),ast=acorn.parse(text,{ecmaVersion:'latest'}),scopes=[];
walk(ast,node=>{
  if(!/Function/.test(node.type)||node.body?.type!=='BlockStatement')return;
  const scope=new Map();
  for(const statement of node.body.body){
    if(statement.type==='VariableDeclaration')for(const declaration of statement.declarations){
      if(declaration.id.type==='Identifier')scope.set(declaration.id.name,declaration.init);
    }else if(statement.id?.name)scope.set(statement.id.name,statement);
  }
  if(['an','un','dn','Ct','Ys','Zs'].every(name=>scope.has(name)))scopes.push(scope);
});
if(scopes.length!==1)throw Error('Ambiguous Macro business closure');
const scope=scopes[0],receipt=node=>({file,sha256:hash(text),offset:node.start,end:node.end,source:text.slice(node.start,node.end)});
const bindings=Object.fromEntries(['Mn','hn','vn','Zs','$a'].map(name=>[name,receipt(scope.get(name))]));
const modules={};
for(const [module,names]of [[58190,['St','Fr','br','ja','Le','va','dn','Za','Zn']],[25572,['O','N']]]){
  for(const name of names)modules[`${module}.${name}`]=source.receipt(module,source.binding(module,name));
}
for(const [module,name]of [[25572,'FM'],[25572,'NH'],[47990,'Ay']]){
  let node=source.exported(module,name);
  if(node.type==='Identifier')node=source.binding(module,node.name);
  modules[`${module}.${name}`]=source.receipt(module,node);
}
for(const name of ['NCD','KNn']){
  let node=source.exported(37927,name);
  if(node.type==='Identifier')node=source.binding(37927,node.name);
  modules[`37927.${name}`]=source.receipt(37927,node);
}
const manifest=JSON.parse(fs.readFileSync(path.join(root,source.directory,'asset-manifest.json'),'utf8'));
const css=Object.values(manifest.files).filter(file=>/^\.\/static\/css\/(main|8190)\./.test(file)&&file.endsWith('.css')).map(file=>{
  const name=source.directory+'/'+file.slice(2),value=fs.readFileSync(path.join(root,name),'utf8');
  return {file:name,sha256:hash(value),rules:parseCSS(value).filter(rule=>/MacroContent_|randomized|\.navbar|\.profile-bar|\.module-nav|pairing|#line|active_name/.test(rule.selector))};
});
const nativePaths=['Cargo.toml','Cargo.lock','crates/razer-pages/src/features/macro_library.rs','crates/razer-app-pages/src/macro_page.rs',...fs.readdirSync(path.join(root,'crates/razer-shell/src/shell/macro_page'),{recursive:true}).filter(file=>file.endsWith('.rs')).map(file=>'crates/razer-app-pages/src/macro_page/'+file.replaceAll('\\','/'))].sort();
const native={verification:'Static reviewed file fingerprints only; not rendered/runtime equivalence',files:Object.fromEntries(nativePaths.map(file=>[file,hash(fs.readFileSync(path.join(root,file)))]))};
const result={route:'synapse/macro',method:'Current manifest + Acorn scoped bindings/export getters + static CSS; receipts do not certify implemented UI',bindings,modules,css,native};
const target=path.join(root,'docs/re/macro-full-page-source.json');
if(process.argv.includes('--check')){
  if(JSON.stringify(JSON.parse(fs.readFileSync(target,'utf8')))!==JSON.stringify(result))throw Error('Stale Macro full-page receipts');
}else fs.writeFileSync(target,JSON.stringify(result,null,2)+'\n');
console.log(`Current Macro full-page source: 5 business roots, ${Object.keys(modules).length} module bindings, CSS receipts verified`);
