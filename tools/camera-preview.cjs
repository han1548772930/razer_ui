// Static camera preview metadata. Only literal AST nodes are interpreted.
const fs=require('fs'),path=require('path'),crypto=require('crypto'),acorn=require('acorn');
const root=path.resolve(__dirname,'..'),products=[],assets=[];
const hash=s=>crypto.createHash('sha256').update(s).digest('hex');
const walk=(n,fn)=>{if(!n?.type)return;fn(n);for(const v of Object.values(n)){if(Array.isArray(v))v.forEach(x=>walk(x,fn));else if(v?.type)walk(v,fn);}};
function prepare({result,audited,label,source,file}) {
 const pid=result.product_id, camera=audited.pages.find(p=>p.key==='CAMERA');
 const component=camera.components.find(c=>c.source.includes('resolutionList'));
 if(pid===3592){if(component)throw Error('Ultra unexpectedly mounts preview source');products.push({product_id:pid,mounted:false});return;}
 if(!component)throw Error(`Missing mounted preview source ${pid}`);
 const rootComponent=camera.components.find(c=>c.source.includes('className:"camera-container'));
 const activation=rootComponent.source.includes('supportCamoStudioActivation:!0');
 if(activation!==(pid!==3596))throw Error('Changed Camo activation mount');
 const receipt=n=>({path:file,offset:n.start,end:n.end,sha256:hash(source.slice(n.start,n.end)),source:source.slice(n.start,n.end)});
 const expression=start=>{let n=acorn.parseExpressionAt(source,start,{ecmaVersion:'latest'});return n.type==='SequenceExpression'?n.expressions[0]:n;};
 const initialMarker=source.indexOf('isPreviewEnabled:null');
 const initialNode=expression(source.lastIndexOf('={',initialMarker)+1);
 if(initialNode.type!=='ObjectExpression')throw Error('Missing preview reducer object');
 const literal=n=>n.type==='Literal'?n.value:n.type==='ArrayExpression'&&n.elements.length===0?[]:n.type==='UnaryExpression'&&n.operator==='!'?!literal(n.argument):undefined;
 const keys=['isPreviewEnabled','isCamoStudioInstalled','cameraDeviceList','selectedCameraDevice','isThirdPartySelected','refreshCamera','reconnectCamera','isHigherGenCableRequired'];
 const initial={};for(const key of keys){const p=initialNode.properties.find(p=>(p.key?.name??p.key?.value)===key);initial[key]=p&&literal(p.value);if(initial[key]===undefined)throw Error(`Unresolved camera initial ${pid}:${key}`);}
 if(initial.isPreviewEnabled!==null||initial.isCamoStudioInstalled!==null||initial.cameraDeviceList.length!==0||initial.isThirdPartySelected!==false)throw Error('Changed preview initial state');
 const marker=source.indexOf('className:"video-container"',component.end);
 let video;
 const candidates=[...source.slice(0,marker).matchAll(/(?:=|,|\()((?:e|\([^()]*\))=>\{)/g)].reverse();
 for(const candidate of candidates){const start=candidate.index+1;try{const n=expression(start);if(n.end>marker){video=receipt(n);break;}}catch{}}
 if(!video?.source.includes('.wh6')||!video.source.includes('.iRy'))throw Error(`Missing preview-off branch ${pid}`);
 const linkMarker=source.indexOf('let E=e.text,a=void 0===E?',source.indexOf('3567('));
 const link=receipt(expression(source.lastIndexOf('e=>{',linkMarker)));
 const svgNodes=[];walk(expression(link.offset),n=>{if(n.type==='CallExpression'&&n.arguments[0]?.value==='svg')svgNodes.push(n.arguments[1]);});
 if(svgNodes.length!==2)throw Error('Changed camera inline link icons');
 for(const [index,node] of svgNodes.entries()){
  const props=Object.fromEntries(node.properties.filter(p=>p.type==='Property').map(p=>[p.key.name??p.key.value,p.value]));let d;
  walk(props.children,n=>{if(n.type==='Property'&&(n.key.name??n.key.value)==='d')d=n.value.value;});
  if(!d||!props.viewBox?.value)throw Error('Nonliteral camera link path');
  const name=index===0?'external':'arrow';
  const bytes=`<svg xmlns="http://www.w3.org/2000/svg" width="${props.width.value}" height="${props.height.value}" viewBox="${props.viewBox.value}"><path fill="currentColor" d="${d}"/></svg>\n`;
  const output=`assets/synapse/camera-preview-${name}.svg`,existing=assets.find(a=>a.output===output);
  if(existing&&existing.sha256!==hash(bytes))throw Error('Camera inline icons differ');
  const origin=receipt(node);
  if(existing)existing.sources.push(origin);else{fs.writeFileSync(path.join(root,output),bytes);assets.push({output,sha256:hash(bytes),sources:[origin]});}
 }
 const pg=result.pages.find(p=>p.key==='CAMERA');
 const spec={activation,promo_title:label('vad'),promo_intro:label(activation?'zpZ':'cYg'),promo_items:['eXB','GRK','k$y','SCX','OvL','Q$X'].map(label),promo_link:label(activation?'_A9':'jpU'),source_title:label('Vug'),source_tooltip:label('bMi'),preview_off:label('wh6'),enable_preview:label('iRy'),higher_gen_cable:label('VZD')};
 pg.camera_preview=spec;
 pg.camera_groups.unshift({key:`${pid}:CAMERA:PREVIEW_SOURCE`,title:spec.source_title,controls:[],collapsible:true,tooltip:spec.source_tooltip,presentation:'preview-source'});
 pg.camera_groups.unshift({key:`${pid}:CAMERA:CAMO_PROMO`,title:spec.promo_title,controls:[],collapsible:true,presentation:'camera-promo'});
 const manifestPath=`.ref/devices/${pid}/asset-manifest.json`,manifest=JSON.parse(fs.readFileSync(path.join(root,manifestPath),'utf8'));
 const cssPath=`.ref/devices/${pid}/${manifest.files['main.css'].replace(/^\.\//,'')}`,css=fs.readFileSync(path.join(root,cssPath),'utf8');
 const rules=[...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)].filter(m=>/tutorial-description|select-camera-container|device-preview-container|show-preview|icon-open-eye|icon-close-eye|btn-reset|camera-preview|camera-reconnect|link-style|\.img-text \.external|invert-horizontal/.test(m[1])).map(m=>({offset:m.index,source:m[0]}));
 products.push({product_id:pid,mounted:true,spec,initial,component,root:rootComponent,reducer:receipt(initialNode),video,link,css:{path:cssPath,sha256:hash(css),rules}});
}
function write(){fs.writeFileSync(path.join(root,'docs/re/camera-preview-current-evidence.json'),JSON.stringify({schema_version:1,scanner_sha256:hash(fs.readFileSync(__filename)),products,assets},null,2)+'\n');}
module.exports={prepare,write};
