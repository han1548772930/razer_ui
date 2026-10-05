// Offline-only recovery of two CURRENT Dashboard SVGs. Run with
// node --openssl-legacy-provider. Only static parsing and content hashes run.
const fs = require('fs'), path = require('path'), crypto = require('crypto');
const {Source, hash} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const directory = '.ref/applications/synapse/dashboard';
const manifestPath = `${directory}/asset-manifest.json`;
const manifestBytes = fs.readFileSync(path.join(root, manifestPath));
const manifest = JSON.parse(manifestBytes);
const md4 = bytes => crypto.createHash('md4').update(bytes).digest('hex');
const targets = [
  {name:'icon_close.130e45fb.svg', output:'assets/synapse/shortcuts-emoji-close.svg'},
  {name:'animated_startup_dotted_scaling.d5d9ac9c.svg', output:'assets/synapse/service-removing-dots.svg'},
].map(item=>({...item, target:`${directory}/static/media/${item.name}`, fingerprint:item.name.match(/\.([0-9a-f]{8})\.svg$/)[1]}));
const cssPath = `${directory}/static/css/55.4e8559cb.chunk.css`;
const css = fs.readFileSync(path.join(root, cssPath), 'utf8');
const closeRule = parseCSS(css).find(rule=>rule.selector === '.maptext .action_bar_wrapper .emoji-close');
if (!closeRule?.declarations.includes(targets[0].name)) throw Error('Current MapText close rule changed');
if (manifest.files['static/media/animated_startup_dotted_scaling.svg'] !== `./static/media/${targets[1].name}`) throw Error('Current dotted media manifest changed');
const source = new Source('synapse/dashboard');
const dotsReference = {module:61252, ...source.receipt(61252, source.module(61252).fn)};
if (!dotsReference.source.includes(targets[1].name)) throw Error('Current dotted media module changed');
// The manifest's icon_close basename is shared by multiple chunks. The exact
// 130e45fb variant is proved by the current MapText CSS, not by substituting the
// manifest's different 55fe41f1 variant. MD4 naming is calibrated independently.
const calibration = [];
for (const value of Object.values(manifest.files)) {
  const relative = value.replace(/^\.\//, '');
  if (!/^static\/media\/[^/]+\.[0-9a-f]{8}\.svg$/.test(relative)) continue;
  const file = `${directory}/${relative}`;
  if (targets.some(t=>t.target === file) || !fs.existsSync(path.join(root,file))) continue;
  const bytes = fs.readFileSync(path.join(root,file));
  const digest = md4(bytes), fingerprint = path.basename(file).match(/\.([0-9a-f]{8})\.svg$/)[1];
  if (!digest.startsWith(fingerprint)) throw Error(`Current SVG fingerprint mismatch: ${file}`);
  calibration.push({path:file, sha256:hash(bytes), md4:digest, fingerprint});
}
if (!calibration.length) throw Error('No current manifest SVGs to calibrate MD4 naming');
const scanRoots = ['.ref/applications', '.ref/devices', 'assets/synapse'];
const candidates = new Map();
let scanned = 0;
function scan(relative) {
  for (const entry of fs.readdirSync(path.join(root,relative), {withFileTypes:true})) {
    const file = `${relative}/${entry.name}`;
    if (entry.isDirectory()) scan(file); // Never follow symlinks/junctions.
    else if (entry.isFile() && entry.name.endsWith('.svg') && !targets.some(t=>file === t.target || file === t.output)) {
      scanned++;
      const bytes = fs.readFileSync(path.join(root,file)), digest = md4(bytes);
      if (targets.some(t=>t.fingerprint === digest.slice(0,8))) {
        const found = candidates.get(digest.slice(0,8)) || [];
        found.push({candidate:file, sha256:hash(bytes), md4:digest});
        candidates.set(digest.slice(0,8), found);
      }
    }
  }
}
for (const directory of scanRoots) scan(directory);
const recovered = [], missing = [];
for (const target of targets) {
  const found = candidates.get(target.fingerprint) || [];
  if (!found.length) { missing.push(target); continue; }
  if (new Set(found.map(item=>item.sha256)).size !== 1) throw Error(`Ambiguous fingerprint: ${target.name}`);
  const match = found[0], bytes = fs.readFileSync(path.join(root,match.candidate));
  for (const file of [target.target, target.output]) {
    const absolute = path.join(root,file);
    if (fs.existsSync(absolute) && !fs.readFileSync(absolute).equals(bytes)) throw Error(`Existing destination differs: ${file}`);
    if (process.argv.includes('--recover')) fs.writeFileSync(absolute, bytes);
    if (process.argv.includes('--check') && !fs.readFileSync(absolute).equals(bytes)) throw Error(`Recovery drift: ${file}`);
  }
  recovered.push({...target, ...match, candidates:found.map(item=>item.candidate)});
}
const receipt = {
  method:'Offline recovery only: current manifest SVG MD4-prefix calibration, exact current CSS/module target references, unique candidate SHA-256 and full MD4. No network or vendor-code execution.',
  manifest:{path:manifestPath, sha256:hash(manifestBytes)},
  calibration,
  references:[{path:cssPath, sha256:hash(css), rule:closeRule}, dotsReference],
  scan_roots:scanRoots, scanned_svg_files:scanned, recovered, missing,
};
const receiptPath = path.join(root,'docs/re/shortcuts-service-media-offline-recovery.json');
const text = JSON.stringify(receipt,null,2)+'\n';
if (process.argv.includes('--check')) {
  if (fs.readFileSync(receiptPath,'utf8') !== text) throw Error('Offline recovery receipt changed');
} else if (process.argv.includes('--recover')) fs.writeFileSync(receiptPath,text);
console.log(JSON.stringify({calibrated:calibration.length, scanned, recovered, missing, written:process.argv.includes('--recover'), check:process.argv.includes('--check')}));
