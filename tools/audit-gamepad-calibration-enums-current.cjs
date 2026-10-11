// Resolve exported calibration constants lexically; never evaluate vendor JS.
const fs = require('node:fs');
const path = require('node:path');
const {ProductSource} = require('./audit-source-profile-menu.cjs');
const {hash,walk} = require('./webpack-source.cjs');
const root = path.resolve(__dirname,'..');
const pid = Number(process.argv[2]);
if (![2676,2684].includes(pid)) throw Error('Select an independently audited popup calibration product');
const directory = `local-ui-reverse/source/official/apps.razer.com/synapse/products/${pid}/ui`;
const manifest = JSON.parse(fs.readFileSync(path.join(root,directory,'asset-manifest.json'),'utf8'));
const file = `${directory}/${manifest.files['main.js'].replace(/^\.\//,'')}`;
const source = new ProductSource(pid,file);
const names = ['CONTROLLER_TRIGGERS_ENUM','CONTROLLER_THUMBSTICKS_ENUM','CALIBRATION_PROGRESS_STEP','CALIBRATION_USER_MOVEMENT','ON_SET_CONTROLLER_CALIBRATION'];
const constants = [];
for (const name of names) {
  const matches=[];
  for (const module of source.modules.values()) {
    const expression=module.exports.get(name);
    if(!expression)continue;
    const value=source.literal(expression);
    matches.push({name,value,module:source.receipt(module.fn),expression:source.receipt(source.resolve(expression))});
  }
  if(matches.length!==1)throw Error(`Expected one lexical ${name}, found ${matches.length}`);
  constants.push(matches[0]);
}
const action = constants.find(c=>c.name==='ON_SET_CONTROLLER_CALIBRATION').value;
const actionCases=[];
for(const host of source.cache.values())walk(host.ast,node=>{
  if(node.type!=='SwitchCase'||!node.test)return;
  try{if(host.literal(node.test)===action)actionCases.push(host.receipt(node));}catch(_){}
});
const evidence={product_id:pid,source:{path:file,sha256:hash(source.source)},
  offset_unit:'UTF-16 code units',constants,action_cases:actionCases};
fs.writeFileSync(path.join(root,`docs/re/gamepad-${pid}-calibration-enums-live-source.json`),JSON.stringify(evidence,null,2)+'\n');
console.log(JSON.stringify({product_id:pid,constants:constants.map(({name,value})=>({name,value}))}));
