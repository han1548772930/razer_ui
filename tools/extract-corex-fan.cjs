// Current Core X V2 data only; downloaded modules are never evaluated.
const fs=require('fs'),path=require('path'),crypto=require('crypto'),acorn=require('acorn');
const root=path.resolve(__dirname,'..'), read=p=>fs.readFileSync(path.join(root,p),'utf8');
const hash=s=>crypto.createHash('sha256').update(s).digest('hex');
const product=JSON.parse(read('docs/re/accessory-system-source.json')).products.find(p=>p.product_id===3921);
for(const file of product.source_files) if(hash(read(file.path))!==file.sha256) throw Error('Changed '+file.path);
const main=read(product.source), chunkPath='.ref/devices/3921/static/js/684.ebbca785.chunk.js',chunk=read(chunkPath);
function walk(n,fn){if(!n?.type||fn(n)===false)return;for(const v of Object.values(n)){if(Array.isArray(v))v.forEach(x=>walk(x,fn));else if(v?.type)walk(v,fn)}}
function decode(n,defs=new Map()){
 if(n.type==='Literal')return n.value;
 if(n.type==='Identifier'&&defs.has(n.name))return decode(defs.get(n.name),defs);
 if(n.type==='UnaryExpression'&&n.operator==='!')return !decode(n.argument,defs);
 if(n.type==='ArrayExpression')return n.elements.map(n=>decode(n,defs));
 if(n.type==='ObjectExpression')return Object.fromEntries(n.properties.map(p=>[p.key.name??p.key.value,decode(p.value,defs)]));
 throw Error('Nonliteral '+n.type);
}
let constants;
walk(acorn.parse(main,{ecmaVersion:'latest'}),n=>{if(n.type==='Property'&&n.key.value===2502){constants=n.value;return false}});
const defs=new Map();walk(constants.body,n=>{if(/Function/.test(n.type))return false;if(n.type==='VariableDeclarator')defs.set(n.id.name,n.init)});
const modes=decode(defs.get('o')),max=decode(defs.get('s'));
if(modes.Auto!=='Auto'||modes.Smart!=='Manual'||max!==2800)throw Error('Fan modes changed');
if(!main.includes('try{c=n(8193).DEFAULT_CURVES}catch(C){c=n(2502).zO}'))throw Error('Curve source changed');
let translations;
walk(acorn.parse(chunk,{ecmaVersion:'latest'}),n=>{if(n.type==='ObjectExpression'&&n.properties.some(p=>p.key.name==='en')&&chunk.slice(n.start,n.end).includes('SYSTEM_FAN_CONTROL:')){translations=decode(n);return false}});
if(!translations?.en?.SYSTEM_FAN_CONTROL)throw Error('Missing translations');
const curves=product.config.DEFAULT_CURVES;
const state=product.unresolved.find(x=>x.name==='S'&&x.source.includes('smartFanCurveSettings'));
const expected='{data:{fanMode:r.l8.Smart,activeManualPreset:"quiet",manualPresets:['+Object.keys(curves).map(mode=>`{mode:"${mode}",smartFanCurve:{activeTemperatureMode:"gpu",gpu:c.${mode}.gpu,chassis:c.${mode}.chassis}}`).join(',')+']},smartFanCurveSettings:{showInRPM:!1,showInFahrenheit:!1,currentPoint:{temp:0}}}';
if(state.source!==expected)throw Error('Reducer defaults changed');
for(const text of ['t.length<=2','l>=20','width:820,height:410,xPadding:75,yPadding:75','(e-t.min)/(t.max-t.min)*100','gpu:[...n[t.mode].gpu],chassis:[...n[t.mode].chassis]']){
 if(!(main+chunk).includes(text))throw Error('Chart contract changed: '+text);
}
const output={product_id:3921,modes,min_rpm:1000,max_rpm:max,curves,translations,
 initial:{fanMode:modes.Smart,activeManualPreset:'quiet',manualPresets:Object.entries(curves).map(([mode,curve])=>({mode,smartFanCurve:{activeTemperatureMode:'gpu',...curve}}))},
 axes:Object.fromEntries([['gpu','le'],['chassis','oe']].map(([key,symbol])=>[key,product.arrays.find(x=>x.symbol===symbol).value])),
 source_files:product.source_files,rule:'2..20 points; fixed temperature on vertical drag; speed clamped between neighbours; reset both targets in active preset',
 state_source:{path:state.path,offset:state.offset,end:state.end,sha256:hash(state.source)}};
fs.writeFileSync(path.join(root,'src/features/corex_fan_data.json'),JSON.stringify(output,null,2)+'\n');
console.log('Extracted Core X V2 fan defaults, axes, modes and translations.');
