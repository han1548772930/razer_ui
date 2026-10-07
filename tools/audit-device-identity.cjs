// Source-only USB identity and enumeration evidence for the current host.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {walk,key,hash}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..'),receipts=[];
function capture(file,names){
  const text=fs.readFileSync(path.join(root,file),'utf8'),tree=acorn.parse(text,{ecmaVersion:'latest',sourceType:'module'});
  for(const name of names){
    const matches=[];
    walk(tree,n=>{
      if((['MethodDefinition','PropertyDefinition'].includes(n.type)&&key(n.key)===name)||
        (n.type==='FunctionDeclaration'&&n.id?.name===name)||
        (n.type==='SwitchCase'&&n.test?.value===name)||
        (n.type==='AssignmentExpression'&&n.left.type==='MemberExpression'&&key(n.left.property)===name))matches.push(n);
    });
    if(matches.length!==1)throw Error(`${file} ${name}: ${matches.length} matches`);
    const n=matches[0];receipts.push({name,path:file,sha256:hash(text),offset:n.start,end:n.end,source:text.slice(n.start,n.end)});
  }
}
capture('.ref/host-4.0.827/source-evidence/background-current-source.js',
  ['HD','_checkUSBDetail','checkAllRzDevice','getRazerDevices','getHidDevices']);
capture('.ref/host-4.0.827/electron/UsbRzDeviceAction.js',['usb.getDevices','hid.getDevices']);
capture('.ref/host-4.0.827/native-evidence/node_modules/rz-usb-detect/index.js',['find']);
const text=JSON.stringify({method:'Acorn AST only; USB/HID/BLE/IoT/monitor sources kept distinct; no vendor execution',receipts},null,2)+'\n';
const output=path.join(root,'docs/re/device-identity-current-evidence.json');
if(process.argv.includes('--check')){if(fs.readFileSync(output,'utf8')!==text)throw Error('Stale identity evidence');}
else fs.writeFileSync(output,text);
console.log(`Current host identity: ${receipts.length} AST receipts`);
