// Static current-source receipt. Acorn parses text; vendor code is never evaluated.
const fs = require('fs');
const crypto = require('crypto');
const acorn = require('../.work/local-ui/analyzer/node_modules/acorn');
const root = 'local-ui-reverse/source/official/apps.razer.com/synapse/products';
const hash = s => crypto.createHash('sha256').update(s).digest('hex');
const specs = JSON.parse(fs.readFileSync('crates/razer-pages/src/features/mouse_products_data.json', 'utf8'));
const files = [], slices = [], products = [];
const policies=[];
function record(file, source, name, start, end, extra={}) {
  const text = source.slice(start,end);
  const r = {file, name, utf16_range:[start,end], utf8_byte_range:[Buffer.byteLength(source.slice(0,start)),Buffer.byteLength(source.slice(0,end))], slice_sha256:hash(text), ...extra, source:text};
  slices.push(r); return r;
}
function expr(file, source, name, start, extra={}) {
  let ast=acorn.parseExpressionAt(source,start,{ecmaVersion:'latest'});
  if(ast.type==='SequenceExpression') ast=ast.expressions[0];
  return record(file,source,name,ast.start,ast.end,extra);
}
const methods=['loadFilteredMappings','updateMappingsToButtons','updateMappingsToButtonsNew','clearButtonValues','updateButtonWithMappings','assignMappingGroup','assignMouseGroup','assignMacroGroup','assignTextGroup','assignLaunchGroup','assignAILauncherGroup','assignSensitivityGroup','assignDisableGroup','assignHyperShiftGroup'];
for(const spec of specs) {
  const pid=spec.product_id;
  const dir=`${root}/${pid}/ui/static/js`;
  const mains=fs.existsSync(dir)?fs.readdirSync(dir).filter(x=>/^main\..*\.js$/.test(x)):[];
  const main=mains.find(x=>hash(fs.readFileSync(`${dir}/${x}`))===spec.source_sha256)||mains[0];
  const file=main?`${dir}/${main}`:spec.source;
  if(!file||!fs.existsSync(file)) {products.push({product_id:pid,source_missing:true,expected_file:spec.source});continue;}
  const bytes=fs.readFileSync(file), source=bytes.toString('utf8');
  if(hash(bytes)!==spec.source_sha256) {products.push({product_id:pid,source_hash_mismatch:true,file,expected_sha256:spec.source_sha256,actual_sha256:hash(bytes)});continue;}
  files.push({product_id:pid,file,sha256:hash(bytes)});
  const classes=[...source.matchAll(/class ([\w$]+) extends /g)].map(m=>({name:m[1],start:m.index}));
  const found=[];
  for(const method of methods) {
    const needle=`this.${method}=`;
    let offset=0,index;
    while((index=source.indexOf(needle,offset))>=0) {
      const owner=classes.filter(c=>c.start<index).at(-1);
      const r=expr(file,source,method,index+needle.length,{product_id:pid,owner_class:owner?.name,owner_start:owner?.start,occurrence:found.filter(x=>x.name===method).length});
      found.push(r); offset=r.utf16_range[1];
    }
  }
  products.push({product_id:pid,category:spec.info?.category,
    generic_rust_surface:![70,190].includes(pid),
    methods:found.map(r=>({name:r.name,occurrence:r.occurrence,owner_class:r.owner_class,owner_start:r.owner_start,utf16_range:r.utf16_range,slice_sha256:r.slice_sha256,
      resets_enabled:r.source.includes('isEnabled=!0'),
      left_follows_hypershift:r.source.includes('_.isEnabled=this.props.isHyperShiftOn')||/isEnabled=this\.props\.isHyperShiftOn/.test(r.source),
      has_primary_helpers:/\.wP\)|\.j4\)/.test(r.source),
      hyper_group_disables:r.source.includes('isHyperShiftOn&&(e.isEnabled=!1)'),
      source:r.source
    }))});
  if(found.some(r=>r.name==='updateMappingsToButtons')) {
    const expectedProducers={
      mouseGroup:['assignMouseGroup','MOUSE_FUNCTION',/\.eK\([\w$]+\.mouseAssignment\)/],
      sensitivityGroup:['assignSensitivityGroup','SENSITIVITY',/\.Cz\([\w$]+\.sensitivityAssignment\)/],
      macroGroup:['assignMacroGroup','MACRO',/[\w$]+\.name\?[\w$]+\.name:" "/],
      textBlockGroup:['assignTextGroup','TEXT_FUNCTION',/(?:Value"\)\]|Value`\])=[\w$]+\.text/],
      launchGroup:['assignLaunchGroup','LAUNCH_PROGRAM',/void 0!==[\w$]+\.path\?[\w$]+\.path:[\w$]+\.url/],
      aiLauncherGroup:['assignAILauncherGroup','AI_LAUNCHER',/(?:Value"\)\]|Value`\])=[\w$]+\.assignment/],
      disableGroup:['assignDisableGroup','DISABLE',/(?:Value"\)\]|Value`\])=[\w$]+\.[\w$]+/],
      hyperShiftGroup:['assignHyperShiftGroup','RAZER_HYPERSHIFT',/(?:Value"\)\]|Value`\])="Razer Hypershift"/],
    };
    const labelBindings={},labelExports={};
    let constantsInput={file,source};
    let mc=/(?:^|[,\{])4693:/.exec(source);
    if(!mc) {
      const slash=file.lastIndexOf('/'),jsdir=file.slice(0,slash);
      for(const chunk of fs.readdirSync(jsdir).filter(n=>n.endsWith('.js')&&!n.startsWith('main.'))) {
        const chunkFile=`${jsdir}/${chunk}`,chunkBytes=fs.readFileSync(chunkFile),chunkSource=chunkBytes.toString('utf8');
        const match=/(?:^|[,\{])4693:/.exec(chunkSource);
        if(match){mc=match;constantsInput={file:chunkFile,source:chunkSource};files.push({product_id:pid,file:chunkFile,sha256:hash(chunkBytes)});break;}
      }
    }
    if(mc) {
      let ast=acorn.parseExpressionAt(constantsInput.source,mc.index+mc[0].length,{ecmaVersion:'latest'});
      if(ast.type==='SequenceExpression')ast=ast.expressions[0];
      record(constantsInput.file,constantsInput.source,'assignment label constants module4693',ast.start,ast.end,{product_id:pid});
      function walkLabels(node){if(!node||typeof node!=='object')return;if(node.type==='VariableDeclarator'&&node.id.type==='Identifier'&&node.init?.type==='Literal')labelBindings[node.id.name]=node.init.value;if(node.type==='Property'&&node.value?.type==='ArrowFunctionExpression'&&node.value.body.type==='Identifier')labelExports[node.key.name]=node.value.body.name;for(const v of Object.values(node))if(Array.isArray(v))v.forEach(walkLabels);else if(v&&typeof v==='object')walkLabels(v)}walkLabels(ast);
    }
    const verifiedProducers=[];
    for(const [group,[method,label,valuePattern]] of Object.entries(expectedProducers)) {
      const sourceMethod=found.find(r=>r.name===method);
      const labelSymbol=sourceMethod?.source.match(/\.isMapped=!0,[\w$]+\[[\w$]+\]=[\w$]+\.([\w$]+)/)?.[1];
      if(sourceMethod&&labelBindings[labelExports[labelSymbol]]===label&&valuePattern.test(sourceMethod.source))verifiedProducers.push(group);
    }
    const mm=/(?:^|[,\{])9937:/.exec(source);
    let mast=mm?acorn.parseExpressionAt(source,mm.index+mm[0].length,{ecmaVersion:'latest'}):null;
    if(mast?.type==='SequenceExpression') mast=mast.expressions[0];
    const bindings={}, exports={};
    function walk(node) {
      if(!node||typeof node!=='object') return;
      if(node.type==='VariableDeclarator'&&node.id.type==='Identifier'&&node.init?.type==='Literal') bindings[node.id.name]=node.init.value;
      if(node.type==='Property'&&node.value?.type==='ArrowFunctionExpression'&&node.value.body.type==='Identifier') exports[node.key.name]=node.value.body.name;
      for(const value of Object.values(node)) if(Array.isArray(value)) value.forEach(walk);else if(value&&typeof value==='object') walk(value);
    }
    walk(mast);
    const switchMethod=found.find(r=>r.name==='assignMappingGroup');
    let knownOutputs=[...switchMethod.source.matchAll(/\.concat\([\w$]+\.([\w$]+)\)/g)].map(m=>bindings[exports[m[1]]]).filter(v=>typeof v==='string');
    if(mast) record(file,source,'output group constants module9937',mast.start,mast.end,{product_id:pid});
    // Older products store this shared constants module in their chunk. The
    // conservative list records only literal Group names actually in this main;
    // unresolved known fields become an explicit Rust gap, never a made-up value.
    if(!knownOutputs.length) knownOutputs=[...source.matchAll(/"([A-Za-z][A-Za-z0-9]*Group)"/g)].map(m=>m[1]);
    const lookups={};
    for(const [group,markers] of Object.entries({mouseGroup:['Click','RepeatScrollUp','BossKey'],sensitivityGroup:['DPI_Clutch']})) {
      lookups[group]={};
      for(const marker of markers) {
        const i=source.indexOf(`[{id:"${marker}",content:`);
        if(i<0) throw Error(`Missing static ${group} array ${pid}/${marker}`);
        const ast=acorn.parseExpressionAt(source,i,{ecmaVersion:'latest'});
        const arr=ast.type==='SequenceExpression'?ast.expressions[0]:ast;
        record(file,source,`${group} ${marker} literal table`,arr.start,arr.end,{product_id:pid});
        for(const item of arr.elements) {
          const props=Object.fromEntries(item.properties.map(p=>[p.key.name,p.value]));
          if(props.id?.type==='Literal'&&props.content?.type==='Literal') lookups[group][props.id.value]=props.content.value;
        }
      }
    }
    policies.push({product_id:pid,source:file,source_sha256:hash(bytes),category:spec.info.category,
      has_attachment:!!spec.info.attachmentInfo?.hasAttachement,
      use_cycle_scroll_mode_label:!!spec.info.useCycleScrollModeLabel,
      known_outputs:[...new Set(knownOutputs)],
      verified_producers:verifiedProducers,
      lookups,
      methods:found.filter(r=>r.occurrence===0).map(r=>({name:r.name,utf16_range:r.utf16_range,slice_sha256:r.slice_sha256}))});
  }
  if(pid===163) {
    for(const id of [1867,4693]) {
      const match=new RegExp('(?:^|[,\\{])'+id+':').exec(source);
      if(!match) throw Error(`Missing module ${id}`);
      expr(file,source,`webpack module ${id}`,match.index+match[0].length,{product_id:pid,webpack_module:id});
    }
    const m1350=/(?:^|[,\{])1350:/.exec(source);
    expr(file,source,'webpack module 1350 button descriptor loaders',m1350.index+m1350[0].length,{product_id:pid,webpack_module:1350});
    for(const name of ['EP','Uh']) {
      const start=source.indexOf(`class ${name} extends `);
      expr(file,source,`class ${name} caller and lifecycle`,start,{product_id:pid,class_name:name});
    }
    for(const needle of ['Ne=e=>{var a;let E=null===(a=de.find','re=[{id:"Click"']) {
      const i=source.indexOf(needle);if(i>=0) expr(file,source,needle,i+needle.indexOf('=')+1,{product_id:pid});
    }
    for(const needle of ['V=(e,a)=>','Y=e=>{let a=!1;try{for(const E of e)','W=e=>{let a=0;try{for(const t of e)','Xa="LEFT_CLICK"']) {
      const i=source.indexOf(needle); if(i<0) throw Error(`Missing helper ${needle}`);
      expr(file,source,needle,i+needle.indexOf('=')+1,{product_id:pid});
    }
  }
}
const output='docs/re/mouse-button-state-current-source.json';
fs.writeFileSync(output,JSON.stringify({vendor_code_executed:false,method:'Acorn parseExpressionAt current source; zero-based UTF16 / UTF8 byte ranges, end exclusive',files,products,slices},null,2)+'\n');
fs.writeFileSync('crates/razer-pages/src/features/mouse_button_state_current_data.json',JSON.stringify(policies,null,2)+'\n');
console.log(JSON.stringify({output,products:products.length,slices:slices.length,method_counts:products.map(p=>({pid:p.product_id,missing:p.source_missing,methods:p.methods?.length,primary:p.methods?.filter(m=>m.has_primary_helpers).length,hyper:p.methods?.filter(m=>m.left_follows_hypershift).length}))}));
