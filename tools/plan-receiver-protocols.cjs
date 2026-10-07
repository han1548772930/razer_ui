// Follow every current product's dual-link factory. No product whitelist and no
// execution of downloaded code. This is an acquisition plan, not capabilities.
const fs = require('fs'), path = require('path');
const {CurrentMiddlewareSource} = require('./current-middleware-source.cjs');
const {walk, key} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const read = f => JSON.parse(fs.readFileSync(path.join(root, f), 'utf8'));
const inventory = read('docs/re/middleware-device-bindings-current.json');
const bootstrap = read('.ref/middleware/bootstrap-requests.json');
const products = [], requests = [];
const nodes = (node, predicate) => { const out=[]; walk(node,n=>{if(predicate(n))out.push(n)}); return out; };
const only = (items,label) => { if(items.length!==1)throw Error(`${label}: ${items.length} candidates`);return items[0]; };
for (const product of inventory.products) {
  const features = (product.feature_bindings || []).filter(f => f.options.isDualLinkDevice === true);
  if (!features.length) continue;
  const row = {product_id:product.product_id};
  try {
    const feature = only(features,'feature'), info = only(product.device_info_candidates,'DeviceInfo');
    row.feature=feature; row.device_info=info;
    if (!Number.isInteger(info.values.dongleId)) {
      row.status='no_direct_dongle'; products.push(row); continue;
    }
    if (feature.class_key !== 'rzDevice25') throw Error('Different feature protocol');
    const source = new CurrentMiddlewareSource(product.product_id);
    source.parse(source.mainFile);
    for (const file of bootstrap.products.find(p=>p.product_id===product.product_id).files) source.parse(file);
    const factories=[];
    for (const [id,module] of source.modules) for (const [name,node] of module.definitions) {
      if (!node || !['ArrowFunctionExpression','FunctionExpression'].includes(node.type)) continue;
      const text=source.snippet(id,node);
      if(text.includes('getFeatures("rzDeviceType")')&&text.includes('.connectDevice()')) factories.push({id,name,node});
    }
    const factory=only(factories,'factory'), factoryText=source.snippet(factory.id,factory.node);
    row.factory={module:factory.id,...source.receipt(factory.id,factory.node)};
    if(!factoryText.includes('DeviceInfo.dongleId')||!factoryText.includes('DeviceInfo.claimInterface'))throw Error('Factory identity routing changed');
    let selected;
    if(info.values.category==='KEYBOARD') {
      selected='rzDevice25DualLinkKeyboard';
      if(!factoryText.includes('rzDevice25DualLinkKeyboard()'))throw Error('Keyboard factory branch absent');
    } else {
      const kind=feature.options.duallinkDeviceType||'MOUSE';
      if(kind==='MOUSE') selected='rzDevice25DualLinkMouse';
      else if(kind==='LINKERMULTIDEVICES') selected='rzDevice25LinkerMultiDevices';
      else if(kind==='LINKER') {
        const special=only(nodes(factory.node,n=>n.type==='IfStatement'&&
          n.test.type==='BinaryExpression'&&n.test.operator==='==='&&
          n.test.left.type==='Literal'&&key(n.test.right.property)==='productId'&&
          source.snippet(factory.id,n.consequent).includes('.rzDevice25LinkerUma()')),'linker discriminator');
        selected=info.values.dongleId===special.test.left.value?'rzDevice25LinkerUma':'rzDevice25Linker';
        row.discriminator=source.receipt(factory.id,special.test);
      } else throw Error('Unknown dual-link kind '+kind);
    }
    const tables=[];
    for(const [name,node]of source.module(factory.id).definitions){
      if(node?.type!=='ObjectExpression'||!nodes(factory.node,n=>n.type==='Identifier'&&n.name===name).length)continue;
      for(const p of nodes(node,n=>n.type==='Property'&&key(n.key)===selected)){
        const imports=nodes(p.value,n=>n.type==='CallExpression'&&key(n.callee.property)==='bind'&&Number.isInteger(n.arguments[1]?.value));
        if(imports.length===1)tables.push({node:p,module:imports[0].arguments[1].value});
      }
    }
    const loader=only(tables,'selected loader');
    const chunks=[...new Set(nodes(loader.node,n=>n.type==='CallExpression'&&key(n.callee.property)==='e'&&Number.isInteger(n.arguments[0]?.value)).map(n=>n.arguments[0].value))];
    row.selected_class=selected; row.class_module=loader.module;
    row.loader=source.receipt(factory.id,loader.node);
    row.files=chunks.map(id=>source.chunkFile(id));
    for(const file of row.files)requests.push({product_id:product.product_id,file:path.basename(file)});
    row.status='selected_factory_branch'; row.acquisition=source.acquisition;
  } catch(error) { row.status='unresolved'; row.reason=error.message; }
  products.push(row);
}
const result={method:'Static factory and loader ASTs across the entire current product inventory; no vendor execution',scope_products:inventory.scope_products,products,requests};
fs.writeFileSync(path.join(root,'.ref/middleware/receiver-protocol-requests.json'),JSON.stringify(result,null,2)+'\n');
console.log(JSON.stringify({products:products.length,requests:requests.length,unresolved:products.filter(p=>p.status==='unresolved').map(p=>({product_id:p.product_id,reason:p.reason}))}));
