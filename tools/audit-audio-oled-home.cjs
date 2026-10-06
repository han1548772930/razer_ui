// Product 1383 only. Static webpack AST/CSS receipts; no vendor code evaluation.
const fs=require('fs'),path=require('path');
const {Source,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),directory='.ref/devices/1383';
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
const manifest=JSON.parse(read(`${directory}/asset-manifest.json`));
const source=Object.create(Source.prototype);
source.directory=directory;
source.files=[...new Set(Object.values(manifest.files))].filter(file=>file.includes('/static/js/')&&file.endsWith('.js')).map(file=>`${directory}/${file.slice(file.indexOf('static/'))}`);
source.modules=new Map();source.texts=new Map();source.parsed=new Set();
const file=source.files.find(file=>file.endsWith('/6141.5d00192e.chunk.js'));
if(!file)throw Error('Current manifest no longer declares OLED chunk');
source.parse(file);
const owners=[...source.modules.values()].filter(module=>module.definitions.has('Dv'));
if(owners.length!==1)throw Error('Ambiguous HomeScreenDisplay owner');
const owner=owners[0].id;
const names=process.argv.slice(2);
if(names.length===1&&names[0]==='--check'){
  require('./prepare-audio-oled-home.cjs');
  process.exit(0);
}
if(names[0]==='--module'){const id=Number(names[1]);console.log(source.snippet(id,source.module(id).fn).slice(0,Number(names[2]||5000)));process.exit(0);}
if(names[0]==='--exports'){
 const id=Number(names[1]);for(const [name,node]of source.module(id).exports)console.log(JSON.stringify({name,source:source.snippet(id,node)}));process.exit(0);
}
if(names[0]==='--find'){
  for(const file of source.files)source.parse(file);
  for(const module of source.modules.values())for(const [symbol,node]of module.definitions){
    if(!node)continue;
    const snippet=source.snippet(module.id,node);
    if(snippet.includes(names[1]))console.log(JSON.stringify({module:module.id,symbol,path:module.file,offset:node.start,source:snippet.slice(0,Number(names[2]||1000))}));
  }
  process.exit(0);
}
const symbols=names.length?names:['Dv'];
const receipts=symbols.map(symbol=>{const [id,name]=symbol.includes(':')?symbol.split(':'):[owner,symbol];return {symbol,...source.receipt(Number(id),source.binding(Number(id),name))};});
const cssPaths=[...new Set(Object.values(manifest.files))].filter(file=>file.includes('/static/css/')&&file.endsWith('.css')).map(file=>`${directory}/${file.slice(file.indexOf('static/'))}`);
const css=cssPaths.flatMap(file=>parseCSS(read(file)).filter(rule=>/HomeScreenDisplay_|DisplayWidget_|CustomizeBanner_|CustomizeSystemInfo_|CustomizeAudioMeter_/.test(rule.selector)).map(rule=>({path:file,sha256:hash(read(file)),...rule})));
const result={product_id:1383,module:owner,offset_unit:'UTF-16 code units',receipts,css};
console.log(JSON.stringify(result,null,2));
