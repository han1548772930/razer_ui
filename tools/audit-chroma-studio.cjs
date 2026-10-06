// Static exploration of the current independent Studio application; no vendor evaluation.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {Source,walk,key}=require('./webpack-source.cjs');
const s=new Source('synapse/chroma-studio');
const root=path.resolve(__dirname,'..');
const main=s.files.find(f=>/\/main\.[^.]+\.js$/.test(f));
const ast=acorn.parse(s.text(main),{ecmaVersion:'latest'});
const rootCall=s.text(main).lastIndexOf('.createRoot(');
let body;
walk(ast,n=>{
 if(/FunctionExpression$/.test(n.type)&&n.body.type==='BlockStatement'&&n.start<rootCall&&n.end>rootCall){
  if(!body||n.end-n.start<body.end-body.start)body=n;
 }
});
if(!body)throw Error('Missing startup scope');
const scope={id:-1,file:main,fn:body,definitions:new Map(),exports:new Map()};
for(const statement of body.body.body){
 if(statement.type==='VariableDeclaration')for(const d of statement.declarations){if(d.id.type==='Identifier')scope.definitions.set(d.id.name,d.init);}
 else if(['FunctionDeclaration','ClassDeclaration'].includes(statement.type))scope.definitions.set(statement.id.name,statement);
}
s.modules.set(-1,scope);
const args=process.argv.slice(2);
module.exports={source:s,startup:scope};
if(require.main===module){
if(args[0]==='--find'){
 for(const file of s.files)s.parse(file);
 for(const m of s.modules.values())for(const [symbol,node]of m.definitions){
  if(node&&s.snippet(m.id,node).includes(args[1]))console.log(JSON.stringify({module:m.id,symbol,...s.receipt(m.id,node),source:s.snippet(m.id,node).slice(0,Number(args[2]||700))}));
 }
}else if(args[0]==='--module'){
 const id=Number(args[1]);console.log(s.snippet(id,s.module(id).fn).slice(0,Number(args[2]||6000)));
}else if(args[0]==='--objects'){
 const id=Number(args[1]);for(const [name,n]of s.module(id).definitions)if(n&&['ObjectExpression','ArrayExpression'].includes(n.type))console.log(name,s.snippet(id,n).slice(0,Number(args[2]||700)));
}else if(args[0]==='--tail'){
 const [id,name]=args[1].split(':');const text=s.snippet(Number(id),s.binding(Number(id),name));console.log(text.slice(-Number(args[2]||6500)));
}else if(args[0]==='--exports'){
 const id=Number(args[1]);for(const [name,n]of s.module(id).exports)console.log(name,s.snippet(id,n));
}else{
 for(const input of args.length?args:['-1:rr']){
  const [id,name]=input.split(':');console.log(JSON.stringify({module:Number(id),symbol:name,...s.receipt(Number(id),s.binding(Number(id),name))}));
 }
}
}
