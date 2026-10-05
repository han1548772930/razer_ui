// Print the source of a named declaration inside a reference device bundle.
// Static only: parses with Acorn, never requires, imports or evaluates source.
//
//   node tools/extract-device-decl.cjs <file.js> <Name>
//   node tools/extract-device-decl.cjs <bundle-directory> <Name>   # search every .js
const fs = require('fs');
const path = require('path');
const acorn = require('acorn');

const root = path.resolve(__dirname, '..');
const [target, name] = process.argv.slice(2);
if (!target || !name) throw Error('usage: node extract-device-decl.cjs <file|dir> <Name>');
const absolute = path.join(root, target);
const files = fs.statSync(absolute).isDirectory()
  ? fs.readdirSync(absolute).filter(entry => entry.endsWith('.js')).sort().map(entry => path.join(target, entry))
  : [target];

for (const file of files) {
  const text = fs.readFileSync(path.join(root, file), 'utf8');
  const found = [];
  (function walk(node) {
    if (!node || typeof node.type !== 'string') return;
    if (node.type === 'VariableDeclarator' && node.id.type === 'Identifier' && node.id.name === name) {
      found.push({kind: 'var', start: node.init ? node.init.start : node.start, end: node.end});
    }
    if (node.type === 'FunctionDeclaration' && node.id?.name === name) found.push({kind: 'function', start: node.start, end: node.end});
    if (node.type === 'ClassDeclaration' && node.id?.name === name) found.push({kind: 'class', start: node.start, end: node.end});
    if (node.type === 'AssignmentExpression' && node.left.type === 'Identifier' && node.left.name === name) {
      found.push({kind: 'assign', start: node.right.start, end: node.end});
    }
    for (const value of Object.values(node)) {
      if (Array.isArray(value)) value.forEach(child => walk(child));
      else if (value && typeof value.type === 'string') walk(value);
    }
  })(acorn.parse(text, {ecmaVersion: 'latest'}), 0);

  if (!found.length) continue;
  console.log(`${found.length} declaration(s) of ${name} in ${file}`);
  for (const hit of found) {
    const snippet = text.slice(hit.start, hit.end);
    console.log(`--- ${hit.kind} @${hit.start}-${hit.end} (${snippet.length} chars) ---`);
    console.log(snippet.length > 4000 ? snippet.slice(0, 4000) + '\n...[truncated]' : snippet);
  }
}
