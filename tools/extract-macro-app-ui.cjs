// Extract the current Macro application's UI contract without evaluating any
// downloaded JavaScript: the window chrome, its two navigation tabs with the
// app's own zh-CN strings, the function palette definition, and which of the
// palette icons the local reference package actually contains.
const fs=require('fs'),path=require('path'),crypto=require('crypto');
const root=path.resolve(__dirname,'..');
const hash=value=>crypto.createHash('sha256').update(value).digest('hex');
const read=relative=>fs.readFileSync(path.join(root,relative));
const text=relative=>read(relative).toString('utf8');
const receipt=(relative,offset,length)=>{
 const source=text(relative);
 const excerpt=source.slice(offset,offset+length);
 if(excerpt.length!==length)throw Error(`Short receipt ${relative}@${offset}`);
 return {path:relative,offset,end:offset+length,sha256:hash(read(relative)),excerpt};
};
const find=(source,literal,label)=>{const at=source.indexOf(literal);if(at<0)throw Error(`Missing ${label}: ${literal}`);return at;};
const appDir='.ref/applications/synapse/macro';
const mainPath=`${appDir}/static/js/main.3f4b9604.js`;
const main=text(mainPath);

// --- Window chrome classes --------------------------------------------------
const containerChunk=`${appDir}/static/js/8190.61506f5b.chunk.js`;
const container=text(containerChunk);
const chromeLiteral='Br="MacroContainer_my_macro__jCmj8",Vr="MacroContainer_setup_svgs__92r2C"';
const chromeAt=find(container,chromeLiteral,'macro container classes');
const chromeMarkup='(0,Ne.jsxs)("div",{className:Br,children:[(0,Ne.jsx)("div",{className:Vr}),(0,Ne.jsx)(kr,{})]})';
const chromeMarkupAt=find(container,chromeMarkup,'macro container markup');
const cssFiles=fs.readdirSync(path.join(root,`${appDir}/static/css`)).filter(name=>name.endsWith('.css'));
function cssRules(className){
 const found=[];
 for(const name of cssFiles){
  const source=text(`${appDir}/static/css/${name}`);
  const at=source.indexOf(`.${className}`);
  if(at<0)continue;
  let depth=0,end=at;
  for(;end<source.length;end++){
   if(source[end]==='{')depth++;
   else if(source[end]==='}'){depth--;if(depth===0){end++;break;}}
  }
  found.push({file:`${appDir}/static/css/${name}`,offset:at,rule:source.slice(at,end),
   sha256:hash(read(`${appDir}/static/css/${name}`))});
 }
 return found;
}
const chrome={
 container_class:'MacroContainer_my_macro__jCmj8',
 setup_svgs_class:'MacroContainer_setup_svgs__92r2C',
 markup:chromeMarkup,
 receipt:receipt(containerChunk,chromeAt,chromeLiteral.length),
 markup_receipt:receipt(containerChunk,chromeMarkupAt,chromeMarkup.length),
 container_css:cssRules('MacroContainer_my_macro__jCmj8'),
 setup_css:cssRules('MacroContainer_setup_svgs__92r2C'),
};

// --- Navigation tabs and the strings this application ships -----------------
const tabsLiteral='const o="TEXT_NAV_TAB_MY_MACROS",r="TEXT_NAV_TAB_KEY_BINDS"';
const tabsAt=find(main,tabsLiteral,'macro nav tab keys');
const zhChunk=fs.readdirSync(path.join(root,`${appDir}/static/js`)).find(name=>name.startsWith('trans-zh-CN.'));
if(!zhChunk)throw Error('Missing macro zh-CN chunk');
const zhPath=`${appDir}/static/js/${zhChunk}`;
const zh=text(zhPath);
function zhText(key){
 const exportAt=find(zh,`${key}:()=>`, `zh export ${key}`);
 const local=/^([\w$]+)/.exec(zh.slice(exportAt+key.length+5))[1];
 const valueAt=find(zh,`${local}="`,`zh value ${key}`);
 const value=/^"((?:[^"\\]|\\.)*)"/.exec(zh.slice(valueAt+local.length+1));
 if(!value)throw Error(`Unparsed zh value ${key}`);
 return {key,value:JSON.parse(`"${value[1]}"`),receipt:receipt(zhPath,valueAt,local.length+1+value[0].length)};
}
// Keys this application renders but does not translate itself: they are absent
// from both the macro and the Dashboard zh-CN chunks, so their text has no local
// source yet and the UI must not invent it.
const unresolvedKeys=['TEXT_PROFILE_BAR_MACRO','TEXT_PROFILE_BAR_NEW_MACRO','TEXT_PROFILE_BAR_DROPDOWN',
 'TEXT_PROFILE_BAR_DROPDOWN_ADD_MACRO','TEXT_ADD_MENU','TEXT_ADD_FOLDER','TEXT_MACRO_SEARCH','TEXT_MULTI_FUNCTION'];
const dashboardZh='.ref/applications/synapse/dashboard/static/js/trans-zh-CN.9295e59f.chunk.js';
const searchedLocales=[zhPath,dashboardZh].map(relative=>({path:relative,sha256:hash(read(relative)),
 missing:unresolvedKeys.filter(key=>!text(relative).includes(`${key}:()=>`))}));
const tabs={
 keys:[{symbol:'TEXT_NAV_TAB_MY_MACROS',receipt:receipt(mainPath,tabsAt,tabsLiteral.length)},
       {symbol:'TEXT_NAV_TAB_KEY_BINDS',receipt:receipt(mainPath,tabsAt,tabsLiteral.length)}],
 strings:[zhText('TEXT_NAV_TAB_MY_MACROS'),zhText('TEXT_NAV_TAB_KEY_BINDS')],
 unresolved_keys:unresolvedKeys,
 searched_locales:searchedLocales,
 locale_chunk:zhPath,
};

// --- Function palette (documented; icons are missing locally) ---------------
const paletteLiteral='n.d(t,{JB:()=>m,i:()=>f,vB:()=>h})';
const paletteAt=find(main,paletteLiteral,'palette export map');
const paletteBody=main.slice(paletteAt,paletteAt+1800);
const types=/const h=\{([^}]+)\}/.exec(paletteBody);
if(!types)throw Error('Missing palette type map');
const typeMap={};
for(const pair of types[1].split(',')){const [name,value]=pair.split(':');typeMap[name.trim()]=JSON.parse(value);}
function paletteItems(symbol){
 const at=paletteBody.indexOf(`${symbol}=[{`);
 if(at<0)throw Error(`Missing palette group ${symbol}`);
 const end=paletteBody.indexOf('}]',at)+2;
 const body=paletteBody.slice(at,end);
 const items=[];
 for(const match of body.matchAll(/\{type:h\.([A-Z_]+),img:([\w$]+)\.A,name:g\.([\w$]+)\}/g))
  items.push({type:typeMap[match[1]],icon_symbol:match[2],name_symbol:match[3]});
 return items;
}
const groups=[{symbol:'i',items:paletteItems('f')},{symbol:'JB',items:paletteItems('m')}];
// Resolve each icon module to its media path. Icon modules are spread across
// lazily loaded chunks, so every chunk of this application is searched.
const chunkDir=`${appDir}/static/js`;
const chunkFiles=fs.readdirSync(path.join(root,chunkDir)).filter(name=>name.endsWith('.js'));
const paletteImports={};
for(const match of paletteBody.slice(0,paletteBody.indexOf('const h=')).matchAll(/([\w$]+)=n\((\d+)\)/g))
 paletteImports[match[1]]=match[2];
function mediaForModule(moduleId){
 for(const name of chunkFiles){
  const source=text(`${chunkDir}/${name}`);
  const at=source.indexOf(`${moduleId}:(e,t,`);
  if(at<0)continue;
  const media=/static\/media\/[\w.-]+\.svg/.exec(source.slice(at,at+320));
  if(media)return {media:media[0],path:`${chunkDir}/${name}`,offset:at};
 }
 return null;
}
const iconMedia={};
for(const item of groups.flatMap(group=>group.items)){
 const moduleId=paletteImports[item.icon_symbol];
 if(!moduleId)throw Error(`Unresolved icon binding ${item.icon_symbol}`);
 const resolved=mediaForModule(moduleId);
 if(!resolved)throw Error(`Unresolved icon module ${moduleId}`);
 item.icon_module=moduleId;
 item.media=resolved.media;
 item.media_receipt=receipt(resolved.path,resolved.offset,60);
 iconMedia[item.type]=resolved.media;
}
const missingIcons=Object.entries(iconMedia).filter(([,media])=>!fs.existsSync(path.join(root,appDir,media)))
 .map(([type,media])=>({type,media}));
const palette={
 types:typeMap,groups,
 group_titles:{first:'TEXT_ADD_MENU',second:'TEXT_MULTI_FUNCTION'},
 receipt:receipt(mainPath,paletteAt,paletteLiteral.length),
 icons_present:Object.entries(iconMedia).filter(([,media])=>fs.existsSync(path.join(root,appDir,media))).map(([type,media])=>({type,media})),
 icons_missing:missingIcons,
 note:'Manifest-declared SVG assets were fetched as static data on 2026-10-04. Existence does not establish that the palette has been implemented. The older locale scan below is incomplete: application-specific dictionary modules are re-audited separately.',
};

const audit={generated_from:'current source',application:appDir,window:{name:'macro',url:'/synapse/macro/',
 open_param:'policy=3,tab_visible=1',source:'docs/re/display-window-contract.json#named_windows'},
 chrome,tabs,palette};
const jsonPath='docs/re/macro-app-ui-audit.json';
const rendered=JSON.stringify(audit,null,2)+'\n';
if(process.argv.includes('--check')){
 const previous=fs.existsSync(path.join(root,jsonPath))?fs.readFileSync(path.join(root,jsonPath),'utf8'):'';
 if(previous!==rendered){console.error(`${jsonPath} is stale`);process.exit(1);}
 console.log('Verified macro application chrome, navigation tabs and function palette.');
 process.exit(0);
}
fs.writeFileSync(path.join(root,jsonPath),rendered);
console.log(`Macro UI: ${tabs.strings.length} zh-CN strings, ${groups.reduce((total,group)=>total+group.items.length,0)} palette items, ${missingIcons.length} missing icons.`);
