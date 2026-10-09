// Current recording settings are parsed as data; reference JS is never run.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {Source,walk,key,hash}=require('./webpack-source.cjs');
const {parseCSS}=require('./css-source.cjs');
const root=path.resolve(__dirname,'..'),s=new Source('synapse/macro');
function value(id,n){
  if(n.type==='Identifier')return value(id,s.binding(id,n.name));
  if(n.type==='ArrayExpression')return n.elements.map(x=>value(id,x));
  if(n.type==='ObjectExpression')return Object.fromEntries(n.properties.map(p=>[key(p.key),value(id,p.value)]));
  if(n.type==='MemberExpression'&&n.object.type==='MemberExpression')return value(id,n.object)[key(n.property)];
  if(n.type==='CallExpression'&&n.callee.name==='parseInt'&&n.arguments.length===2){
    const [text,radix]=n.arguments.map(x=>value(id,x));
    if(typeof text!=='string'||radix!==16||!/^[0-9a-f]+$/i.test(text))throw Error('Unexpected numeric key constant');
    return Number.parseInt(text,radix);
  }
  try{return s.literal(id,n)}catch(e){
    if(n.type==='MemberExpression'&&n.object.type==='Identifier'){
      const binding=s.binding(id,n.object.name);
      if(binding.type==='CallExpression'){
        const target=binding.arguments[0].value;
        return value(target,s.exported(target,key(n.property)));
      }
      return value(id,binding)[key(n.property)];
    }
    throw e;
  }
}
const groups=value(58190,s.binding(58190,'Za'));
const engineKeys=value(629,s.exported(629,'R'));
const data={groups,keys:engineKeys.filter(k=>k.inputID&&k.keyCode).map(({inputID,keyCode})=>({inputID,keyCode:String(keyCode)})),modifiers:value(58190,s.binding(58190,'pr'))};
const names=['Za','Gr','$r','Ur','Lr','vr','Rr','yr','Pr','Cr','Ir','et','br'];
const contracts=names.map(name=>({module:58190,name,...s.receipt(58190,s.binding(58190,name))}));
contracts.push({module:25572,name:'G',...s.receipt(25572,s.binding(25572,'G'))});
contracts.push({module:5652,name:'feature-hook',...s.receipt(5652,s.module(5652).fn)});
const mf=JSON.parse(fs.readFileSync(path.join(root,s.directory,'asset-manifest.json'),'utf8'));
const css=Object.values(mf.files).filter(f=>/^\.\/static\/css\/(main|8190)\..*\.css$/.test(f)).map(f=>{
  const file=`${s.directory}/${f.slice(2)}`,text=fs.readFileSync(path.join(root,file),'utf8');
  return {path:file,sha256:hash(text),rules:parseCSS(text).filter(r=>/CustomizeDropdown_|DropdownItem_|Tip_|ShortcutKey_|Record_|\.radio|\[type=radio\]|\.stepper|s3-dropdown|s3-options/.test(r.selector))};
});
const result={generator_sha256:hash(fs.readFileSync(__filename)),method:'Acorn module scopes, whitelisted constant hex parsing and manifest CSS only',contracts,css,
  rules:{type_change:'Only current type remains enabled after any event exists; metadata is saved immediately.',phased:'Unobserved global.macro feature keeps the third radio absent.',delay:'Sequence and Phased hide delay/mouse movement groups and reset them to zero.',shortcut:'Browser key-code order; ignore F12 and left GUI; modifiers commit after release, nonmodifiers commit on keyup; no native global registration implied.'}};
for(const [file,object]of [['crates/razer-app-pages/src/macro_page/record_options_data.json',data],['docs/re/macro-record-options-current-evidence.json',result]]){
  const text=JSON.stringify(object,null,2)+'\n',target=path.join(root,file);
  if(process.argv.includes('--check')){if(fs.readFileSync(target,'utf8')!==text)throw Error('Stale '+file)}else fs.writeFileSync(target,text);
}
console.log(`Macro record options: ${contracts.length} scoped contracts, ${data.keys.length} ordered browser keys, ${css.reduce((n,f)=>n+f.rules.length,0)} CSS rules.`);
