// Current analog Keyboard Properties: AST/CSS/resource preparation only.
// No reference JavaScript, DLL export, application or test is executed.
const fs = require('fs'), path = require('path'), acorn = require('acorn');
const {Source, walk, hash, key} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..'), check = process.argv.includes('--check');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
function output(file, value) {
  const bytes = Buffer.isBuffer(value) ? value : Buffer.from(value);
  if (check) {
    if (!fs.readFileSync(path.join(root, file)).equals(bytes)) throw Error('Stale ' + file);
  } else fs.writeFileSync(path.join(root, file), bytes);
}
async function main() {
  const nativeReceipt='docs/re/evidence/sysutils-game-controller-ida.json';
  const native=JSON.parse(read(nativeReceipt)), binary=fs.readFileSync(path.join(root,native.source_path));
  if(hash(binary)!==native.input_sha256||!native.hexrays_available)throw Error('Native input hash/decompiler mismatch');
  const nt=binary.readUInt32LE(0x3c), sections=nt+24+binary.readUInt16LE(nt+20);
  function peBytes(rva,length) {
    for(let n=0;n<binary.readUInt16LE(nt+6);n++) {
      const header=sections+n*40, start=binary.readUInt32LE(header+12), size=Math.max(binary.readUInt32LE(header+8),binary.readUInt32LE(header+16));
      if(rva>=start&&rva+length<=start+size) {
        const offset=binary.readUInt32LE(header+20)+rva-start;return binary.subarray(offset,offset+length);
      }
    }
    throw Error('RVA outside native sections');
  }
  const fn=native.functions.find(fn=>fn.rva===0x3ffd0);
  if(!fn||hash(peBytes(fn.rva,fn.end_rva-fn.rva))!==fn.code_sha256||!fn.pseudocode.includes('WinExec("control joy.cpl", 5u)'))throw Error('Native command changed');
  const products = [];
  for (const product of [614,642,678,679,688]) {
    const directory = `.ref/devices/${product}`, manifestPath = directory + '/asset-manifest.json';
    const manifest = JSON.parse(read(manifestPath)), file = directory + '/' + manifest.files['main.js'].slice(2);
    const text = read(file), ast = acorn.parse(text,{ecmaVersion:'latest'});
    const source = Object.assign(Object.create(Source.prototype), {directory,
      files:[...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.js')).map(f=>directory+'/'+f.slice(2)),
      modules:new Map(),texts:new Map(),parsed:new Set()});
    const receipt = (node,kind) => ({kind,path:file,sha256:hash(text),offset:node.start,end:node.end,source:text.slice(node.start,node.end)});
    const classes = [];
    walk(ast,node=>{
      if (node.type !== 'ClassDeclaration') return;
      const body=text.slice(node.start,node.end);
      if(body.includes('OpenGameController()')&&body.includes('window-game-controller')&&body.includes('this.props.deviceType')) classes.push(node);
    });
    if(classes.length!==1)throw Error(`Re-audit analog class ${product}`);
    const component=classes[0], names=new Set([component.id.name]), receipts=[receipt(component,'analog properties component')];
    walk(ast,node=>{
      if(node.type==='AssignmentExpression'&&node.left?.property?.name==='OpenGameController'
        &&text.slice(node.start,node.end).includes('callElectronAction')) receipts.push(receipt(node,'current Electron action wrapper'));
    });
    if(receipts.filter(r=>r.kind==='current Electron action wrapper').length!==1)throw Error('Wrapper resolution changed');
    walk(ast,node=>{
      if(node.type==='VariableDeclarator'&&node.init?.type==='Identifier'&&names.has(node.init.name)) {
        names.add(node.id.name);receipts.push(receipt(node,'component alias'));
      }
    });
    let count=0;
    walk(ast,node=>{
      if(node.type!=='CallExpression'||!names.has(node.arguments[0]?.name)||node.arguments[1]?.type!=='ObjectExpression')return;
      if(!node.arguments[1].properties.some(p=>key(p.key)==='deviceType'&&p.value.value==='analog'))return;
      receipts.push(receipt(node,'actual analog page consumer'));count++;
    });
    if(count!==1)throw Error(`Re-audit active analog consumer ${product}: ${count}`);
    const labels={};
    for(const symbol of ['n$d','$83','Tpt','G2B']) {
      const exported=source.exported(54693,symbol);labels[symbol]=source.literal(54693,exported);
      receipts.push({kind:'label:'+symbol,...source.receipt(54693,source.binding(54693,exported.name))});
    }
    const css=[];
    for(const relative of [...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css'))) {
      const path=directory+'/'+relative.slice(2), text=read(path);
      const rules=parseCSS(text).filter(r=>/\.img-text.*(?:window-game-controller|external|windows)|\.widget .help|\.widget .title/.test(r.selector));
      if(rules.length)css.push({path,sha256:hash(text),rules});
    }
    if(!text.slice(component.start,component.end).includes('marginTop:"15px"'))throw Error('Second row source spacing changed');
    const consumer=receipts.find(r=>r.kind==='actual analog page consumer');
    const preceding=text.slice(consumer.offset-200,consumer.offset);
    const directions=[...preceding.matchAll(/direction:"(left|right)"/g)];
    if(!directions.length)throw Error('No current column placement');
    products.push({product_id:product,column:directions.at(-1)[1],receipts,labels,css});
  }
  const directory='.ref/devices/614', manifest=JSON.parse(read(directory+'/asset-manifest.json'));
  const relative=manifest.files['static/media/controller_icon.svg'], file=directory+'/'+relative.slice(2);
  if(!fs.existsSync(path.join(root,file))) {
    if(check)throw Error('Missing controller SVG input');
    const base=JSON.parse(read(directory+'/asset-manifest.json.http.json')).source_url;
    const url=new URL(relative,base), response=await fetch(url);
    if(!response.ok)throw Error(`Controller asset ${response.status}`);
    const bytes=Buffer.from(await response.arrayBuffer());
    if(!bytes.toString('utf8').includes('<svg')||bytes.toString('utf8').includes('<script'))throw Error('Unexpected SVG');
    fs.mkdirSync(path.dirname(path.join(root,file)),{recursive:true});output(file,bytes);
    output(file+'.http.json',JSON.stringify({source_url:url.href,final_url:response.url,http_status:response.status,
      fetched_at_utc:new Date().toISOString(),sha256:hash(bytes),bytes:bytes.length},null,2)+'\n');
  }
  const bytes=fs.readFileSync(path.join(root,file)), asset='assets/synapse/keyboard-game-controller.svg';output(asset,bytes);
  const host=[];
  for(const [path,token,tail] of [
    ['.ref/host-4.0.827/electron/main.js','case"OpenGameController"','return io.callDLL(n,i)'],
    ['.ref/host-4.0.827/electron/modules/sysutil/win/index.js','OpenGameController:["void",[]]','OpenGameController:["void",[]]']
  ]) {
    const text=read(path), offset=text.indexOf(token), end=text.indexOf(tail,offset)+tail.length;
    if(offset<0||end<=offset)throw Error('Current host route changed');
    host.push({path,sha256:hash(text),offset,end,source:text.slice(offset,end)});
  }
  for(const product of [642,678,679,688]) {
    const m=JSON.parse(read(`.ref/devices/${product}/asset-manifest.json`));
    if(m.files['static/media/controller_icon.svg']!==relative)throw Error('Controller hash varies; acquire separately');
  }
  output('docs/re/keyboard-analog-properties-current-evidence.json',JSON.stringify({schema_version:1,
    method:'Current AST/CSS static parsing; offsets are UTF-16; source JS never executed',products,host,
    asset:{source:file,output:asset,sha256:hash(bytes)},
    native_receipt:nativeReceipt,
    native_rva:0x3ffd0,command:'control joy.cpl',show:5,
    implementation:['crates/razer-pages/src/features/keyboard_properties.rs','crates/razer-platform/src/system.rs',
      'crates/razer-platform/src/platform/windows/system.rs'],runtime_acceptance:'not executed'},null,2)+'\n');
  console.log('Analog Keyboard Properties: 5 actual page consumers, AST labels/CSS and source controller SVG validated');
}
main().catch(e=>{console.error(e);process.exitCode=1;});
