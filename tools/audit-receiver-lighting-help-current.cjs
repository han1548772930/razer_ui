// Product 241 Lighting/Help source receipts. Vendor code is never executed.
const fs=require('fs'),path=require('path');
const {Source,walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),read=p=>fs.readFileSync(path.join(root,p),'utf8');
const directory='.ref/devices/241',manifestPath=directory+'/asset-manifest.json',manifest=JSON.parse(read(manifestPath));
const s=Object.create(Source.prototype);
s.directory=directory;s.files=[...new Set(Object.values(manifest.files))].filter(p=>/^\.\/static\/js\/[^/]+\.js$/.test(p)).map(p=>directory+'/'+p.slice(2));
s.modules=new Map();s.texts=new Map();s.parsed=new Set();
const receipts=[];
function record(id,name,node,purpose) {
  const r={id:`${id}:${name}`,module:id,name,purpose,...s.receipt(id,node)};
  if(receipts.some(x=>x.id===r.id))throw Error('Duplicate '+r.id);
  receipts.push(r);return r;
}
function bind(id,name,purpose) {return record(id,name,s.binding(id,name),purpose);}
record(8193,'factory',s.module(8193).fn,'Actual product descriptor, eight declared effects and default profile values');
for(const [id,exportName] of [[3254,'AoV'],[3254,'U2A'],[9228,'hz']]) {
  const exported=s.exported(id,exportName);
  if(exported.type!=='Identifier')throw Error('Changed literal export '+exportName);
  record(id,exportName,s.binding(id,exported.name),'Actual effect IDs/default settings and wave direction enumeration');
}
for(const name of ['Mn','Ln','Rn','M','L','f','g','v','N','fn','Cn','dn','pn','En','on','an','rn','ln','In','gn',
  'vt','mt','ee','te','Ne','me','Ae','ue','Ce','De','ct','rt','It','Tt','St','Dt','je','He','Ye','Ke','et','gt','x','J','Z','X','Q']) {
  bind(9259,name,'Lighting actual mounted subtree or directly referenced control/action; dynamic branch reachability is conditional');
}
record(5107,'factory',s.module(5107).fn,'Lighting Redux action creators and effect cache/profile identity; no hardware success acknowledgment');
for(const [id,name] of [[1422,'s'],[3765,'s'],[6299,'p']])bind(id,name,'Actual page/column/widget wrapper and tooltip ownership');
for(const statement of s.module(9259).fn.body.body) {
  if(statement.type==='VariableDeclaration')for(const d of statement.declarations)
    if(d.id.type==='ObjectPattern'&&d.id.properties.some(p=>['ft','Dn','Sn','hn'].includes(p.value?.name)))
      record(9259,'config-'+d.start,d,'Actual destructured product configuration and defaults; not inferred from a neighboring product');
  if(statement.type==='ExpressionStatement'&&statement.expression.type==='AssignmentExpression') {
    const n=statement.expression;
    if(n.left.type==='MemberExpression'&&key(n.left.property)==='defaultProps'&&['vt','h','J','X'].includes(n.left.object.name))
      record(9259,'defaults-'+n.left.object.name,n,'Actual shared default props');
  }
}
for(const name of ['Ia','Oa','tt','et','wa','nn','na','Ui','Li','ki','Ki','Hi','Zi','Gi','qi'])bind(9163,name,'Help content, prop gates, links, confirmations and normal-root reset caller');
for(const statement of s.module(9163).fn.body.body)if(statement.type==='ExpressionStatement'&&statement.expression.type==='AssignmentExpression') {
  const n=statement.expression;
  if(n.left.type==='MemberExpression'&&key(n.left.property)==='defaultProps'&&n.left.object.name==='Ia')
    record(9163,'defaultProps',n,'Help resetTitle is the only default prop at this assignment');
}
const effect= s.binding(9259,'vt'),cases=[];
walk(effect,n=>{if(n.type==='SwitchCase')cases.push({test:n.test?s.snippet(9259,n.test):'default',...s.receipt(9259,n)});});
const help=s.binding(9163,'Ia'),methods=[];
walk(help,n=>{
  if(n.type==='MethodDefinition'||n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&n.left.object.type==='ThisExpression'&&/FunctionExpression$/.test(n.right.type))
    methods.push({name:n.type==='MethodDefinition'?key(n.key):key(n.left.property),...s.receipt(9163,n)});
});
const css=[...new Set(Object.values(manifest.files))].filter(p=>/\.css$/.test(p)).map(p=>{
  const file=directory+'/'+p.replace(/^\.\//,''),t=read(file);
  const rules=parseCSS(t).filter(r=>/(?:body-widgets|widget-container|widget-col|slider-container|slider-tip|slider\b|chroma|lighting|modes-|effects-area|color-|random-color|cp-chunk|direction|battery-percent|battery-level|battery-header|help-component|support-link|show-all-button|reset-btn|fix-btn|uppercase-title|z-index11|s3-dropdown|s3-options|adveffect|\.mt(?:5|10|20)\b|\.flex\b|\.disabled\b)/.test(r.selector));
  return {path:file,sha256:hash(fs.readFileSync(path.join(root,file))),rules};
}).filter(c=>c.rules.length);
const result={schema_version:1,product_id:241,source_date:'2026-10-02',generator_sha256:hash(fs.readFileSync(__filename)),
  dependencies:['tools/webpack-source.cjs','tools/css-source.cjs'].map(p=>({path:p,sha256:hash(fs.readFileSync(path.join(root,p)))})),
  manifest:{path:manifestPath,sha256:hash(fs.readFileSync(path.join(root,manifestPath)))},
  offset_unit:'UTF-16 code units; end exclusive; source file hashes use actual UTF-8 bytes',
  scope:'Product 241 Lighting and Help actual source details, UI state/actions and style contexts; not device mutation integration',
  receipts,effect_render_cases:cases,help_methods:methods,css,
  limitations:['CSS receipts are context candidates, not computed style or visual/DPI validation.',
    'Redux/local-state changes and source broadcasts do not establish hardware write success.',
    'Generic props are recorded separately from the actual 241 call sites; feature presence in shared code is not enabled capability.',
    'No application, build, test, vendor JavaScript or DLL executed.']};
// Maintain narrow semantic invariants observed in the original source.
const source=r=>receipts.find(x=>x.id===r)?.source??'';
if(!source('8193:factory').includes('Fire_Effect')||cases.some(c=>c.test.endsWith('.Fire_Effect')))throw Error('Fire declaration/render boundary changed');
if(!source('9259:Mn').includes('direction:"left"')||!source('9259:Mn').includes('direction:"right"'))throw Error('Lighting columns changed');
if(!source('9163:nn').includes('(0,C.jsx)(Oa,{resetObm:this.resetDevice})'))throw Error('Help actual caller props changed');
if(!source('9163:defaultProps').includes('resetTitle'))throw Error('Help defaults changed');
const target='docs/re/receiver-lighting-help-current-evidence.json',text=JSON.stringify(result,null,2)+'\n';
if(process.argv.includes('--check')){if(read(target)!==text)throw Error('Stale '+target);}else fs.writeFileSync(path.join(root,target),text);
console.log(JSON.stringify({receipts:receipts.length,effect_cases:cases.length,help_methods:methods.length,css_context_rules:css.reduce((n,c)=>n+c.rules.length,0)}));
