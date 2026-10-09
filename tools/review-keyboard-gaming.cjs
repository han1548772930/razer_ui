// Independently check each current keyboard Gaming Mode mount and read-only rows.
const fs=require('fs'),path=require('path');
const {subtree,componentAst,walk,hash}=require('./current-page-subtree.cjs');
const root=path.resolve(__dirname,'..'),check=process.argv.includes('--check');
const read=file=>fs.readFileSync(path.join(root,file),'utf8');
const pages=JSON.parse(read('docs/re/keyboard-product-pages.json'));
const specs=JSON.parse(read('crates/razer-pages/src/features/keyboard_products_data.json'));
const rows=[],controls=[];
for(const spec of specs.filter(s=>s.controls.gaming_mode)){
 const page=pages.products.find(p=>p.product_id===spec.product_id)?.pages.find(p=>p.key==='TAB_CUSTOMIZE');
 const matches=page?.components.filter(c=>c.source.includes('id:"winKey"')&&c.source.includes('checkAltTab'))??[];
 const row={product_id:spec.product_id,page_key:'TAB_CUSTOMIZE',subtree:'Gaming Mode',page_status:'partial; this review does not cover the rest of Customize'};
 if(matches.length!==1){row.review='pending-source';rows.push(row);continue;}
 const target=matches[0],evidence=subtree(page,target);
 if(!evidence){row.review='pending-mount';rows.push(row);continue;}
 Object.assign(row,evidence);
 const ast=componentAst(target),render=ast.body?.body?.find(n=>n.type==='MethodDefinition'&&n.key.name==='render')?.value;
 if(!render)throw Error('Unexpected current gaming render '+spec.product_id);
 const origins=new Map(),widgets=new Map();
 walk(render.body,n=>{
  if(n.type==='VariableDeclarator'){
   if(n.id.type==='Identifier'&&n.init?.type==='MemberExpression')origins.set(n.id.name,n.init.property.name);
   if(n.id.type==='ObjectPattern')for(const p of n.id.properties){if(p.value?.name)origins.set(p.value.name,p.key.name);}
  }
  if(n.type==='CallExpression'&&n.callee.type==='SequenceExpression'&&['jsx','jsxs'].includes(n.callee.expressions.at(-1)?.property?.name)){
   const props=n.arguments[1]?.properties??[],id=props.find(p=>p.key?.name==='id')?.value?.value;
   if(['winKey','menuKey','copilotKey'].includes(id))widgets.set(id,Object.fromEntries(props.filter(p=>p.type==='Property').map(p=>[p.key.name,p.value])));
  }
 });
 const propOf=node=>node?.type==='Identifier'?origins.get(node.name):node?.type==='MemberExpression'?node.property.name:null;
 const windows=widgets.get('winKey'),menu=widgets.get('menuKey'),copilot=widgets.get('copilotKey');
 const readOnly=widget=>widget?.disabled?.type==='UnaryExpression'&&widget.disabled.operator==='!'&&widget.disabled.argument.value===0;
 const upstreamSystem=evidence.receipts.slice(0,-1).filter(r=>r.source.includes('isSystem'));
 const verified={product_id:spec.product_id,
  windows_prop:propOf(windows?.active)==='isWindowsKeyDisabled'&&readOnly(windows),
  menu:!!menu&&propOf(menu.active)==='isWindowsKeyDisabled'&&readOnly(menu)&&target.source.includes('"KEY_APPLICATION"')&&upstreamSystem.length===0,
  copilot:!!copilot&&propOf(copilot.active)==='isWindowsKeyDisabled'&&readOnly(copilot)&&target.source.includes('"DKM_D2"')&&target.source.includes('"DKM_F6"')};
 row.source_rows=verified;
 row.current_keys={menu:spec.keys.some(k=>k.inputID==='KEY_APPLICATION'),copilot:spec.keys.some(k=>['DKM_D2','DKM_F6'].includes(k.inputID))};
 row.upstream_isSystem_mentions=upstreamSystem.map(r=>r.symbol);
 row.review='source-subtree-reviewed';
 row.remaining=['Full Gaming Mode switch/help/shortcut and all product-specific extra controls remain partial.',
  'Runtime observations and full Customize/input/visual acceptance are not established by these receipts.'];
 controls.push(verified);rows.push(row);
}
const summary={products:rows.length,subtrees:rows.filter(r=>r.review==='source-subtree-reviewed').length,
 windows:controls.filter(c=>c.windows_prop).length,menu_eligible:rows.filter(r=>r.source_rows?.menu&&r.current_keys.menu).length,
 copilot_eligible:rows.filter(r=>r.source_rows?.copilot&&r.current_keys.copilot).length,completed_pages:0,completed_products:0};
function output(file,value){const text=JSON.stringify(value,null,2)+'\n';if(check){if(read(file)!==text)throw Error('Stale '+file);}else fs.writeFileSync(path.join(root,file),text);}
output('crates/razer-pages/src/features/keyboard_gaming_review_data.json',controls);
output('docs/re/keyboard-gaming-review-2026-10-07.json',{date:'2026-10-07',scope:'Read-only Windows/Menu/Copilot rows per mounted current Gaming Mode; not full-page completion',
 inputs:[{path:'docs/re/keyboard-product-pages.json',sha256:hash(read('docs/re/keyboard-product-pages.json'))}],summary,rows});
console.log(JSON.stringify(summary));
console.log('upstream isSystem: '+rows.filter(r=>r.upstream_isSystem_mentions?.length).map(r=>r.product_id+':'+r.upstream_isSystem_mentions.join(',')).join(';'));
