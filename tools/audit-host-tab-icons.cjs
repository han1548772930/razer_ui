// Audit page-favicon-updated -> TabUI and the current native host icon mapping.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),read=p=>fs.readFileSync(path.join(root,p),'utf8');
function find(file,predicate){const source=read(file),matches=[];walk(acorn.parse(source,{ecmaVersion:'latest'}),n=>{if(predicate(n))matches.push(n)});if(matches.length!==1)throw Error(`Changed host contract ${file}: ${matches.length}`);const n=matches[0];return {path:file,sha256:hash(source),offset:n.start,end:n.end,source:source.slice(n.start,n.end)}}
const hostEvent=find('.ref/host-4.0.827/electron/components/Tab/Tab.js',n=>n.type==='CallExpression'&&n.arguments[0]?.value==='page-favicon-updated');
const hostConsumer=find('.ref/host-4.0.827/electron/components/Tab/TabUI.js',n=>n.type==='MethodDefinition'&&key(n.key)==='changeTabIcon');
const cssFile='.ref/host-4.0.827/electron/index.css',css=read(cssFile);
const defaultIcon=parseCSS(css).find(r=>r.selector==='.etabs-tab-icon'&&r.declarations.includes('rzAppEngine.ico'));
if(!defaultIcon||!hostConsumer.source.includes('e[0]'))throw Error('Current host favicon contract changed');
const expected={Tour:'host-app-tour.svg',Alexa:'host-alexa-favicon.png',FirmwareUpdate:'host-default-tab.png',ProfileMigration:'migration-favicon.svg',Macro:'host-macro-favicon.png',Armory:'host-app-armory.svg',Chroma:'host-app-chroma.svg',Profiles:'host-app-profiles.svg',Feedback:'host-app-feedback.svg'};
const native=read('src/shell/host_tabs.rs');
for(const [tab,icon] of Object.entries(expected))if(!native.includes(`Some(HostTab::${tab}${tab==='Tour'?'(_)':''}) => "synapse/${icon}"`))throw Error('Missing native favicon '+tab);
if(!native.includes('icons::device_favicon')||native.includes('_ => "synapse/host-category-mouse.svg"'))throw Error('Guessed product icon fallback');
if(!native.includes('None => "synapse/host-app-dashboard.svg"'))throw Error('Dashboard must use its production runtime favicon');
const mapping=JSON.parse(read('src/shell/host_device_favicons.json'));
const evidence=JSON.parse(read('docs/re/host-device-favicons-current-evidence.json'));
for(const product of evidence.products){if(mapping[product.product_id]!==product.output.replace('assets/',''))throw Error('Product favicon map mismatch');}
const appSources=JSON.parse(read('docs/re/app-favicons-current-fetch.json')).entries;
for(const entry of appSources){if(hash(fs.readFileSync(path.join(root,entry.path)))!==entry.sha256)throw Error('Changed app favicon');}
const runtimePath='docs/re/runtime-tab-icons-current-evidence.json',runtime=JSON.parse(read(runtimePath));
if(runtime.generator_sha256!==hash(fs.readFileSync(path.join(root,'tools/audit-runtime-tab-icons.cjs'))))throw Error('Stale runtime icon generator receipt');
const runtimeAssets={'synapse/dashboard:runtime':'host-app-dashboard.svg','synapse/armory:workshop':'host-app-armory.svg','synapse/armory:exchange':'host-app-armory-exchange.svg'};
for(const [route,asset] of Object.entries(runtimeAssets)){
  const source=appSources.find(e=>e.route===route);if(!source||hash(fs.readFileSync(path.join(root,'assets/synapse',asset)))!==source.sha256)throw Error('Incorrect runtime favicon '+route);
}
const icoConversions=JSON.parse(read('docs/re/host-tab-ico-current-conversion.json'));
for(const entry of icoConversions.entries)if(hash(fs.readFileSync(path.join(root,entry.output)))!==entry.sha256)throw Error('Changed converted favicon');
const nativeFiles=['src/shell/host_tabs.rs','src/shell/host_tabs/icons.rs','src/shell/host_device_favicons.json','tools/receiver_tab_assets.py'];
const result={generator_sha256:hash(fs.readFileSync(__filename)),hostEvent,hostConsumer,defaultIcon:{path:cssFile,sha256:hash(css),...defaultIcon},nativeIcons:expected,product_count:Object.keys(mapping).length,appSources,runtime:{path:runtimePath,sha256:hash(read(runtimePath)),final_icons:runtime.final_icons},icoConversions,native:nativeFiles.map(path=>({path,sha256:hash(read(path))}))};
const output=JSON.stringify(result,null,2)+'\n',target=path.join(root,'docs/re/host-tab-icons-current-evidence.json');
if(process.argv.includes('--check')){if(fs.readFileSync(target,'utf8')!==output)throw Error('Stale host tab icon receipt');}else fs.writeFileSync(target,output);
console.log(`Verified current host favicon chain, ${Object.keys(expected).length} app tabs and ${Object.keys(mapping).length} product HTML favicons.`);
