// Targeted parsing only. No imported/downloaded JavaScript is evaluated.
const fs=require('fs'),acorn=require('acorn'),crypto=require('crypto');
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
function declaration(raw,symbol,near){
 const escaped=symbol.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
 const re=new RegExp('(?:,|;|const )'+escaped+'=|(?:function|class) '+escaped+'(?=[ (])','g');
 const candidates=[...raw.matchAll(re)].filter(m=>Math.abs(m.index-near)<150000);
 if(candidates.length!==1)throw Error(`Ambiguous ${symbol}: ${candidates.length}`);
 const m=candidates[0],start=/^(class|function) /.test(m[0])?m.index:m.index+m[0].length;
 let ast=acorn.parseExpressionAt(raw,start,{ecmaVersion:'latest',preserveParens:true});
 if(ast.type==='SequenceExpression')ast=ast.expressions[0];
 return {symbol,offset:start,end:ast.end,source:raw.slice(start,ast.end),ast};
}
for(const [pid,initial] of [[2676,['VL','FL','ep','cm','pm','Xm','Wm','wL','HL','GL','vL','yL','BL']],[2684,['Kl','Sl','xc','Xl','ac','Dc','vc','Sc','vl','ml','_l','hl','El','gl']]]){
 const parent=JSON.parse(fs.readFileSync(`docs/re/gamepad-${pid}-calibration-live-source.json`,'utf8'));
 const raw=fs.readFileSync(parent.source,'utf8'),queue=[...initial],receipts=[];
 for(let i=0;i<queue.length;i++){
  const receipt=declaration(raw,queue[i],parent.component.offset),ast=receipt.ast;
  if(ast.type==='Identifier'&&!queue.includes(ast.name))queue.push(ast.name);
  if(ast.type==='CallExpression'&&ast.arguments[0]?.type==='Identifier'&&!queue.includes(ast.arguments[0].name))queue.push(ast.arguments[0].name);
  delete receipt.ast;receipts.push(receipt);
 }
 fs.writeFileSync(`docs/re/gamepad-${pid}-trigger-resources-live-source.json`,JSON.stringify({product_id:pid,source:parent.source,sha256:hash(raw),receipts},null,2)+'\n');
 console.log(pid,receipts.map(r=>r.symbol+':'+(r.end-r.offset)).join(' '));
 for(const symbol of initial.slice(7))console.log(symbol,receipts.find(r=>r.symbol===symbol).source);
}
