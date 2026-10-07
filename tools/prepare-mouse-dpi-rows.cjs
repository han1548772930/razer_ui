// Register only independently audited DPI-row implementations. Static parsing only.
const fs = require('fs'), path = require('path');
const acorn = require('acorn');
const {Source, walk, key, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const pages = JSON.parse(read('docs/re/mouse-page-source.json')).products;
const configs = JSON.parse(read('docs/re/mouse-product-source.json')).products;
const capabilities = [], evidence = [];

// Product identifiers belong to audited registration data, never UI dispatch.
for (const registration of [
  {product_id:70, panel:'cI', rows:'EI', row:'XT', inset:'eI', initial:'KE'},
  {product_id:226, panel:'Ms', rows:'ls', row:'es', inset:'rs', initial:'Pe'},
]) {
  const {product_id, panel, rows, row, inset, initial} = registration;
  const directory = `.ref/devices/${product_id}`;
  const manifestPath = directory + '/asset-manifest.json';
  const manifest = JSON.parse(read(manifestPath));
  const source = Object.assign(Object.create(Source.prototype), {directory,
    files:[...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.js')).map(f=>directory+'/'+f.replace(/^\.\//,'')),
    modules:new Map(),texts:new Map(),parsed:new Set()});
  const page = pages.find(p=>p.product_id===product_id).pages.find(p=>p.key==='TAB_PERFORMANCE');
  if (!source.files.includes(page.path) || hash(read(page.path))!==page.sha256) throw Error('Stale mounted DPI source '+product_id);
  source.parse(page.path);
  let owner = [...source.modules.values()].find(m=>[panel,rows,row].every(name=>m.definitions.get(name)?.type==='ClassDeclaration'));
  // Some current products inline the entry scope outside the webpack module
  // table. Parse that lexical scope; it is still data, never imported code.
  if(!owner)walk(acorn.parse(read(page.path),{ecmaVersion:'latest'}),node=>{
    if(!/Function/.test(node.type)||node.body?.type!=='BlockStatement'
      ||!node.body.body.some(statement=>statement.type==='ClassDeclaration'&&statement.id.name===panel))return;
    owner={id:'dpi-row-entry',file:page.path,fn:node,definitions:new Map(),exports:new Map()};
    for(const statement of node.body.body){
      if(statement.type==='VariableDeclaration')for(const declaration of statement.declarations){
        if(declaration.id.type==='Identifier')owner.definitions.set(declaration.id.name,declaration.init);
      }
      else if(['ClassDeclaration','FunctionDeclaration'].includes(statement.type))owner.definitions.set(statement.id.name,statement);
    }
    source.modules.set(owner.id,owner);
    return false;
  });
  if (!owner) throw Error('Missing mounted row owner '+product_id);
  const receipts = [];
  for (const symbol of [panel, rows, row]) {
    const receipt = source.receipt(owner.id, source.binding(owner.id,symbol));
    const mounted = page.components.find(c=>c.symbol===symbol);
    if (!mounted || receipt.source!==mounted.source) throw Error('Mounted row receipt changed '+symbol);
    receipts.push({symbol,...receipt});
  }

  const config = configs.find(p=>p.product_id===product_id).config;
  if (hash(read(config.path))!==config.sha256) throw Error('Stale current CONFIG '+product_id);
  const info = source.literal(config.module_id,source.exported(config.module_id,'DeviceInfo'));
  let profile = source.exported(config.module_id,'DEFAULTPROFILE');
  if (profile.type==='Identifier') profile = source.binding(config.module_id,profile.name);
  const profileStageNode = profile.properties.find(p=>['DPIStages','dpiStages'].includes(key(p.key)));
  if (!profileStageNode) throw Error('Missing source profile stage schema');
  const profileStages = source.literal(config.module_id,profileStageNode.value);
  receipts.push({symbol:'DeviceInfo',...source.receipt(config.module_id,source.exported(config.module_id,'DeviceInfo'))});
  receipts.push({symbol:'profile-stage-schema',...source.receipt(config.module_id,profileStageNode)});
  const upper = key(profileStageNode.key)==='DPIStages';
  const stageValues = upper ? profileStages.Stages.DPIStage : profileStages.stages;
  const schema = upper ? {
    stages_path:'/DPIStages/Stages/DPIStage',active_path:'/DPIStages/Active',enabled_path:'/DPIStages/State',
    x_key:'X',y_key:'Y',independent_key:'Independent',visible_key:'Active',
  } : {
    stages_path:'/dpiStages/stages',active_path:'/dpiStages/active',enabled_path:'/dpiStages/enable',
    x_key:'x',y_key:'y',independent_key:'independent',visible_key:'visible',
  };
  if (!stageValues.length || stageValues.some(stage=>![schema.x_key,schema.y_key,schema.independent_key,schema.visible_key].every(k=>Object.hasOwn(stage,k))))
    throw Error('Inconsistent profile row schema '+product_id);

  const insetNode = source.binding(owner.id,inset);
  if (!source.snippet(owner.id,insetNode).includes('.isSensitivitySliderWithGrid')) throw Error('Changed inset condition');
  receipts.push({symbol:'row-inset-condition',...source.receipt(owner.id,insetNode)});
  let insetValue, dragField, descriptionKey, minimumVisible;
  walk(source.binding(owner.id,rows),node=>{
    if (node.type==='Property' && key(node.key)==='marginLeft') insetValue=source.literal(owner.id,node.value);
    if (node.type==='AssignmentExpression' && node.left.property?.name==='onDragStart') {
      walk(node.right,child=>{
        if (child.type==='BinaryExpression' && child.operator==='===' && child.right.type==='MemberExpression'
          && child.left.type==='UnaryExpression' && child.left.operator==='!' && child.left.argument.value===0)
          dragField=key(child.right.property);
      });
    }
  });
  walk(source.binding(owner.id,panel),node=>{
    if (node.type==='ObjectExpression' && node.properties.some(p=>key(p.key)==='className'&&p.value.value==='h1-body')) {
      walk(node,child=>{
        if(child.type==='Property'&&key(child.key)==='text'){
          const imported=source.binding(owner.id,child.value.object.name);
          const target=imported.arguments?.[0]?.value;
          descriptionKey=source.literal(target,source.exported(target,key(child.value.property)));
        }
      });
    }
  });
  walk(source.binding(owner.id,row),node=>{
    if(node.type==='BinaryExpression'&&node.operator==='<='&&node.left.property?.name==='length'
      &&node.left.object.callee?.property?.name==='filter')minimumVisible=source.literal(owner.id,node.right);
  });
  if(insetValue!=='-20px'||dragField!=='Active'||descriptionKey!=='SENSITIVITY_DESC'||minimumVisible!==2)
    throw Error('Re-audit DPI-row contracts '+product_id);

  // Read the actual runtime stage schema independently of storage casing.
  const mainPath = directory+'/'+manifest.files['main.js'].replace(/^\.\//,'');
  source.parse(mainPath);
  const reducerOwner=[...source.modules.values()].find(m=>m.definitions.get(initial)?.type==='ObjectExpression'
    &&m.definitions.get(initial).properties.some(p=>key(p.key)==='stages'));
  if(!reducerOwner)throw Error('Missing runtime stage initial state');
  const runtime=source.literal(reducerOwner.id,source.binding(reducerOwner.id,initial));
  const runtimeKeys=Object.keys(runtime.stages[0]);
  if(!['x','y','independent','visible'].every(k=>runtimeKeys.includes(k)))throw Error('Changed runtime row schema');
  receipts.push({symbol:'runtime-stage-schema',...source.receipt(reducerOwner.id,source.binding(reducerOwner.id,initial))});
  const sameSchema=runtimeKeys.every(k=>Object.hasOwn(stageValues[0],k));
  const fieldMap={x:schema.x_key,y:schema.y_key,independent:schema.independent_key,visible:schema.visible_key};
  // The legacy profile's Active becomes runtime visible. It must not also
  // satisfy the source caption's unrelated runtime Active read.
  const dragOrdinalKey=fieldMap[dragField] ?? (sameSchema ? dragField : null);

  const css=[];
  for(const file of [...new Set(Object.values(manifest.files))].filter(f=>f.endsWith('.css'))){
    const cssPath=directory+'/'+file.replace(/^\.\//,'');
    const rules=parseCSS(read(cssPath)).filter(r=>/^\.stage(?:[ .:#-]|$)|^\.stages|^#drag-image|^\.h1-body/.test(r.selector));
    if(rules.length)css.push({path:cssPath,sha256:hash(read(cssPath)),rules});
  }
  if(!css.length)throw Error('Missing current DPI row CSS');
  capabilities.push({product_id,...schema,minimum_visible_stages:minimumVisible,
    rows_margin_x:info.isSensitivitySliderWithGrid?parseFloat(insetValue):0,
    description_key:descriptionKey,drag_ordinal_key:dragOrdinalKey});
  evidence.push({product_id,manifest:{path:manifestPath,sha256:hash(read(manifestPath))},
    page:{path:page.path,sha256:page.sha256,component:page.component,nav_offset:page.nav_offset},
    runtime_stage_keys:runtimeKeys,source_drag_ordinal_key:dragField,receipts,css});
}
function output(file,value){
  const text=JSON.stringify(value,null,2)+'\n';
  if(process.argv.includes('--check')){if(read(file)!==text)throw Error('Stale '+file);}
  else fs.writeFileSync(path.join(root,file),text);
}
output('src/features/mouse_dpi_rows_data.json',capabilities);
output('docs/re/mouse-dpi-rows-capability-evidence.json',{method:'Current source AST/CSS only; no reference execution',products:evidence});
console.log(`DPI rows: ${capabilities.length} audited capabilities; source schemas, description, inset and drag caption registered.`);
