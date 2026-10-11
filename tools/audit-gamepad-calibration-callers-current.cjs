// Targeted AST receipts for mounted caller trees and calibration dispatchers.
const fs=require('fs'),acorn=require('acorn'),crypto=require('crypto');
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
for(const [pid,symbols] of [[2676,['Qm','qm','Km','Zm','Xm','Im','lm','Hm','np','Mm','Am','Pm','ap','tp']],[2684,['yc','Tc','Wl','mc','Ic','cc','Zl','$l','rc','Oc','Rc']]]){
 const receipt=JSON.parse(fs.readFileSync(`docs/re/gamepad-${pid}-calibration-live-source.json`,'utf8'));
 const raw=fs.readFileSync(receipt.source,'utf8'),callers=[];
 for(const symbol of symbols){
  const escaped=symbol.replace(/[.*+?^${}()|[\]\\]/g,'\\$&');
  const re=new RegExp('(?:,|;|const )'+escaped+'=|(?:function|class) '+escaped+'(?=[ (])','g');
  const candidates=[...raw.matchAll(re)].filter(m=>Math.abs(m.index-receipt.component.offset)<100000);
  if(candidates.length!==1)throw Error(`Ambiguous ${pid}/${symbol}: ${candidates.length}`);
  const m=candidates[0],start=/^(class|function) /.test(m[0])?m.index:m.index+m[0].length;
  let ast=acorn.parseExpressionAt(raw,start,{ecmaVersion:'latest',preserveParens:true});
  if(ast.type==='SequenceExpression')ast=ast.expressions[0];
  callers.push({symbol,offset:start,end:ast.end,source:raw.slice(start,ast.end)});
 }
 const output={product_id:pid,source:receipt.source,sha256:hash(raw),callers};
 fs.writeFileSync(`docs/re/gamepad-${pid}-calibration-callers-live-source.json`,JSON.stringify(output,null,2)+'\n');
 console.log(JSON.stringify({product_id:pid,callers:callers.map(c=>({symbol:c.symbol,offset:c.offset,end:c.end}))}));
}
