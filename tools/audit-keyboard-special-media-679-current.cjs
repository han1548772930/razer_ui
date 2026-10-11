// Current raw source AST/text only. Never executes downloaded JavaScript.
const fs=require('fs'),acorn=require('acorn'),crypto=require('crypto');
const hash=s=>crypto.createHash('sha256').update(s).digest('hex');
const root='local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static';
const names=fs.readdirSync(root+'/js').filter(n=>/^main\..*\.js$/.test(n));if(names.length!==1)throw Error('main missing/ambiguous');
const path=root+'/js/'+names[0],s=fs.readFileSync(path,'utf8'),receipts=[];
function walk(n,visit){if(!n?.type||visit(n)===false)return;for(const v of Object.values(n))for(const c of Array.isArray(v)?v:v?.type?[v]:[])walk(c,visit);}
function receipt(n,label){const source=s.slice(n.start,n.end),r={label,path,file_sha256:hash(s),utf16_range:[n.start,n.end],utf8_byte_range:[Buffer.byteLength(s.slice(0,n.start)),Buffer.byteLength(s.slice(0,n.end))],slice_sha256:hash(source),source};receipts.push(r);return r;}
function module(id){const hits=[...s.matchAll(new RegExp('(?:^|[,\\{])'+id+':','g'))];if(hits.length!==1)throw Error('module missing/ambiguous '+id);let n=acorn.parseExpressionAt(s,hits[0].index+hits[0][0].length,{ecmaVersion:'latest'});if(n.type==='SequenceExpression')n=n.expressions[0];return n;}
const tree=acorn.parse(s,{ecmaVersion:'latest'}),classes=[];walk(tree,n=>{if(n.type==='ClassDeclaration')classes.push(n);});
for(const symbol of ['SM','RM','DM','Sh','QM']){const rows=classes.filter(n=>n.id.name===symbol&&n.start>7000000);if(rows.length!==1)throw Error('class missing/ambiguous '+symbol);receipt(rows[0],symbol);}
const producers=classes.filter(n=>s.slice(n.start,n.end).includes('this.updateMappingsToButtons='));
for(const owner of producers){const text=s.slice(owner.start,owner.end),label='producer '+owner.id.name;receipt(owner,label);walk(owner,n=>{if(n.type==='AssignmentExpression'&&n.left.object?.type==='ThisExpression'&&/^(?:assign|updateMappings|updateButton|clearButton|loadFiltered|loadButton|setActive|updateActive|getKeyValue)/.test(n.left.property.name))receipt(n,owner.id.name+'.'+n.left.property.name);});}
for(const id of [69937,54693,78193,60481,61350,13254,99095,21368,29267])receipt(module(id),'module '+id);
const pos=s.indexOf('class Sh extends');let entry;walk(tree,n=>{if(n.type==='ArrowFunctionExpression'&&n.start<pos&&n.end>pos&&n.end-n.start>500000&&s.slice(n.start,n.start+45).includes('var e={};a.r(e)')){if(entry)throw Error('entry ambiguous');entry=n;}});if(!entry)throw Error('entry missing');
const vars=new Map();walk(entry.body,n=>{if(n.type==='VariableDeclarator'&&n.id.type==='Identifier')vars.set(n.id.name,n.init);if(n!==entry.body&&/Function|Class/.test(n.type))return false;});
for(const [symbol,node]of vars){if(!node)continue;const text=s.slice(node.start,node.end);if(['uM','NM','CM','dh','dM','vM','GM','yM','gM','HM','fM','mM','PM','MM','Sh','JM','Mr','He','lm'].includes(symbol)||text.includes('specialTipShowed')||text.includes('systemKeyboardLayout')||text.includes('buttonList:e.customizeReducer'))receipt(node,'inline '+symbol);}
const css=[];for(const name of fs.readdirSync(root+'/css').filter(n=>n.endsWith('.css'))){const path=root+'/css/'+name,c=fs.readFileSync(path,'utf8');for(const m of c.matchAll(/[^{}]*(?:tip-special|config-btn|active-color|disable-color|key-tip)[^{}]*\{[^}]*\}/g))css.push({path,file_sha256:hash(c),utf16_range:[m.index,m.index+m[0].length],utf8_byte_range:[Buffer.byteLength(c.slice(0,m.index)),Buffer.byteLength(c.slice(0,m.index+m[0].length))],slice_sha256:hash(m[0]),source:m[0]});}
fs.writeFileSync('docs/re/keyboard-679-special-media-current-source.json',JSON.stringify({method:'Current main/CSS static Acorn AST only; complete per-owner producers and popovers. No vendor execution.',product_id:679,main:{path,sha256:hash(s)},receipts,css},null,2)+'\n');
console.log(JSON.stringify({owners:producers.map(n=>n.id.name),receipts:receipts.length,css:css.length}));
