#!/usr/bin/env node
'use strict';
// Own static audit code only. Reference JavaScript is parsed and never imported.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), acorn = require('acorn');
const ROOT = path.resolve(__dirname, '..');
const OUT = 'docs/re/application-resource-metadata-current-evidence.json';
const sha = value => crypto.createHash('sha256').update(value).digest('hex');
const read = file => fs.readFileSync(path.join(ROOT, file));
const sources = [], receipts = [];
function collect(file, selectors) {
  const raw = read(file), source = raw.toString('utf8');
  const ast = acorn.parse(source, {ecmaVersion: 'latest', sourceType: 'module'});
  sources.push({file, size: raw.length, sha256: sha(raw)});
  const hits = new Map();
  function walk(n) {
    if (!n || typeof n.type !== 'string') return;
    for (const sel of selectors) {
      if (n.type === sel.type && n.start >= sel.min && n.start <= sel.max && (!sel.name || n.id?.name === sel.name)) {
        if (hits.has(sel.id)) throw Error('duplicate selector ' + sel.id);
        hits.set(sel.id, n);
      }
    }
    for (const v of Object.values(n)) {
      if (Array.isArray(v)) v.forEach(walk);
      else if (v && typeof v.type === 'string') walk(v);
    }
  }
  walk(ast);
  for (const sel of selectors) {
    const n = hits.get(sel.id);
    if (!n) throw Error('missing selector ' + sel.id);
    const text = source.slice(n.start, n.end);
    receipts.push({id: sel.id, file, type: n.type, start: n.start, end: n.end, text, sha256: sha(text)});
  }
}
const vd = (id, name, min, max=min) => ({id, name, type:'VariableDeclarator', min, max});
collect('.ref/applications/background-manager/assets/index-8d39b3d5.js', [
  {id:'background-json-helper', name:'qD', type:'FunctionDeclaration', min:432800, max:433200},
  {id:'background-resource-manifest-owner', name:'aC', type:'ClassDeclaration', min:433000, max:433500},
]);
collect('.ref/applications/alisha/static/js/main.83ea24ca.js', [
  vd('alisha-fallback-sparkle','g',674974), vd('alisha-app-sparkle-path','v',676549), vd('alisha-appcast','y',678173),
]);
collect('.ref/applications/sophie-lite/static/js/main.bb22144c.chunk.js', [
  vd('lite-thx-appcast','z',265212), vd('lite-sparkle-appcast','k',267657), vd('lite-native-sparkle-path','Qe',443553),
  {id:'lite-updater-migration-appcast-switch',name:'et',type:'FunctionDeclaration',min:443656,max:443656},
  vd('lite-thx-update-switch','nt',444000,445000),
]);
collect('.ref/applications/sophie/static/js/main.0dad7d2f.chunk.js', [
  vd('sophie-thx-appcast','Y',247948), vd('sophie-native-sparkle-path-constant','W',243556), vd('sophie-sparkle-path-selector','Jn',533591),
  vd('sophie-updater-main-appcast','qn',533755,534000), vd('sophie-updater-thx-appcast-switch','$n',533900,534500),
]);
collect('.ref/applications/natalie/static/js/main.35e04e8c.chunk.js', [
  vd('natalie-main-appcast','we',227356), vd('natalie-dll-appcast','Le',227609), vd('natalie-sparkle-appcast-constant','Ue',228219),
  vd('natalie-sparkle-path-selector','Ya',379385), vd('natalie-main-appcast-init','Ja',379558), vd('natalie-dll-appcast-switch','Za',379848),
]);
const manifestFiles = ['background-manager','alisha','sophie-lite','sophie','natalie','synapse/chroma-studio','synapse/dashboard'].map(a=>'.ref/applications/'+a+'/manifest.json');
const targets = ['RzSparkle.dll','ThxNativeLite.dll','ThxNativeFull.dll','VirtualRingLight.dll','chromaStudioNative.dll','NanoleafNative.dll'];
const manifests = manifestFiles.map(file => {
  const bytes=read(file),m=JSON.parse(bytes);
  const resources=m.resourceManifest?.resources || [];
  return {file,sha256:sha(bytes),version:m.version??null,buildVersion:m.buildVersion??null,resource_manifest_present:!!m.resourceManifest,resource_manifest_version:m.resourceManifest?.version??null,resources:resources.map(r=>({resourceName:r.resourceName,resourceVersion:r.resourceVersion,url:r.url,sha256:r.sha256})),exact_target_basename_matches:resources.filter(r=>targets.includes(path.posix.basename(r.url||'')))};
});
const metadataDir='docs/re/application-resource-metadata-2026-10-09';
const remote = [];
for (const file of ['http-receipts.json','background-resources-http-receipt.json']) {
  const full=metadataDir+'/'+file,raw=read(full),j=JSON.parse(raw);
  for(const r of j.receipts) {
    const body=read(r.body_path);
    if(sha(body)!==r.sha256||body.length!==r.size)throw Error('HTTP body mismatch '+r.id);
    remote.push({...r,receipt_file:full,receipt_file_sha256:sha(raw)});
  }
}
const b = JSON.parse(read(metadataDir+'/background-resources.json'));
const bgResources = b.resources || [];
const assets=JSON.parse(read(metadataDir+'/synapse-assets-index.json'));
const appcastFile=metadataDir+'/appcast-items.json',appcastRaw=read(appcastFile),appcasts=JSON.parse(appcastRaw);
for(const entry of appcasts)if(sha(read(entry.file))!==entry.sha256)throw Error('appcast summary source mismatch '+entry.id);
const inventoryFiles=['docs/re/native-library-pe-current-evidence.json','docs/re/host-native-library-pe-current-evidence.json'];
const inventories=inventoryFiles.map(file=>{const raw=read(file),j=JSON.parse(raw);const entries=j.resources||j.files;return{file,sha256:sha(raw),entries:entries.length,exact_target_matches:entries.filter(r=>JSON.stringify(r).match(/(?:RzSparkle|ThxNativeLite|ThxNativeFull|VirtualRingLight|chromaStudioNative|NanoleafNative)\.dll/i))};});
const evidence={schema_version:1,source_freeze_date:'2026-10-02',metadata_query_date:'2026-10-09',offset_unit:'UTF-16 JavaScript code units',method:'Own Acorn parser + exact source slices; independently fetched JSON/XML metadata only. No package downloads, reference JS execution, DLL loading or device actions.',summary:{source_files:sources.length,source_receipts:receipts.length,remote_metadata_responses:remote.length,remote_200:remote.filter(r=>r.status===200).length,remote_403:remote.filter(r=>r.status===403).length,app_manifests:manifests.length,background_resources:bgResources.length,assets_index_entries:assets.length,target_exact_manifest_matches:manifests.reduce((n,m)=>n+m.exact_target_basename_matches.length,0)},targets,sources,receipts,manifests,remote,appcast_summary:{file:appcastFile,sha256:sha(appcastRaw),entries:appcasts},background_resources:{manifest_version:b.version??null,entries:bgResources,exact_target_basename_matches:bgResources.filter(r=>targets.includes(path.posix.basename(r.url||'')))},assets_index:{entry_count:assets.length,binary_url_entries:assets.filter(r=>/\.(dll|exe|node|zip)$/i.test(r.url||''))},inventories,limitations:['Appcast item version and enclosure name do not prove enclosed DLL basename, PE version, hash or latest stable ownership.','2026-10-09 metadata does not revalidate the frozen 2026-10-02 UI or upgrade the current host.','403 responses are saved as HTTP failure bodies, not parsed as appcast.','No current resource package has been downloaded or statically unpacked in this metadata-only task.']};
const serialized=JSON.stringify(evidence,null,2)+'\n';
if(process.argv.includes('--check')){if(read(OUT).toString('utf8')!==serialized)throw Error('resource metadata evidence drift');console.log(JSON.stringify(evidence.summary));}
else{fs.writeFileSync(path.join(ROOT,OUT),serialized);console.log(JSON.stringify(evidence.summary));}
