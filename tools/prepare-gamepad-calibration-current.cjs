// Static current-source extraction. Vendor code is parsed, never evaluated.
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const acorn = require('acorn');
const {execFileSync} = require('child_process');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const hash = b => crypto.createHash('sha256').update(b).digest('hex');
const base = 'local-ui-reverse/source/official/apps.razer.com/synapse/products';
const products = [[2629,'nM'],[2636,'XM'],[2647,'Um'],[2650,'sm'],[2676,'hm'],[2684,'lc'],[4133,'iM'],[4144,null]];
const embedded = [], receipts = [];
function expr(raw, symbol, near) {
  const re = new RegExp('(?:,|;|const )' + symbol + '=','g');
  re.lastIndex = Math.max(0, near - 25000);
  const m = re.exec(raw);
  if (!m || m.index > near) throw Error('Missing component '+symbol);
  const start = m.index+m[0].length;
  let ast = acorn.parseExpressionAt(raw,start,{ecmaVersion:'latest',preserveParens:true});
  if(ast.type==='SequenceExpression')ast=ast.expressions[0];
  return {symbol, offset:start, end:ast.end, source:raw.slice(start,ast.end)};
}
for (const [pid,symbol] of products) {
  const dir = `${base}/${pid}/ui`, jsdir = path.join(root,dir,'static/js');
  const filename = fs.readdirSync(jsdir).find(f => pid === 2684 ? /^8169\..*\.js$/.test(f) : /^main\..*\.js$/.test(f));
  if (!filename) throw Error('Missing main '+pid);
  const source = `${dir}/static/js/${filename}`, raw = fs.readFileSync(path.join(root,source),'utf8');
  const near = raw.indexOf('prodImage-calibration');
  let component;
  if (symbol) component = expr(raw,symbol,near);
  else {
    // Find the enclosing initializer by parsing only candidates immediately
    // before the distinctive mounted visualization, never the whole bundle.
    const head=raw.slice(Math.max(0,near-25000),near), offset=Math.max(0,near-25000);
    const candidates=[...head.matchAll(/,([A-Za-z_$][\w$]*)=(?:\(\)|[A-Za-z_$][\w$]*)=>\{/g)].reverse();
    for(const m of candidates){
      const start=offset+m.index+m[0].indexOf('=')+1;
      let ast=acorn.parseExpressionAt(raw,start,{ecmaVersion:'latest',preserveParens:true});
      if(ast.type==='SequenceExpression')ast=ast.expressions[0];
      if(ast.end>near){component={symbol:m[1],offset:start,end:ast.end,source:raw.slice(start,ast.end)};break;}
    }
    if(!component)throw Error('Missing enclosing SVG wizard '+pid);
  }
  const cssdir=path.join(root,dir,'static/css'), css=[];
  for(const f of fs.readdirSync(cssdir).filter(f=>f.endsWith('.css'))){
    const text=fs.readFileSync(path.join(cssdir,f),'utf8');
    const rules=parseCSS(text).filter(r=>/calibration|joystick-direction-overlay|thumbstick-simulator|lb-overlay|popup-widget|choose-a-mat|linked-games-popup/.test(r.selector));
    if(rules.length)css.push({path:`${dir}/static/css/${f}`,sha256:hash(text),rules});
  }
  const images=[];
  const imageMatches=[...raw.matchAll(new RegExp('"\\./'+pid+'_(\\d+)/img_prods/0-3x\\.png":\\[(\\d+),1,(\\d+)\\]','g'))];
  const seen=new Set();
  for(const m of imageMatches){
    if(seen.has(m[1]))continue;seen.add(m[1]);
    const chunk=fs.readdirSync(jsdir).find(f=>f.startsWith(m[3]+'.')&&f.endsWith('.js'));
    if(!chunk)throw Error('Missing image chunk '+pid+'/'+m[3]);
    const chunkbytes=fs.readFileSync(path.join(jsdir,chunk));
    const media=/"(static\/media\/[^" ]+\.avif)"/.exec(chunkbytes.toString())[1];
    const input=path.join(root,dir,media), bytes=fs.readFileSync(input);
    const output=`assets/synapse/gamepad-${pid}-calibration-edition-${m[1]}.png`;
    const python=process.env.RAZER_RESOURCE_PYTHON || 'C:/Users/15487/AppData/Local/Python/pythoncore-3.14-64/python.exe';
    const png=execFileSync(python,['-c','from PIL import Image; import sys; Image.open(sys.argv[1]).convert("RGBA").save(sys.stdout.buffer,format="PNG")',input],{env:{...process.env,PYTHONPATH:path.join(root,'.work/audio-image-deps')},maxBuffer:16*1024*1024});
    fs.writeFileSync(path.join(root,output),png);
    embedded.push(output);
    images.push({edition:Number(m[1]),source:`${dir}/${media}`,sha256:hash(bytes),module:Number(m[2]),chunk:`${dir}/static/js/${chunk}`,chunk_sha256:hash(chunkbytes),output,output_sha256:hash(png)});
  }
  const svg=fs.readdirSync(path.join(root,dir,'static/media')).filter(f=>/^Calibration\..*\.svg$/.test(f));
  for(const filename of svg){
    const input=path.join(root,dir,'static/media',filename), bytes=fs.readFileSync(input);
    const output=`assets/synapse/gamepad-${pid}-calibration-source.svg`;
    fs.writeFileSync(path.join(root,output),bytes);embedded.push(output);
    images.push({source:`${dir}/static/media/${filename}`,sha256:hash(bytes),output,output_sha256:hash(bytes)});
  }
  if(pid===2676||pid===2684){
    const relative='static/media/icon_close.55fe41f1.svg',bytes=fs.readFileSync(path.join(root,dir,relative));
    const output=`assets/synapse/gamepad-${pid}-calibration-close.svg`;
    fs.writeFileSync(path.join(root,output),bytes);embedded.push(output);
    images.push({source:`${dir}/${relative}`,sha256:hash(bytes),output,output_sha256:hash(bytes)});
  }
  const receipt={product_id:pid,source,sha256:hash(raw),component,css,images};
  fs.writeFileSync(path.join(root,`docs/re/gamepad-${pid}-calibration-live-source.json`),JSON.stringify(receipt,null,2)+'\n');
  receipts.push({product_id:pid,symbol:component.symbol,offset:component.offset,end:component.end,images:images.length});
}
fs.writeFileSync(path.join(root,'assets/synapse/gamepad-calibration-current-embedded.rs'),'&[\n'+embedded.map(output=>`    ("synapse/${path.basename(output)}", include_bytes!("${path.basename(output)}") as &[u8]),`).join('\n')+'\n]\n');
console.log(JSON.stringify(receipts));
