// Inspect one current product in one process; never evaluate vendor JavaScript.
// Separate outputs are merged only after a product has resolved successfully.
const fs = require('node:fs');
const path = require('node:path');
const {inspect} = require('./audit-source-profile-menu.cjs');
const pid = Number(process.argv[2]);
if (!Number.isInteger(pid) || pid <= 0) throw Error('Provide one product ID');
const result = inspect(pid);
const directory = path.resolve(__dirname, '../.work/profile-menu-current');
fs.mkdirSync(directory, {recursive:true});
fs.writeFileSync(path.join(directory, `${pid}-evidence.json`), JSON.stringify(result.evidence, null, 2)+'\n');
fs.writeFileSync(path.join(directory, `${pid}-data.json`), JSON.stringify(result.data, null, 2)+'\n');
console.log(JSON.stringify({product_id:pid,menu:result.menu,output:directory}));
