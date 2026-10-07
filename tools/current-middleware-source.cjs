// Current manifest-scoped source reader with explicit partial acquisition.
// Parsing AST nodes is data inspection; no vendor functions are ever called.
const fs=require('fs'),path=require('path'),acorn=require('acorn');
const {Source,hash,walk,key}=require('./webpack-source.cjs');
const root=path.resolve(__dirname,'..');
class CurrentMiddlewareSource extends Source {
  constructor(product){
    const source=Object.create(new.target.prototype);
    source.product=product;source.directory=`.ref/middleware/${product}`;
    const read=name=>fs.readFileSync(path.join(root,source.directory,name),'utf8');
    source.manifest=JSON.parse(read('webpackManifest.json'));
    source.files=[...new Set(Object.values(source.manifest))].filter(f=>typeof f==='string'&&f.endsWith('.js')).map(f=>{
      if(!/^[\w.-]+\.js$/.test(f))throw Error('Unsafe middleware file '+f);return source.directory+'/'+f;
    });
    source.modules=new Map();source.texts=new Map();source.parsed=new Set();source.acquisition=[];
    source.mainFile=source.directory+'/'+source.manifest['main.js'];
    for(const name of ['index.html','manifest.json','webpackManifest.json'])source.verify(source.directory+'/'+name);
    if(!read('index.html').includes(source.manifest['main.js']))throw Error('HTML/main mismatch');
    return source;
  }
  verify(file){
    const absolute=path.join(root,file),body=fs.readFileSync(absolute),receipt=JSON.parse(fs.readFileSync(absolute+'.http.json','utf8'));
    const url=`https://apps.razer.com/synapse/products/${this.product}/mw/${path.basename(file)}`;
    if(receipt.source_url!==url||receipt.final_url!==url||receipt.http_status!==200||receipt.sha256!==hash(body)||receipt.bytes!==body.length)throw Error('Invalid acquisition '+file);
    if(!this.acquisition.some(r=>r.path===file))this.acquisition.push({path:file,sha256:hash(body),bytes:body.length,source_url:url,fetched_at_utc:receipt.fetched_at_utc});
    return body.toString('utf8');
  }
  text(file){
    if(!this.files.includes(file))throw Error('Source absent from manifest '+file);
    if(!this.texts.has(file))this.texts.set(file,this.verify(file));
    return this.texts.get(file);
  }
  available(file){return fs.existsSync(path.join(root,file))&&fs.existsSync(path.join(root,file+'.http.json'));}
  parse(file){
    if(this.parsed.has(file))return;
    const text=this.text(file),tree=acorn.parse(text,{ecmaVersion:'latest'});
    walk(tree,node=>{
      if(node.type!=='ObjectExpression'||!node.properties.length||!node.properties.every(p=>p.type==='Property'&&Number.isInteger(key(p.key))&&['FunctionExpression','ArrowFunctionExpression'].includes(p.value.type)))return;
      for(const property of node.properties){
        const id=key(property.key),fn=property.value;
        if(this.modules.has(id))throw Error(`Duplicate module ${id}`);
        const scope={id,file,fn,definitions:new Map(),exports:new Map()};
        for(const statement of fn.body.body||[]){
          if(statement.type==='VariableDeclaration'){for(const d of statement.declarations)if(d.id.type==='Identifier')scope.definitions.set(d.id.name,d.init);}
          else if(['FunctionDeclaration','ClassDeclaration'].includes(statement.type))scope.definitions.set(statement.id.name,statement);
          walk(statement,child=>{
            if(/Function|Class/.test(child.type))return false;
            if(child.type==='AssignmentExpression'&&child.operator==='='&&child.left.type==='MemberExpression'&&child.left.object.name===fn.params[1]?.name)scope.exports.set(key(child.left.property),child.right);
            if(child.type!=='CallExpression'||child.callee.type!=='MemberExpression'||child.callee.object.name!==fn.params[2]?.name||key(child.callee.property)!=='d'||child.arguments[0]?.name!==fn.params[1]?.name)return;
            const get=fn=>fn?.body?.type==='BlockStatement'?fn.body.body.find(s=>s.type==='ReturnStatement')?.argument:fn?.body;
            if(child.arguments[1]?.type==='ObjectExpression')for(const p of child.arguments[1].properties){const value=get(p.value);if(value)scope.exports.set(key(p.key),value);}
            else if(child.arguments[1]?.type==='ArrayExpression'){
              // Current webpack also emits d(exports,[name,0,value,name,getter]).
              // Read descriptors only; never invoke a getter or runtime helper.
              const entries=child.arguments[1].elements;
              for(let ix=0;ix<entries.length;){
                const name=entries[ix++],kind=entries[ix++];
                if(name?.type!=='Literal'||typeof name.value!=='string')throw Error('Unknown array export name');
                const value=kind?.type==='Literal'&&kind.value===0?entries[ix++]:get(kind);
                if(!value)throw Error('Unknown array export descriptor');
                scope.exports.set(name.value,value);
              }
            }
            else if(child.arguments[1]?.type==='Literal'){const value=get(child.arguments[2]);if(value)scope.exports.set(child.arguments[1].value,value);}
          });
        }
        this.modules.set(id,scope);
      }
      return false;
    });
    this.parsed.add(file);return tree;
  }
  module(id){
    if(!this.modules.has(id)){
      const locator=new RegExp(`(?:[,{])\\s*${id}\\s*(?::|\\()`);
      for(const file of this.files){if(this.parsed.has(file)||!this.available(file)||!locator.test(this.text(file)))continue;this.parse(file);if(this.modules.has(id))break;}
    }
    const module=this.modules.get(id);if(!module)throw Error('Not acquired/resolved module '+id);return module;
  }
  chunkFile(id){
    const direct=this.manifest[id+'.js'];if(direct)return this.directory+'/'+direct;
    const text=this.text(this.mainFile),tree=this.mainTree||(this.mainTree=acorn.parse(text,{ecmaVersion:'latest'})),names=[];
    walk(tree,n=>{
      if(n.type!=='AssignmentExpression'||n.left.type!=='MemberExpression'||key(n.left.property)!=='u'||!['FunctionExpression','ArrowFunctionExpression'].includes(n.right.type))return;
      walk(n.right,p=>{if(p.type==='Property'&&key(p.key)===id&&typeof p.value.value==='string'&&this.manifest[p.value.value+'.js'])names.push(this.manifest[p.value.value+'.js']);});
    });
    const unique=[...new Set(names)];if(unique.length!==1)throw Error('Cannot resolve manifest chunk '+id);return this.directory+'/'+unique[0];
  }
}
module.exports={CurrentMiddlewareSource};
