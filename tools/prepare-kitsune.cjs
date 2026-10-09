// Current Kitsune source inspection and inert asset preparation; no vendor execution.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {Source,walk,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),directory='.ref/devices/4115';
const manifestFile=directory+'/asset-manifest.json',manifest=JSON.parse(fs.readFileSync(path.join(root,manifestFile),'utf8'));
// Source's literal/module parser is also applicable to a product manifest.
const s=Object.assign(Object.create(Source.prototype),{directory,files:[...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.js')).map(f=>directory+'/'+f.replace(/^\.\//,'')),modules:new Map(),texts:new Map(),parsed:new Set()});
const file=directory+'/static/js/main.1a52fc92.js',raw=s.text(file);
if(hash(raw)!=='03ad7f2d6eafc52a1766a7b850c0d03912344a5bfcd00695b62c5baff76254aa')throw Error('Kitsune current source changed; re-audit');
const wanted=new Set(['hP','MP','CP','PP','OP','SP','pP','mP','NP','RP','I_','u_','S_','O_','oP']);
const receipts=[];
walk(acorn.parse(raw,{ecmaVersion:'latest'}),node=>{
 if(node.start<6100000||!['VariableDeclarator','ClassDeclaration'].includes(node.type)||!wanted.has(node.id?.name))return;
 receipts.push({symbol:node.id.name,path:file,sha256:hash(raw),offset:node.start,end:node.end,source:raw.slice(node.start,node.end)});
});
for(const name of wanted)if(receipts.filter(r=>r.symbol===name).length!==1)throw Error('Ambiguous Kitsune binding '+name);
const symbols=['SYq','eyU','IlY','asZ','gJm','s1g','kM3','Obv','USp','eJd','CKV','oDm','wAK','Qeq','iYC','Jrg','e4Q','kto','q_4','WYV','lGq','orU','PDD'];
const labels=Object.fromEntries(symbols.map(name=>[name,s.literal(54693,s.exported(54693,name))]));
for(const locale of fs.readdirSync(path.join(root,'locales')).filter(f=>f.endsWith('.json'))){
 const strings=JSON.parse(fs.readFileSync(path.join(root,'locales',locale),'utf8'));
 for(const name of Object.values(labels))if(typeof strings[name]!=='string')throw Error(`Missing Kitsune label ${name} in ${locale}`);
}
for(const name of symbols)receipts.push({symbol:'label:'+name,...s.receipt(54693,s.binding(54693,s.exported(54693,name).name))});
const geometry=s.literal(76482,s.exported(76482,'h5'));
receipts.push({symbol:'geometry',...s.receipt(76482,s.binding(76482,s.exported(76482,'h5').name))});
const productContext=s.literal(58388,s.binding(58388,'n'));
const svgRequests=Object.fromEntries(Object.entries(productContext).filter(([key])=>key.includes('svg_prods')));
if(JSON.stringify(svgRequests)!==JSON.stringify({'./4115_0/svg_prods/0.svg':[19105,1,9105]}))throw Error('Re-audit Kitsune edition/layout assets');
receipts.push({symbol:'product-context',...s.receipt(58388,s.binding(58388,'n'))});
receipts.push({symbol:'product-svg-module',...s.receipt(19105,s.module(19105).fn)});
receipts.push({symbol:'radio-module:49412',...s.receipt(49412,s.module(49412).fn)});
const css=[];
for(const rel of [...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css'))){
 const file=directory+'/'+rel.replace(/^\.\//,''),text=fs.readFileSync(path.join(root,file),'utf8');
 css.push({path:file,sha256:hash(text),rules:parseCSS(text).filter(r=>/socd|mode-option|mode-switcher|config-wrapper|config-block|body-widgets|widget-col|radio|polling|h1-body|titleRow|dot-bg|dim-corner|widget.*help|widget.*tip/.test(r.selector)||r.selector==='.msg')});
}
const base=JSON.parse(fs.readFileSync(path.join(root,directory,'index.html.http.json'),'utf8')).source_url;
const assets=[['device','static/media/0.ff39cdc0.svg'],['socd','static/media/kitsune_dpad_socd.37442489.svg']].map(([name,relative])=>{
 if(!Object.values(manifest.files).includes('./'+relative))throw Error('Undeclared asset '+relative);
 const source=directory+'/'+relative,bytes=fs.readFileSync(path.join(root,source));
 return {name,source,url:new URL(relative,base).href,output:`assets/synapse/kitsune-${name}.svg`,sha256:hash(bytes)};
});
const check=process.argv.includes('--check');
const helpSource=directory+'/'+manifest.files['static/media/tooltip_questionmark.svg'].replace(/^\.\//,'');
const helpOutput='assets/synapse/automation-tooltip_questionmark.svg';
const helpBytes=fs.readFileSync(path.join(root,helpSource));
if(!helpBytes.equals(fs.readFileSync(path.join(root,helpOutput))))throw Error('Kitsune help icon differs from shared resource');
const shared_assets=[{source:helpSource,output:helpOutput,sha256:hash(helpBytes)}];
function output(file,text){const target=path.join(root,file);if(check){if(!fs.readFileSync(target).equals(Buffer.from(text)))throw Error('Stale '+file);}else fs.writeFileSync(target,text);}
for(const asset of assets)output(asset.output,fs.readFileSync(path.join(root,asset.source)));
output('assets/synapse/kitsune-embedded.rs','&[\n'+assets.map(a=>`    ("synapse/kitsune-${a.name}.svg", include_bytes!("kitsune-${a.name}.svg") as &[u8]),`).join('\n')+'\n]\n');
output('crates/razer-pages/src/features/kitsune_data.json',JSON.stringify({labels,geometry,assets:Object.fromEntries(assets.map(a=>[a.name,a.output.replace(/^assets\//,'')]))},null,2)+'\n');
output('docs/re/kitsune-current-evidence.json',JSON.stringify({product_id:4115,method:'Current source AST/CSS and inert SVG preparation only',manifest:{path:manifestFile,sha256:hash(fs.readFileSync(path.join(root,manifestFile)))},receipts,css,svgRequests,assets,shared_assets,status:'partial; source-derived root layout and local controls do not prove live device state or pixel fidelity'},null,2)+'\n');
console.log(`Kitsune: ${receipts.length} AST receipts, ${css.length} CSS files, ${assets.length} SVGs.`);
