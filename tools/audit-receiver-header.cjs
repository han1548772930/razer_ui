// Audit the live receiver root and its header consumer without evaluating JS.
const fs=require('fs'),path=require('path'),crypto=require('crypto'),acorn=require('acorn');
const root=path.resolve(__dirname,'..'),file='.ref/devices/179/static/js/main.4849f7ca.js';
const bytes=fs.readFileSync(path.join(root,file)),source=bytes.toString('utf8');
const hash=b=>crypto.createHash('sha256').update(b).digest('hex');
const ast=acorn.parse(source,{ecmaVersion:'latest'}),matches=[],consumers=[];
function walk(n){
 if(!n?.type)return;
 if(n.type==='Property'&&n.key.name==='renderProfileBar'&&n.value.type==='UnaryExpression'&&n.value.operator==='!'&&n.value.argument.value===1)matches.push(n);
 if(n.type==='LogicalExpression'&&n.operator==='&&'&&source.slice(n.start,n.end)==='this.props.renderProfileBar&&this.props.renderProfileBar()')consumers.push(n);
 for(const v of Object.values(n)){if(Array.isArray(v))v.forEach(walk);else if(v?.type)walk(v);}
}
walk(ast);
if(matches.length!==1||consumers.length!==1)throw Error('Changed receiver header contract; re-audit');
const receipt=n=>({path:file,sha256:hash(bytes),offset:n.start,end:n.end,source:source.slice(n.start,n.end)});
const result={product_id:179,name:'HyperPolling Wireless Dongle',show_profile_bar:false,method:'Acorn root prop and actual conditional consumer; no vendor execution',generator_sha256:hash(fs.readFileSync(__filename)),root_prop:receipt(matches[0]),header_consumer:receipt(consumers[0])};
const target=path.join(root,'docs/re/receiver-profile-header-current-evidence.json'),output=JSON.stringify(result,null,2)+'\n';
if(process.argv.includes('--check')){if(fs.readFileSync(target,'utf8')!==output)throw Error('Stale receiver header evidence');}else fs.writeFileSync(target,output);
console.log('Receiver root disables its profile bar; verified the current header consumer.');
