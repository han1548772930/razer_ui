// Static inspection of adjacent broadcast/haptic products, never source execution.
const fs=require('fs'),path=require('path'),crypto=require('crypto');
const {inspect}=require('./extract-audio-evidence.cjs');
const root=path.resolve(__dirname,'..');
const selected=[3334,3337,3909,3942,3949,1382];
const products=JSON.parse(fs.readFileSync(path.join(root,'docs/re/source-product-configs.json'),'utf8')).products.filter(p=>selected.includes(p.product_id));
const data=products.map(inspect);
const sha=f=>crypto.createHash('sha256').update(fs.readFileSync(f)).digest('hex');
fs.writeFileSync(path.join(root,'docs/re/audio-additional-evidence.json'),JSON.stringify({schema_version:1,scanner_sha256:sha(__filename),parser_sha256:sha(path.join(__dirname,'extract-audio-evidence.cjs')),products:data},null,2)+'\n');
console.log(`Parsed ${data.length} adjacent broadcast and haptic products.`);
