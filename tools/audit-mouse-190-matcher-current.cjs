// Current PID190 static text and AST audit. Never evaluate vendor JavaScript.
const fs=require('fs'),crypto=require('crypto'),acorn=require('../.work/local-ui/analyzer/node_modules/acorn');
const root='local-ui-reverse/source/official/apps.razer.com/synapse/products/190/ui/static';
const hash=s=>crypto.createHash('sha256').update(s).digest('hex');
const files=[],slices=[];
function read(relative,expected){const file=`${root}/${relative}`,bytes=fs.readFileSync(file),sha256=hash(bytes);if(expected&&sha256!==expected)throw Error('Current hash mismatch');files.push({file,sha256,bytes:bytes.length});return{file,source:bytes.toString('utf8')}}
function slice(input,name,start,end){const source=input.source.slice(start,end);slices.push({file:input.file,name,utf16_range:[start,end],utf8_byte_range:[Buffer.byteLength(input.source.slice(0,start)),Buffer.byteLength(input.source.slice(0,end))],slice_sha256:hash(source),source});}
function expr(input,name,start){let node=acorn.parseExpressionAt(input.source,start,{ecmaVersion:'latest'});if(node.type==='SequenceExpression')node=node.expressions[0];slice(input,name,node.start,node.end);return node;}
const main=read('js/main.81b09779.js','4c6d60b23bebcc9423a561aa86ebf6ed24cd9cbe0938aa13d7fac46022a4dc2f');
const css=read('css/main.7ce428ff.css','108e1e1f5035ab0746450b6744c7a9bd00f888bdd20980cde797c0b8b3ce5023');
for(const cls of ['AS','dS','uS','CO'])expr(main,`class ${cls}`,main.source.indexOf(`class ${cls} extends `));
for(const [name,needle] of Object.entries({reducer:'Ot=(e,E)=>',initial_state:'Tt={sensitivityMatcher:',profile_connect:'SS=(0,v.connect)',widget_connect:'NS=(0,v.connect)',delete_all:'OS=g().forwardRef',warning:'RS=e=>{let{onRetry',show_popup_action:'eS=e=>async E=>',new_dpi_action:'ES=e=>async E=>',calibration_states:'tS={READY:',slave_device_helper:'TO=function(){'})){
  const i=main.source.indexOf(needle);if(i<0)throw Error(`Missing ${name}`);expr(main,name,i+needle.indexOf('=')+1);
}
slice(main,'calibration and profile command literals',4602653,4602869);
slice(main,'multipairing observation',4539140,4539515);
slice(main,'multipairing mount gate',4543644,4543795);
for(const match of css.source.matchAll(/[^{}]+\{[^{}]*\}/g)){
  const selector=match[0].slice(0,match[0].indexOf('{')).trim();
  if(/sensitivity|popup-calibration|handmouse|mini-synapse|twomouse|calibration-info|dpi-info|calibration-reset|step-desc|dotted-line|warning-wapper|popup-warning|delete-all-popup|action-button/.test(selector))slice(css,`CSS ${selector}`,match.index,match.index+match[0].length);
}
const m=/(?:^|[,\{])4693:/.exec(main.source),constants=expr(main,'module4693 source localization constants',m.index+m[0].length);
const bindings={},constantExports={};
function walk(node){if(!node||typeof node!=='object')return;if(node.type==='VariableDeclarator'&&node.id.type==='Identifier'&&node.init?.type==='Literal')bindings[node.id.name]=node.init.value;if(node.type==='Property'&&node.value?.type==='ArrowFunctionExpression'&&node.value.body.type==='Identifier')constantExports[node.key.name]=node.value.body.name;for(const v of Object.values(node))if(Array.isArray(v))v.forEach(walk);else if(v&&typeof v==='object')walk(v)}walk(constants);
const used=[...new Set(slices.filter(x=>/^class (AS|dS)$|^delete_all$|^warning$/.test(x.name)).flatMap(x=>[...x.source.matchAll(/be\.([\w$]+)/g)].map(m=>m[1])))];
const localized_keys=Object.fromEntries(used.map(key=>[key,bindings[constantExports[key]]??null]));
const output='docs/re/mouse-190-matcher-current-source.json';
fs.writeFileSync(output,JSON.stringify({vendor_code_executed:false,offset_units:'Zero-based UTF16 / UTF8 byte ranges; end exclusive',files,localized_keys,slices,gaps:['Host/native DPI_MATCHER command implementation and state-response chain not audited by this tool','Rust sensitivity matcher and multipairing UI/observations not implemented','No runtime verification or real device operation']},null,2)+'\n');
console.log(JSON.stringify({output,slices:slices.length,localized_keys}));
