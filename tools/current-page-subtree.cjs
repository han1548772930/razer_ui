// Shared static verification of already extracted current component subtrees.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {hash,walk}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..');
function subtree(page,target,rootName=/\.jsx(?:s)?\)\(([^,]+)/.exec(page.component??'')?.[1]){
 const source=fs.readFileSync(path.join(root,page.path),'utf8');
 if(hash(source)!==page.sha256)throw Error('Changed current page '+page.path);
 const components=new Map(page.components.map(c=>[c.symbol,c])),edges=new Map();
 for(const c of page.components){
  const refs=new Set((c.jsx??[]).map(x=>x.component).filter(x=>components.has(x)));
  if(components.has(c.source))refs.add(c.source);
  const alias=/\(([\w$]+)\)\)*$/.exec(c.source)?.[1];
  if(alias&&components.has(alias))refs.add(alias);
  edges.set(c.symbol,[...refs]);
 }
 const queue=[[rootName]],seen=new Set();let mount;
 while(queue.length){const chain=queue.shift(),name=chain.at(-1);if(seen.has(name))continue;seen.add(name);
  if(name===target.symbol){mount=chain;break;}
  for(const next of edges.get(name)??[])queue.push([...chain,next]);
 }
 if(!mount)return null;
 const receipts=mount.map(symbol=>{
  const c=components.get(symbol),file=c.path??page.path;
  const text=file===page.path?source:fs.readFileSync(path.join(root,file),'utf8');
  if(text.slice(c.offset,c.end)!==c.source)throw Error('Changed current subtree '+symbol+' '+file);
  const ast=acorn.parseExpressionAt(text,c.offset,{ecmaVersion:'latest'});
  const node=ast.type==='SequenceExpression'?ast.expressions[0]:ast;
  if(node.start!==c.offset||node.end!==c.end)throw Error('AST boundary '+symbol+' '+file);
  return {symbol,path:file,sha256:hash(text),offset:c.offset,end:c.end,source:c.source};
 });
 return {mount,receipts};
}
function componentAst(component){return acorn.parseExpressionAt(component.source,0,{ecmaVersion:'latest'});}
module.exports={subtree,componentAst,walk,hash};
