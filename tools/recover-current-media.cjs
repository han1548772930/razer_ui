// Recover cached original media by the CURRENT manifest's content fingerprint.
// Run with node --openssl-legacy-provider (Node's built-in MD4). Only hashes and
// static source parsing run; no reference JavaScript is imported or executed.
// This is an offline recovery receipt, never an HTTP/live-fetch success receipt.
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const {Source, hash} = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const chroma = process.argv.includes('--chroma');
const route = chroma ? 'chroma-app/dashboard' : 'synapse/dashboard', directory = `.ref/applications/${route}`;
const manifestPath = `${directory}/asset-manifest.json`;
const manifestBytes = fs.readFileSync(path.join(root, manifestPath));
const manifest = JSON.parse(manifestBytes);
const md4 = bytes => crypto.createHash('md4').update(bytes).digest('hex');
function mediaPath(value) {
  const prefix = `/${route}/`;
  const relative = value?.startsWith(prefix) ? value.slice(prefix.length) : value?.replace(/^\.\//,'');
  return /^static\/media\/[^/]+\.[0-9a-f]{8}\.svg$/.test(relative || '') ? `${directory}/${relative}` : null;
}
const requests = chroma ? Object.keys(manifest.files).filter(key=>/^static\/media\/dashboard_.*(?:gray|animated)\.svg$/.test(key)).map(key=>key.slice('static/media/'.length))
  : ['gamer_room_hotspot_animation.svg','icon_device_power_state_off.svg','xbox-icon.svg','ps-icon.svg'];
const targets = requests.map(name => {
  const key = `static/media/${name}`, value = manifest.files[key];
  const target = mediaPath(value);
  if (!target) throw Error(`Unexpected current media URL: ${key}`);
  return {key, target, fingerprint:path.basename(target).match(/\.([0-9a-f]{8})\.svg$/)[1]};
});
// Calibrate the naming algorithm against every available current Dashboard SVG,
// not just the candidate being recovered. An inconsistent file aborts recovery.
const calibration = [];
for (const value of Object.values(manifest.files)) {
  const relative = mediaPath(value);
  if (!relative) continue;
  if (targets.some(t=>t.target===relative) || !fs.existsSync(path.join(root,relative))) continue;
  const bytes = fs.readFileSync(path.join(root,relative));
  const digest = md4(bytes), fingerprint = path.basename(relative).match(/\.([0-9a-f]{8})\.svg$/)[1];
  if (!digest.startsWith(fingerprint)) throw Error(`Current cached SVG naming/hash mismatch: ${relative}`);
  calibration.push({path:relative,sha256:hash(bytes),md4:digest,fingerprint});
}
if (!calibration.length) throw Error('No current media available to establish the naming algorithm');
const receiptPath = path.join(root,chroma ? 'docs/re/current-media-offline-recovery-chroma.json' : 'docs/re/current-media-offline-recovery.json');
if (process.argv.includes('--check')) {
  const receipt = JSON.parse(fs.readFileSync(receiptPath,'utf8'));
  if (receipt.manifest.sha256!==hash(manifestBytes) || JSON.stringify(receipt.calibration)!==JSON.stringify(calibration)) throw Error('Current manifest or calibration changed');
  for (const item of receipt.recovered) {
    const sourceBytes=fs.readFileSync(path.join(root,item.candidate));
    const outputBytes=fs.readFileSync(path.join(root,item.target));
    if (hash(sourceBytes)!==item.sha256 || !sourceBytes.equals(outputBytes) || md4(sourceBytes)!==item.md4 || !item.md4.startsWith(item.fingerprint)) throw Error(`Recovery drift: ${item.target}`);
  }
  console.log(JSON.stringify({calibrated:calibration.length,recovered:receipt.recovered.length,missing:receipt.missing.length,check:true}));
  process.exit(0);
}
const candidates = new Map();
function scan(relative) {
  for (const item of fs.readdirSync(path.join(root,relative),{withFileTypes:true})) {
    const file=`${relative}/${item.name}`;
    if (item.isDirectory()) scan(file); // symlinks/junctions are not followed
    else if (item.isFile() && item.name.endsWith('.svg') && !targets.some(t=>t.target===file)) {
      const bytes=fs.readFileSync(path.join(root,file)), digest=md4(bytes);
      if (targets.some(t=>t.fingerprint===digest.slice(0,8))) {
        const list=candidates.get(digest.slice(0,8))||[];
        list.push({candidate:file,sha256:hash(bytes),md4:digest});candidates.set(digest.slice(0,8),list);
      }
    }
  }
}
// Only current source roots and existing bundled artifacts; obsolete reference
// directories and historical tools are intentionally unreachable from this list.
for (const directory of ['assets/synapse','.ref/applications','.ref/devices']) scan(directory);
const recovered=[],missing=[];
for(const target of targets) {
  const found=candidates.get(target.fingerprint)||[];
  if(!found.length) {missing.push(target);continue;}
  if(new Set(found.map(x=>x.sha256)).size!==1) throw Error(`Ambiguous content fingerprint: ${target.target}`);
  const candidate=found[0], bytes=fs.readFileSync(path.join(root,candidate.candidate));
  const absolute=path.join(root,target.target);
  if(fs.existsSync(absolute) && !fs.readFileSync(absolute).equals(bytes)) throw Error(`Existing current media differs: ${target.target}`);
  if(process.argv.includes('--recover')) {fs.mkdirSync(path.dirname(absolute),{recursive:true});fs.writeFileSync(absolute,bytes);}
  recovered.push({...target,...candidate,candidates:found.map(x=>x.candidate)});
}
const source=new Source(route);
if (chroma) source.files=[...new Set(Object.values(manifest.files))].filter(value=>/^\/chroma-app\/dashboard\/static\/js\/[^/]+\.js$/.test(value)).map(value=>`.ref/applications${value}`);
const references=(chroma ? [[62296,'bs']] : [[19388,'l'],[22534,'H'],[22534,'z']]).map(([module,name])=>({module,name,...source.receipt(module,source.binding(module,name))}));
const receipt={method:'Offline recovery using the current manifest MD4-prefix content fingerprint, calibrated against existing current SVG bytes; candidate SHA-256 and full MD4 retained. Not an independent live-download byte comparison.',manifest:{path:manifestPath,sha256:hash(manifestBytes)},calibration,references,recovered,missing};
if(process.argv.includes('--recover'))fs.writeFileSync(receiptPath,JSON.stringify(receipt,null,2)+'\n');
console.log(JSON.stringify({calibrated:calibration.length,recovered,missing,written:process.argv.includes('--recover')}));
