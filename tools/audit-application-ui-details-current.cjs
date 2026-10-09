#!/usr/bin/env node
'use strict';
// Static source receipts only. No reference file is imported or evaluated.
const fs=require('fs'),path=require('path'),zlib=require('zlib'),crypto=require('crypto'),acorn=require('acorn');
const ROOT=path.resolve(__dirname,'..'),OUT='docs/re/application-ui-details-current.json';
const hash=x=>crypto.createHash('sha256').update(x).digest('hex');
const read=f=>fs.readFileSync(path.join(ROOT,f));
const catalogFile='docs/re/all-application-chains-current.json',catalogRaw=read(catalogFile),catalog=JSON.parse(catalogRaw);
const graphRaw=read('docs/re/'+catalog.data_file);
if(hash(graphRaw)!==catalog.data_sha256)throw Error('application source graph drift');
const graph=JSON.parse(zlib.gunzipSync(graphRaw));
const sources=new Map(graph.sources.map(s=>[s.path,s]));
const cache=new Map(),receipts=[],receiptKeys=new Map();
function source(file){if(!file.startsWith('.ref/applications/'))throw Error('noncurrent app source '+file);if(!cache.has(file)){const raw=read(file),text=raw.toString('utf8'),expected=sources.get(file)?.sha256;if(expected&&hash(raw)!==expected)throw Error('current source hash mismatch '+file);cache.set(file,{raw,text,sha256:hash(raw)});}return cache.get(file);}
function receipt(file,start,end,kind){const s=source(file),key=file+':'+start+':'+end;if(receiptKeys.has(key))return receiptKeys.get(key);if(!(start>=0&&end>start&&end<=s.text.length))throw Error('receipt range '+key);const text=s.text.slice(start,end),id='ui-'+receipts.length;receipts.push({id,file,start,end,kind,source_sha256:s.sha256,sha256:hash(text),text});receiptKeys.set(key,id);return id;}
const key=n=>n?.type==='Identifier'?n.name:n?.value;
const member=n=>n?.type==='MemberExpression'?key(n.property):null;
const lastCallee=n=>n?.type==='SequenceExpression'?lastCallee(n.expressions.at(-1)):n?.type==='CallExpression'&&n.callee.type==='Identifier'&&n.callee.name==='Object'&&n.arguments.length===1?lastCallee(n.arguments[0]):n;
const isUI=n=>n?.type==='CallExpression'&&['jsx','jsxs','createElement'].includes(member(lastCallee(n.callee)));
function cssEnd(text,offset){let depth=0,quote=null,comment=false;for(let i=offset;i<text.length;i++){const c=text[i],next=text[i+1];if(comment){if(c==='*'&&next==='/'){comment=false;i++;}continue;}if(quote){if(c==='\\'){i++;continue;}if(c===quote)quote=null;continue;}if(c==='/'&&next==='*'){comment=true;i++;continue;}if(c==='"'||c==="'"){quote=c;continue;}if(c==='{')depth++;else if(c==='}'&&--depth===0)return i+1;}throw Error('unterminated CSS rule');}
const eventPattern=/^(on[A-Z]|toggle|onSave|onCancel|onApply|selectOption)/;
const propKeys=new Set(['id','className','style','type','title','text','name','value','checked','disabled','readOnly','min','max','step','maxLength','placeholder','active','isMounted','show','hasSwitch','withHeader','withFooter','href','src','accept','multiple','selected','loading']);
const applications=[],fileDetails=[];
for(const app of graph.applications){
 const html=app.html?.path?source(app.html.path):null;
 if(html&&html.sha256!==app.html.sha256)throw Error('HTML drift '+app.route);
 const classes=new Set(),files=[],rootReceipts=[];
 let uiCount=0,eventCount=0,guardCount=0;
 for(const file of app.js_files){
  // Externals/locales have no app render props. Keep their current graph hashes as provenance.
  const g=sources.get(file);
  if(!g)throw Error('source missing from current graph '+file);
  if(!file.startsWith('.ref/applications/'+app.route.slice(1)))continue;
  const hasUI=g.modules.some(m=>m.jsx_references.length)||g.root_mount_candidates.length||/main\.|App\./.test(path.basename(file));
  if(!hasUI)continue;
  const s=source(file),ast=acorn.parse(s.text,{ecmaVersion:'latest',sourceType:'module'}),facts=[],mounts=[];
  function walk(n,anc){
   if(!n||typeof n.type!=='string')return;
   if(isUI(n)){
    const props=n.arguments[1],properties=props?.type==='ObjectExpression'?props.properties:[];
    const selected=[];
    for(const p of properties){if(p.type!=='Property')continue;const name=key(p.key);if(!propKeys.has(name)&&!eventPattern.test(name||''))continue;selected.push({name,receipt:receipt(file,p.start,p.end,'ui_prop'),expression:s.text.slice(p.value.start,p.value.end)});if(eventPattern.test(name))eventCount++;
     if(name==='className'){
      const raw=s.text.slice(p.value.start,p.value.end);
      for(const m of raw.matchAll(/["'`]([^"'`]+)["'`]/g))for(const token of m[1].split(/\s+/))if(/^[a-zA-Z_][\w-]*$/.test(token))classes.add(token);
     }
    }
    const guards=anc.filter(a=>['ConditionalExpression','LogicalExpression','IfStatement','SwitchCase'].includes(a.type)).slice(-4).map(a=>({type:a.type,start:a.start,end:a.end,test:a.test?s.text.slice(a.test.start,a.test.end):a.type==='LogicalExpression'?s.text.slice(a.left.start,a.left.end):null,operator:a.operator||null,branch:a.type==='ConditionalExpression'?(n.start>=a.consequent.start&&n.end<=a.consequent.end?'consequent':'alternate'):null}));
    const owner=[...anc].reverse().find(a=>['ClassDeclaration','FunctionDeclaration','VariableDeclarator'].includes(a.type));
    const component=n.arguments[0]?s.text.slice(n.arguments[0].start,n.arguments[0].end):null;
    const children=member(lastCallee(n.callee))==='createElement'?n.arguments.slice(2).map(c=>({start:c.start,end:c.end,expression:s.text.slice(c.start,Math.min(c.end,c.start+160)),truncated:c.end-c.start>160})):properties.filter(p=>p.type==='Property'&&key(p.key)==='children').flatMap(p=>p.value.type==='ArrayExpression'?p.value.elements.filter(Boolean):[p.value]).map(c=>({start:c.start,end:c.end,expression:s.text.slice(c.start,Math.min(c.end,c.start+160)),truncated:c.end-c.start>160}));
    facts.push({start:n.start,end:n.end,component,owner:owner?{name:owner.id?.name||null,type:owner.type,start:owner.start,end:owner.end}:null,props:selected,guards,ordered_children:children});uiCount++;guardCount+=guards.length;
   }
   for(const v of Object.values(n)){if(Array.isArray(v))for(const q of v)walk(q,[...anc,n]);else if(v&&typeof v.type==='string')walk(v,[...anc,n]);}
  }
  walk(ast,[]);
  for(const r of g.root_mount_candidates){if(s.text.slice(r.offset,r.end)!==r.expression)throw Error('root source mismatch');mounts.push(receipt(file,r.offset,r.end,'root_mount_candidate'));}
  if(facts.length||mounts.length){fileDetails.push({file,sha256:s.sha256,ui_calls:facts,root_mount_receipts:mounts});files.push(file);rootReceipts.push(...mounts);}
 }
 const css=[];
 for(const c of app.css){const s=source(c.path);if(s.sha256!==c.sha256)throw Error('CSS drift '+c.path);const rules=[];
  for(const r of c.rule_receipts){
   const classNames=[...r.selector.matchAll(/\.([A-Za-z_][\w-]*)/g)].map(m=>m[1]);
   const relevant=classNames.some(n=>classes.has(n))||/(^|[,\s])(body|html|button|input|textarea|\*)(?=$|[,\s:.#\[])/.test(r.selector);
   if(!relevant)continue;
   const end=cssEnd(s.text,r.offset);const raw=s.text.slice(r.offset,end);
   if(!raw.includes(r.declarations))throw Error('CSS body mismatch '+c.path+':'+r.offset);
   rules.push({...r,source_receipt:receipt(c.path,r.offset,end,'matched_css_rule'),matched_literal_classes:classNames.filter(n=>classes.has(n))});
  }
  // Font faces are kept exactly as source data; actual font resource consumption remains a separate boundary.
  const fontFaces=[];for(const r of c.font_rules){const end=cssEnd(s.text,r.offset);fontFaces.push({source_receipt:receipt(c.path,r.offset,end,'font_face')});}
  css.push({file:c.path,sha256:s.sha256,matched_rule_count:rules.length,rules,font_faces:fontFaces});
 }
 applications.push({route:app.route,html:app.html,manifest:app.manifest,application_local_ui_files:files,root_mount_candidates:rootReceipts,ui_call_count:uiCount,event_prop_count:eventCount,enclosing_guard_count:guardCount,literal_class_tokens:[...classes].sort(),css,scope_status:html?'static_render_controls_css_indexed_semantic_review_partial':'no_current_html_source',complete_ui_semantics_claimed:false,local_implementation_audited:false,runtime_validation:false});
}
const supportingFiles=[
 'dashboard-device-current-evidence.json','dashboard-card-state-current-evidence.json','app-introduction-banner-current-evidence.json',
 'settings-window-current-evidence.json','settings-presentation-current-evidence.json','public-ui-review-current-evidence.json',
 'profiles-content-current-evidence.json','profiles-transfer-current-evidence.json','macro-full-page-source.json','macro-command-flow-current-evidence.json',
 'macro-onboarding-current-evidence.json','macro-text-current-evidence.json','macro-launch-current-evidence.json','macro-record-options-current-evidence.json','macro-recording-current-evidence.json',
 'ring-light-current-evidence.json','tray-ui-current-evidence.json','chroma-settings-current-evidence.json',
 'chroma-app-current-evidence.json','feedback-app-current-evidence.json','armory-app-current-audit.json','profile-migration-current-source.json','studio-properties-current-evidence.json',
];
const supporting=[];
for(const rel of supportingFiles){const f='docs/re/'+rel;if(!fs.existsSync(path.join(ROOT,f)))continue;const raw=read(f),j=JSON.parse(raw),refs=[],unmatched=[];
 function scan(n,trail){if(!n||typeof n!=='object')return;
  const file=n.path||n.file,start=n.offset??n.start,end=n.end,text=n.source??n.text;
  if(typeof file==='string'&&file.startsWith('.ref/applications/')&&typeof start==='number'&&typeof end==='number'&&typeof text==='string'){
   const s=source(file);if(s.text.slice(start,end)===text)refs.push({json_path:trail,source_receipt:receipt(file,start,end,'revalidated_existing_detail')});else unmatched.push({json_path:trail,file,start,end,reason:'original receipt is not an exact current slice; not promoted'});
  }
  for(const [k,v]of Object.entries(n))if(v&&typeof v==='object')scan(v,trail+'/'+k);
 }
 scan(j,'');supporting.push({file:f,sha256:hash(raw),exact_revalidated_receipts:refs,not_promoted:unmatched});
}
// Manual semantic anchors are added below after reading their complete bodies.
const anchor=(id,route,bundle,name,start,type='VariableDeclarator')=>({id,route:'/'+route+'/',file:'.ref/applications/'+route+'/static/js/'+bundle,name,start,type});
const semanticSelectors=[
 anchor('dashboard-main-sections','synapse/dashboard','7861.1b0e99a4.chunk.js','xi',117297,'ClassDeclaration'),
 anchor('dashboard-device-card-state-tree','synapse/dashboard','7861.1b0e99a4.chunk.js','z',19894,'ClassDeclaration'),
 anchor('dashboard-introduction-banner','synapse/dashboard','7861.1b0e99a4.chunk.js','ji',133175),
 anchor('synapse-settings-startup','synapse/settings','720.1e5d1c8f.chunk.js','Ks',80201),
 anchor('synapse-settings-notifications','synapse/settings','720.1e5d1c8f.chunk.js','bn',48463),
 anchor('synapse-settings-wdl','synapse/settings','720.1e5d1c8f.chunk.js','ia',84051),
 anchor('synapse-settings-about','synapse/settings','720.1e5d1c8f.chunk.js','Fs',77038),
 anchor('standalone-settings-nav-root','settings','97.d87e27ff.chunk.js','ss',190482,'ClassDeclaration'),
 anchor('profiles-games-devices-root','synapse/profiles','9449.8f17b519.chunk.js','Ba',197107,'ClassDeclaration'),
 anchor('macro-command-root','synapse/macro','main.3f4b9604.js','Mn',632159),
 anchor('macro-unsaved-dialog','synapse/macro','main.3f4b9604.js','Gr',688165),
 anchor('lite-render-views','sophie-lite','main.bb22144c.chunk.js','Vn',494058),
 anchor('lite-audio-main-control','sophie-lite','main.bb22144c.chunk.js','kt',464605),
 anchor('lite-device-selector','sophie-lite','main.bb22144c.chunk.js','zt',461226,'ClassDeclaration'),
 anchor('lite-device-selection-native-first','sophie-lite','main.bb22144c.chunk.js','bt',460573),
 anchor('lite-surround-toggle-native-first','sophie-lite','main.bb22144c.chunk.js','vt',460797),
 anchor('lite-code-input','sophie-lite','main.bb22144c.chunk.js','Mt',465389,'ClassDeclaration'),
 anchor('lite-activation-form','sophie-lite','main.bb22144c.chunk.js','tn',471640,'ClassDeclaration'),
 anchor('lite-settings-general-about','sophie-lite','main.bb22144c.chunk.js','_n',475003,'ClassDeclaration'),
 anchor('lite-promo-carousel','sophie-lite','main.bb22144c.chunk.js','Zn',491013),
 anchor('lite-two-step-setup','sophie-lite','main.bb22144c.chunk.js','Ta',507312,'ClassDeclaration'),
 anchor('lite-host-user-session','sophie-lite','main.bb22144c.chunk.js','oa',496435,'ClassDeclaration'),
 anchor('sophie-render-license-gates','sophie','main.0dad7d2f.chunk.js','xl',925149),
 anchor('sophie-nav-descriptors','sophie','main.0dad7d2f.chunk.js','Nu',847799),
 anchor('sophie-nav-history','sophie','main.0dad7d2f.chunk.js','Yu',862450),
 anchor('sophie-audio-output-setup','sophie','main.0dad7d2f.chunk.js','Jc',834393),
 anchor('sophie-equalizer-editor','sophie','main.0dad7d2f.chunk.js','$i',759653),
 anchor('sophie-calibration-editor','sophie','main.0dad7d2f.chunk.js','pc',812523),
 anchor('sophie-redemption-input','sophie','main.0dad7d2f.chunk.js','$u',866404),
 anchor('sophie-onboarding-pricing-trial','sophie','main.0dad7d2f.chunk.js','cl',874018),
 anchor('sophie-settings-general','sophie','main.0dad7d2f.chunk.js','ml',900345),
 anchor('sophie-settings-about','sophie','main.0dad7d2f.chunk.js','vl',902191),
 anchor('sophie-host-user-session','sophie','main.0dad7d2f.chunk.js','Fl',930135),
 anchor('alisha-four-tab-gates-and-settings','alisha','main.83ea24ca.js','st',1026838),
 anchor('alisha-authenticated-lifecycle','alisha','main.83ea24ca.js','Nt',1032718,'ClassDeclaration'),
 anchor('ring-light-render-hash-views','natalie','main.35e04e8c.chunk.js','nc',475258),
 anchor('ring-light-pod-controller','natalie','main.35e04e8c.chunk.js','Li',390529),
 anchor('ring-light-settings-page','natalie','main.35e04e8c.chunk.js','go',447203),
 anchor('feedback-form-and-results','feedback','496.003ef6c6.chunk.js','ps',101277,'ClassDeclaration'),
 anchor('account-popup-user-guest-actions','rz-user-profile-menu','Root.012dd389.chunk.js','N',2609,'ClassDeclaration'),
 anchor('tour-previous-next-skip','synapse/introduction-tour','main.bf769e69.js','Dn',167376),
 anchor('tour-synapse-steps','synapse/introduction-tour','main.bf769e69.js','xn',168314),
 anchor('tour-chroma-steps','synapse/introduction-tour','main.bf769e69.js','jn',169461),
 anchor('tour-path-root','synapse/introduction-tour','main.bf769e69.js','Fn',170656),
 anchor('alexa-login-activation-views','synapse/alexa','main.05f102d2.js','Bp',765057),
 anchor('alexa-device-panel','synapse/alexa','main.05f102d2.js','Yp',771455),
 anchor('alexa-settings','synapse/alexa','main.05f102d2.js','yE',782969),
 anchor('alexa-install-root','synapse/alexa','main.05f102d2.js','qE',803956),
 anchor('firmware-result-actions','synapse/update-fw','main.69cc5fbd.js','Wo',1907857),
 anchor('firmware-blocking-warning-buttons','synapse/update-fw','main.69cc5fbd.js','Zo',1909981),
 anchor('firmware-status-render-dispatch','synapse/update-fw','main.69cc5fbd.js','ri',1912154),
];
const semanticAnchors=[];
for(const sel of semanticSelectors){const s=source(sel.file),ast=acorn.parse(s.text,{ecmaVersion:'latest',sourceType:'module'});let found=null;function walk(n){if(!n||!n.type)return;if(n.start===sel.start&&n.type===sel.type&&(!sel.name||n.id?.name===sel.name)){if(found)throw Error('duplicate semantic selector');found=n;}for(const v of Object.values(n)){if(Array.isArray(v))v.forEach(walk);else if(v&&v.type)walk(v);}}walk(ast);if(!found)throw Error('missing semantic anchor '+sel.id);semanticAnchors.push({...sel,receipt:receipt(sel.file,found.start,found.end,'manual_semantic_anchor')});}
const output={schema_version:1,source_freeze_date:'2026-10-02',audit_date:'2026-10-09',generator_sha256:hash(fs.readFileSync(__filename)),parser:{name:'acorn',version:acorn.version,module:path.relative(ROOT,require.resolve('acorn')).replaceAll('\\','/'),sha256:hash(fs.readFileSync(require.resolve('acorn')))},css_parser_boundary:'No CSS helper imported: existing source graph rule descriptions are hash checked and exact balanced source slices revalidated, including nested rules.',method:'Current source graph hash verified; application-local AST render props (embedded dependency candidates retained), ancestor guards and ordered immediate children; exact CSS/font-face slices; existing detail receipts revalidated against current source; selected manual semantic anchors. No application/vendor JS/DLL execution.',offset_unit:'UTF-16 JavaScript code units',catalog_sha256:hash(catalogRaw),graph_sha256:hash(graphRaw),summary:{endpoints:applications.length,html_endpoints:applications.filter(a=>a.html?.path).length,ui_source_files:fileDetails.length,ui_calls:fileDetails.reduce((n,f)=>n+f.ui_calls.length,0),event_props:applications.reduce((n,a)=>n+a.event_prop_count,0),source_receipts:receipts.length,matched_css_rules:applications.reduce((n,a)=>n+a.css.reduce((m,c)=>m+c.matched_rule_count,0),0),supporting_evidence_files:supporting.length,supporting_exact_receipts:supporting.reduce((n,s)=>n+s.exact_revalidated_receipts.length,0),supporting_not_promoted:supporting.reduce((n,s)=>n+s.not_promoted.length,0),manual_semantic_anchors:semanticAnchors.length,fully_completed_ui_semantics:0},applications,file_details:fileDetails,supporting_evidence:supporting,semantic_anchors:semanticAnchors,receipts,limitations:['Named JSX/createElement calls and handler names are static candidates until root/caller/conditions are followed.','Application-local bundle paths do not establish first-party ownership; embedded dependencies remain in structural candidate counts.','Ordered children are source order, not proof of rendered visual order under CSS or portals.','Literal class matches miss dynamically generated classes; matched CSS is not computed cascade, font loading, geometry, focus or hit testing.','Revalidated existing source receipts do not certify the local Rust implementation or all prior prose claims.','Unknown device/service observations must not be fabricated; original write operations are documented but project DLL write-back remains deferred.','No endpoint is claimed to have complete UI/business/DLL semantics.']};
const serialized=JSON.stringify(output,null,2)+'\n';
if(process.argv.includes('--check')){if(read(OUT).toString('utf8')!==serialized)throw Error('application UI detail evidence drift');console.log(JSON.stringify(output.summary));}
else{fs.writeFileSync(path.join(ROOT,OUT),serialized);console.log(JSON.stringify(output.summary));}
