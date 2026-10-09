// Current HTML/CSS read as data only. This does not compute browser styles.
const fs = require('fs'), path = require('path');
const {hash} = require('./webpack-source.cjs');
const {declarations} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const read = p => fs.readFileSync(path.join(root, p), 'utf8');
const exists = p => fs.existsSync(path.join(root, p));
const receipt = (text, start, end) => ({start, end, source:text.slice(start,end)});
function files(directory, name) {
  const result=[];
  for(const entry of fs.readdirSync(path.join(root,directory),{withFileTypes:true})) {
    const p=directory+'/'+entry.name;
    if(entry.isDirectory()) result.push(...files(p,name));
    else if(entry.name===name) result.push(p);
  }
  return result.sort();
}
function boundary(text,start,end,stops) {
  let quote='',paren=0,bracket=0;
  for(let i=start;i<end;i++) {
    const c=text[i];
    if(c==='\\') {i++;continue;}
    if(quote) {if(c===quote)quote='';continue;}
    if(c==='"'||c==="'") {quote=c;continue;}
    if(c==='/'&&text[i+1]==='*') {const j=text.indexOf('*/',i+2);if(j<0)throw Error('CSS comment');i=j+1;continue;}
    if(c==='(')paren++; else if(c===')')paren--;
    if(c==='[')bracket++; else if(c===']')bracket--;
    if(!paren&&!bracket&&stops.includes(c))return i;
  }
  return end;
}
function cssBlocks(text) {
  const out=[];
  function scan(start,end,conditions) {
    while(start<end) {
      // Preserve leading comments in the receipt, but not in selector semantics.
      const stop=boundary(text,start,end,'{;');if(stop===end)break;
      const selector=text.slice(start,stop).replace(/\/\*[\s\S]*?\*\//g,'').trim();
      if(text[stop]===';') {
        if(selector)out.push({kind:'directive',selector,conditions,...receipt(text,start,stop+1)});
        start=stop+1;continue;
      }
      let cursor=stop+1,depth=1;
      while(depth) {
        cursor=boundary(text,cursor,end,'{}');if(cursor===end)throw Error('CSS block '+selector);
        depth+=text[cursor]==='{'?1:-1;cursor++;
      }
      if(/^@(media|supports|container|layer|scope|document)\b/i.test(selector)) {
        out.push({kind:'condition',selector,conditions,...receipt(text,start,stop+1)});
        scan(stop+1,cursor-1,[...conditions,selector]);
      } else {
        const kind=/^@font-face\b/i.test(selector)?'font_face':/^@(?:-\w+-)?keyframes\b/i.test(selector)?'keyframes':selector.startsWith('@')?'other_at_rule':'rule';
        out.push({kind,selector,conditions,...receipt(text,start,cursor),
          ...(kind==='keyframes'?{}:{properties:declarations(text.slice(stop+1,cursor-1))})});
      }
      start=cursor;
    }
  }
  scan(0,text.length,[]);return out;
}
function attrs(tag) {
  const out={};
  for(const m of tag.matchAll(/([^\s=<>/]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+)))?/g))out[m[1].toLowerCase()]=m[2]??m[3]??m[4]??true;
  return out;
}
function localPath(directory,url) {
  if(typeof url!=='string')return {state:'no_url'};
  const clean=url.split(/[?#]/)[0];let p;
  if(/^(?:[a-z]+:)?\/\//i.test(clean)||clean.startsWith('data:'))return {state:'external_or_data',url};
  if(clean.startsWith('/')) {
    // Shared site resources live under .ref/applications; products have separate roots.
    p='.ref/applications'+clean;
  } else p=path.posix.normalize(directory+'/'+clean);
  if(!p.startsWith('.ref/'))return {state:'outside_reference',url};
  return {path:p,state:exists(p)?'local':'not_acquired',url};
}
const css=new Map(),entries=[];
function addCSS(file) {
  if(css.has(file))return;
  if(!exists(file)) {css.set(file,{path:file,state:'not_acquired'});return;}
  const text=read(file),blocks=cssBlocks(text);
  const fonts=blocks.filter(b=>b.kind==='font_face');
  const base=blocks.filter(b=>b.kind==='rule'&&b.selector.split(',').some(s=>/^\s*(?:\*|html|body|div|p|a|span|button|input|select|textarea|:root|#root|::selection)(?:\s|[:.#\[]|$)/.test(s)));
  const variables=blocks.filter(b=>b.kind==='rule'&&b.properties.some(p=>p.property.startsWith('--')));
  const special=blocks.filter(b=>!['rule','font_face'].includes(b.kind));
  const fontRules=blocks.filter(b=>b.kind==='rule'&&b.properties.some(p=>/^(font(?:-.+)?|line-height|letter-spacing|text-transform)$/.test(p.property)));
  const resources=[];
  for(const b of [...fonts,...base,...special])for(const m of b.source.matchAll(/url\(\s*(?:"([^"]*)"|'([^']*)'|([^)]*?))\s*\)/gi)) {
    const url=(m[1]??m[2]??m[3]).trim();
    resources.push({block_start:b.start,...localPath(path.posix.dirname(file),url)});
  }
  const resourceFiles=[...new Set(resources.filter(r=>r.state==='local').map(r=>r.path))].map(p=>({path:p,sha256:hash(fs.readFileSync(path.join(root,p)))}));
  css.set(file,{path:file,state:'parsed_static',sha256:hash(fs.readFileSync(path.join(root,file))),
    rules:blocks.filter(b=>b.kind==='rule').length,font_faces:fonts,base_rules:base,
    custom_property_rules:variables,special_blocks:special,font_rule_count:fontRules.length,
    resources,resource_files:resourceFiles});
}
const htmls=[...files('.ref/devices','index.html'),...files('.ref/applications','index.html')].sort();
for(const html of htmls) {
  const directory=path.posix.dirname(html),text=read(html),tags=[],manifest=directory+'/asset-manifest.json';
  const excluded=[...text.matchAll(/<!--[\s\S]*?-->|<script\b[^>]*>[\s\S]*?<\/script\s*>/gi)].map(m=>[m.index,m.index+m[0].length]);
  for(const m of text.matchAll(/<link\b[^>]*>|<style\b[^>]*>[\s\S]*?<\/style\s*>/gi)) {
    if(excluded.some(([start,end])=>m.index>=start&&m.index<end))continue;
    const a=attrs(m[0].slice(1,m[0].indexOf('>'))),r=receipt(text,m.index,m.index+m[0].length);
    if(/^<style/i.test(m[0])) {
      const bodyStart=m.index+m[0].indexOf('>')+1,bodyEnd=m.index+m[0].toLowerCase().lastIndexOf('</style');
      const rules=cssBlocks(text.slice(bodyStart,bodyEnd)).map(b=>({...b,start:b.start+bodyStart,end:b.end+bodyStart}));
      tags.push({kind:'inline_style',attributes:a,...r,rules});
    } else if(String(a.rel).toLowerCase().split(/\s+/).includes('stylesheet')) {
      const loc=localPath(directory,a.href);tags.push({kind:'stylesheet',attributes:a,...r,location:loc});
      if(loc.state==='local')addCSS(loc.path);
    } else if(a.as==='style')tags.push({kind:'preload_not_applied_stylesheet',attributes:a,...r,location:localPath(directory,a.href)});
  }
  let declared=[];let manifestHash=null;
  if(exists(manifest)) {
    const mt=read(manifest),mj=JSON.parse(mt);manifestHash=hash(fs.readFileSync(path.join(root,manifest)));
    const ep=new Set((mj.entrypoints??[]).map(p=>p.replace(/^\.\//,'')));
    declared=[...new Set(Object.values(mj.files??{}))].filter(p=>typeof p==='string'&&/\.css(?:[?#]|$)/.test(p)).map(url=>({
      ...localPath(directory,url),declaration:ep.has(url.replace(/^\.\//,''))?'manifest_entrypoint':'manifest_asset_runtime_mount_unknown'}));
    for(const d of declared)if(d.state==='local')addCSS(d.path);
  }
  entries.push({html,sha256:hash(fs.readFileSync(path.join(root,html))),
    manifest:manifestHash?{path:manifest,sha256:manifestHash}:null,html_style_order:tags,declared_css:declared,
    computed_styles:'not_computed',lazy_css_insertion_order:'not_proven_by_manifest'});
}
const data={schema_version:1,source_date:'2026-10-02',scanner_sha256:hash(fs.readFileSync(__filename)),
  dependencies:['tools/webpack-source.cjs','tools/css-source.cjs'].map(p=>({path:p,sha256:hash(fs.readFileSync(path.join(root,p)))})),
  offset_unit:'UTF-16 code units, end exclusive; file hashes use UTF-8 bytes',
  scope:'All current local product/application HTML style order plus manifest CSS font, root/inherited and at-rule receipts',
  summary:{html_entries:entries.length,product_html_entries:entries.filter(e=>e.html.startsWith('.ref/devices/')).length,
    application_html_entries:entries.filter(e=>e.html.startsWith('.ref/applications/')).length,
    html_stylesheets:entries.reduce((n,e)=>n+e.html_style_order.filter(t=>t.kind==='stylesheet').length,0),
    inline_style_blocks:entries.reduce((n,e)=>n+e.html_style_order.filter(t=>t.kind==='inline_style').length,0),
    declared_css_references:entries.reduce((n,e)=>n+e.declared_css.length,0),css_paths:css.size,
    unique_css_hashes:new Set([...css.values()].filter(c=>c.sha256).map(c=>c.sha256)).size,
    font_face_receipts:[...css.values()].reduce((n,c)=>n+(c.font_faces?.length??0),0),
    base_rule_receipts:[...css.values()].reduce((n,c)=>n+(c.base_rules?.length??0),0),
    complete_cascade_or_visual_reviews_claimed:0},
  limitations:['HTML order is proven, but lazy chunk insertion and font load completion are not.',
    'A font-face declaration and acquired bytes do not prove a glyph uses that face, weight or locale fallback.',
    'Base selectors are lexical candidates; pseudo states, selector matching, specificity, inheritance and DOM containment require page-level review.',
    'CSS URLs are extracted from selected font/base/at blocks, not all asset references. Imports are recorded but external sheets are not downloaded or evaluated.',
    'No vendor JavaScript, application, build, test or DLL executed.'],entries,css:[...css.values()].sort((a,b)=>a.path.localeCompare(b.path))};
const output='docs/re/ui-style-sources-current.json',serialized=JSON.stringify(data,null,2)+'\n';
if(process.argv.includes('--check')) {if(read(output)!==serialized)throw Error('Stale '+output);}
else fs.writeFileSync(path.join(root,output),serialized);
console.log(JSON.stringify(data.summary));
module.exports={cssBlocks};
