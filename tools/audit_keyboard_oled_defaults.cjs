// Current JS reducer defaults, decoded as AST literals; no vendor code executes.
const fs=require('fs'),crypto=require('crypto'),acorn=require('acorn');
const file='crates/razer-pages/src/features/keyboard_products_data.json';
const products=JSON.parse(fs.readFileSync(file,'utf8')),receipts=[];
const key=n=>n?.name??n?.value;
function literal(n){if(n.type==='Literal')return n.value;if(n.type==='UnaryExpression'&&n.operator==='!')return !literal(n.argument);if(n.type==='ObjectExpression')return Object.fromEntries(n.properties.map(p=>[key(p.key),literal(p.value)]));throw Error('Nonliteral reducer default '+n.type);}
for(const product of products){
 const root=`local-ui-reverse/source/official/apps.razer.com/synapse/products/${product.product_id}/ui/static/js/`;
 const path=root+fs.readdirSync(root).find(n=>n.startsWith('main.')),bytes=fs.readFileSync(path),s=bytes.toString('utf8');
 const match=/\b([\w$]+)=\{oledLowBatteryWarningDisplay:/.exec(s);
 if(!match)continue;
 const begin=match.index+match[0].indexOf('{'),node=acorn.parseExpressionAt(s,begin,{ecmaVersion:'latest'}),value=literal(node);
 const reducer=new RegExp('oledLowBatteryWarningDisplayReducer:function\\(\\)\\{let \\w+=arguments.length>0&&void 0!==arguments\\[0\\]\\?arguments\\[0\\]:'+match[1].replace(/\$/g,'\\$')).exec(s);
 if(!reducer)throw Error('Default is not connected to source reducer '+product.product_id);
 product.controls.defaults.oledLowBatteryWarningDisplay=value.oledLowBatteryWarningDisplay;
 receipts.push({product_id:product.product_id,path,sha256:crypto.createHash('sha256').update(bytes).digest('hex'),default:{byte_offset:Buffer.byteLength(s.slice(0,node.start)),source:s.slice(node.start,node.end),value},reducer:{byte_offset:Buffer.byteLength(s.slice(0,reducer.index)),source:s.slice(reducer.index,reducer.index+1400)}});
}
fs.mkdirSync('.work/keyboard-oled-defaults',{recursive:true});
fs.writeFileSync('.work/keyboard-oled-defaults/receipts.json',JSON.stringify(receipts,null,2));
fs.writeFileSync('docs/re/keyboard-oled-defaults-current-source.json',JSON.stringify({method:'Fresh current direct JS bytes; pure AST literal decode; default symbol is cross-checked against its reducer initial-state expression. Offset unit is UTF-8 bytes. This is reducer state acquisition, not UI mount or device acceptance.',receipts},null,2)+'\n');
if(process.argv.includes('--prepare'))fs.writeFileSync(file,JSON.stringify(products));
console.log(JSON.stringify(receipts));
