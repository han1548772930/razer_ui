// Serialize only the literal SVG tree in this product's current renderer.
// Acorn parsing never executes downloaded JavaScript or React components.
const fs=require('fs'), acorn=require('acorn');
const {walk,hash}=require('./webpack-source.cjs');
const receipt=JSON.parse(fs.readFileSync('docs/re/armory-product-roots-current-evidence.json','utf8'));
const product=receipt.products.find(p=>p.product_id===3907);
const text=fs.readFileSync(product.source,'utf8');
if(hash(text)!==product.sha256)throw Error('Current source digest changed');
const escape=v=>String(v).replaceAll('&','&amp;').replaceAll('"','&quot;').replaceAll('<','&lt;');
const attrName=k=>({className:'class',strokeWidth:'stroke-width',strokeLinecap:'stroke-linecap',
  strokeLinejoin:'stroke-linejoin',clipPath:'clip-path',fillRule:'fill-rule',clipRule:'clip-rule',
  xmlnsXlink:'xmlns:xlink'}[k]||k);
function svg(n){
  if(!n||n.type==='ConditionalExpression')return '';
  if(n.type==='LogicalExpression')return svg(n.right);
  if(n.type==='AssignmentExpression')return svg(n.right);
  if(n.type==='Literal')return n.value==null?'':escape(n.value);
  if(n.type!=='CallExpression'||n.callee.property?.name!=='createElement')throw Error('Nonliteral SVG child '+n.type);
  const [tag,props,...children]=n.arguments;
  if(tag.type!=='Literal')throw Error('Nonliteral SVG tag');
  const obj=props.type==='CallExpression'?props.arguments[0]:props;
  let attrs='';
  if(obj.type==='ObjectExpression')for(const p of obj.properties){
    const k=p.key.name||p.key.value;
    if(['ref','aria-labelledby','nonce'].includes(k))continue;
    if(p.value.type!=='Literal')throw Error('Nonliteral SVG attribute '+k);
    attrs+=` ${attrName(k)}="${escape(p.value.value)}"`;
  }else if(!(obj.type==='Literal'&&obj.value==null))throw Error('Nonliteral SVG props');
  return `<${tag.value}${attrs}>${children.map(svg).join('')}</${tag.value}>`;
}
const records=[];
for(const [symbol,name]of [['FP','cpu'],['GP','gpu'],['nm','reset']]){
  const component=product.components.find(c=>c.symbol===symbol);
  if(text.slice(component.offset,component.end)!==component.source)throw Error('Stale component '+symbol);
  const ast=acorn.parse('('+component.source+')',{ecmaVersion:'latest'});
  let root;
  walk(ast,n=>{if(n.type==='CallExpression'&&n.callee.property?.name==='createElement'&&n.arguments[0]?.value==='svg')root=n});
  if(!root)throw Error('Missing SVG '+symbol);
  const rendered=svg(root)+'\n',output=`assets/synapse/armory-cooling-${name}.svg`;
  if(process.argv.includes('--check')){
    if(fs.readFileSync(output,'utf8')!==rendered)throw Error('Stale icon '+name);
  }else fs.writeFileSync(output,rendered);
  records.push({asset:output,sha256:hash(rendered),source:product.source,source_sha256:product.sha256,
    component:symbol,offset:component.offset,end:component.end,method:'Static literal createElement SVG serialization'});
}
// The delete glyph is emitted through the JSX runtime rather than createElement.
const deletion=product.components.find(c=>c.symbol==='Vp');
const deletePath='M7.916 11.833H2.083a.583.583 0 0 1-.583-.583v-7a.583.583 0 0 1 .583-.583h5.833a.583.583 0 0 1 .584.583v7a.583.583 0 0 1-.584.583m1.167-10.5H6.75L6.166.167H3.833L3.25 1.333H.916a.583.583 0 0 0 0 1.167h8.167a.583.583 0 0 0 0-1.167';
if(text.slice(deletion.offset,deletion.end)!==deletion.source||!deletion.source.includes(JSON.stringify(deletePath)))
  throw Error('Delete SVG literal changed');
const deleteSvg=`<svg xmlns="http://www.w3.org/2000/svg" width="10" height="12" fill="none" viewBox="0 0 10 12"><path fill="#000" d="${deletePath}"/></svg>\n`;
const deleteOutput='assets/synapse/armory-cooling-delete.svg';
if(process.argv.includes('--check')){if(fs.readFileSync(deleteOutput,'utf8')!==deleteSvg)throw Error('Stale delete glyph');}
else fs.writeFileSync(deleteOutput,deleteSvg);
records.push({asset:deleteOutput,sha256:hash(deleteSvg),source:product.source,source_sha256:product.sha256,
  component:'Vp',offset:deletion.offset,end:deletion.end,method:'Exact current JSX-runtime SVG literal; path verified against renderer'});
// Same current content fingerprint, recovered offline from the current Macro
// cache. MD4 naming is also calibrated by tools/recover-current-media.cjs.
const warningCandidate='.ref/applications/synapse/macro/static/media/warning.ad3f47f8.svg';
const warningBytes=fs.readFileSync(warningCandidate);
const warningMd4=require('crypto').createHash('md4').update(warningBytes).digest('hex');
if(!warningMd4.startsWith('ad3f47f8'))throw Error('Shared warning content fingerprint differs');
const cssPath=product.css[0].path,cssText=fs.readFileSync(cssPath,'utf8');
if(!cssText.includes('static/media/warning.ad3f47f8.svg'))throw Error('Current warning CSS URL changed');
const warningOutput='assets/synapse/armory-cooling-warning.svg';
if(process.argv.includes('--check')){if(!fs.readFileSync(warningOutput).equals(warningBytes))throw Error('Warning drift');}
else fs.writeFileSync(warningOutput,warningBytes);
records.push({asset:warningOutput,sha256:hash(warningBytes),source:warningCandidate,
  source_sha256:hash(warningBytes),component:'current CSS .warning:before',offset:cssText.indexOf('static/media/warning.ad3f47f8.svg'),
  css:cssPath,css_sha256:hash(cssText),md4:warningMd4,
  method:'Offline current CSS content-fingerprint recovery, not a live HTTP success'});
const output='docs/re/armory-cooling-icons-current-evidence.json',rendered=JSON.stringify(records,null,2)+'\n';
if(process.argv.includes('--check')){if(fs.readFileSync(output,'utf8')!==rendered)throw Error('Stale icon receipt');}
else fs.writeFileSync(output,rendered);
const manifestPath='assets/synapse/manifest.json';
const manifest=JSON.parse(fs.readFileSync(manifestPath,'utf8'));
const embeddedPath='assets/synapse/embedded.rs';
let embedded=fs.readFileSync(embeddedPath,'utf8');
for(const record of records){
  const name=record.asset.split('/').at(-1), asset='synapse/'+name;
  const entry={source:record.source,output:record.asset,source_sha256:record.source_sha256,
    sha256:record.sha256,component:record.component,source_offset:record.offset,
    source_kind:'inline_svg_literal',transform:record.method};
  const existing=manifest.entries.findIndex(e=>e.output===record.asset);
  if(process.argv.includes('--check')){
    if(existing<0||manifest.entries[existing].sha256!==record.sha256||!embedded.includes('"'+asset+'"'))
      throw Error('Icon registration missing '+asset);
  }else{
    if(existing<0)manifest.entries.push(entry);else manifest.entries[existing]=entry;
    if(!embedded.includes('"'+asset+'"')){
      embedded=embedded.replace(/\]\s*$/,`    ("${asset}", include_bytes!("${name}") as &[u8]),\n]\n`);
    }
  }
}
if(!process.argv.includes('--check')){
  fs.writeFileSync(manifestPath,JSON.stringify(manifest,null,2)+'\n');
  fs.writeFileSync(embeddedPath,embedded);
}
console.log('Cooling Pad: four current inline SVGs and one fingerprint-matched warning verified.');
