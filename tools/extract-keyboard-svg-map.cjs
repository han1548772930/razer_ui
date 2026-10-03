// Static literal map only. No downloaded function is invoked.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const root=path.resolve(__dirname,'..'),output={};
for(const file of fs.readdirSync(path.join(root,'assets/synapse/keyboard-products'))) {
  const raw=JSON.parse(fs.readFileSync(path.join(root,'assets/synapse/keyboard-products',file),'utf8'));
  const folder=path.join(root,`.ref/devices/${raw.product_id}/static/js`);
  const main=fs.readdirSync(folder).filter(n=>n.endsWith('.js')&&!n.startsWith('trans-')).find(n=>fs.readFileSync(path.join(folder,n),'utf8').includes('{KEY_ESC:"selEsc"'));
  if(!main)continue;
  const text=fs.readFileSync(path.join(folder,main),'utf8'),offset=text.indexOf('{KEY_ESC:"selEsc"');
  if(offset<0)continue;
  let node=acorn.parseExpressionAt(text,offset,{ecmaVersion:'latest'});const ids={};
  if(node.type==='SequenceExpression') node=node.expressions[0];
  for(const p of node.properties) {
    if(p.type==='SpreadElement') {
      if(p.argument.type!=='Identifier'||!text.slice(Math.max(0,offset-1500),offset).includes('CUSTOM_MAPPING_SVG:'+p.argument.name)) throw Error('Unresolved SVG map spread');
      continue; // CUSTOM_MAPPING_SVG merged below from the separately resolved CONFIG.
    }
    if(p.type!=='Property'||p.value.type!=='Literal'||typeof p.value.value!=='string') throw Error(raw.product_id+': Nonliteral SVG identity map '+text.slice(p.start,p.end));
    ids[p.key.name??p.key.value]=p.value.value;
  }
  output[raw.product_id]={source:`.ref/devices/${raw.product_id}/static/js/${main}`,offset,ids:{...ids,...raw.config.CUSTOM_MAPPING_SVG}};
}
fs.writeFileSync(path.join(root,'assets/synapse/keyboard-svg-ids.json'),JSON.stringify(output));
console.log(Object.keys(output).length+' literal SVG identity maps');
