// Inspect webpack module-local declarations and export getters as Acorn data.
// Never require, import, evaluate or execute reference JavaScript.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const acorn = require('acorn');
const root = path.resolve(__dirname, '..');
const key = node => node?.name ?? node?.value;
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
function walk(node, visit) {
  if (!node?.type || visit(node) === false) return;
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) value.forEach(child => walk(child, visit));
    else if (value?.type) walk(value, visit);
  }
}
class Source {
  constructor(route) {
    if (!/^[a-z0-9-]+(?:\/[a-z0-9-]+)*$/.test(route)) throw Error('Invalid application route');
    this.directory = `.ref/applications/${route}`;
    const manifest = JSON.parse(fs.readFileSync(path.join(root, this.directory, 'asset-manifest.json'), 'utf8'));
    this.files = [...new Set(Object.values(manifest.files))]
      .filter(file => /^\.\/static\/js\/[^/]+\.js$/.test(file))
      .map(file => `${this.directory}/${file.slice(2)}`);
    this.modules = new Map();
    this.texts = new Map();
    this.parsed = new Set();
  }
  text(file) {
    if (!this.files.includes(file)) throw Error(`Source not declared by application manifest: ${file}`);
    if (!this.texts.has(file)) this.texts.set(file, fs.readFileSync(path.join(root, file), 'utf8'));
    return this.texts.get(file);
  }
  parse(file) {
    if (this.parsed.has(file)) return;
    this.parsed.add(file);
    const text = this.text(file);
    walk(acorn.parse(text, {ecmaVersion: 'latest'}), node => {
      if (node.type !== 'ObjectExpression' || !node.properties.length
          || !node.properties.every(p => p.type === 'Property' && Number.isInteger(key(p.key))
            && /FunctionExpression$/.test(p.value.type))) return;
      for (const property of node.properties) {
        const id = key(property.key), fn = property.value;
        const scope = {id, file, fn, definitions: new Map(), exports: new Map()};
        for (const statement of fn.body.body) {
          if (statement.type === 'VariableDeclaration') {
            for (const declaration of statement.declarations) {
              if (declaration.id.type === 'Identifier') scope.definitions.set(declaration.id.name, declaration.init);
            }
          } else if (['FunctionDeclaration', 'ClassDeclaration'].includes(statement.type)) {
            scope.definitions.set(statement.id.name, statement);
          }
          // Export getters are read in this module's top-level scope only.
          walk(statement, child => {
            if (/Function|Class/.test(child.type)) return false;
            if (child.type !== 'CallExpression' || child.callee.type !== 'MemberExpression'
                || child.callee.object.name !== fn.params[2]?.name || key(child.callee.property) !== 'd'
                || child.arguments[0]?.name !== fn.params[1]?.name
                || child.arguments[1]?.type !== 'ObjectExpression') return;
            for (const p of child.arguments[1].properties) {
              if (p.value.type !== 'ArrowFunctionExpression' || p.value.params.length) throw Error('Unexpected export getter');
              scope.exports.set(key(p.key), p.value.body);
            }
          });
        }
        this.modules.set(id, scope);
      }
      return false;
    });
  }
  module(id) {
    if (!this.modules.has(id)) {
      // Production webpack also emits concise object methods: 20540(e,t,n){...}.
      const locator = new RegExp(`(?:[,{])\\s*${id}\\s*(?::|\\()`);
      for (const file of this.files) {
        if (this.parsed.has(file) || !locator.test(this.text(file))) continue;
        this.parse(file);
        if (this.modules.has(id)) break;
      }
    }
    const scope = this.modules.get(id);
    if (!scope) throw Error(`Missing module ${id} in ${this.directory}`);
    return scope;
  }
  binding(id, name) {
    const node = this.module(id).definitions.get(name);
    if (!node) throw Error(`Missing module-local declaration ${id}:${name}`);
    return node;
  }
  exported(id, name) {
    const node = this.module(id).exports.get(name);
    if (!node) throw Error(`Missing export ${id}:${name}`);
    return node;
  }
  literal(id, node, seen = new Set()) {
    if (!node || seen.has(node)) throw Error('Unresolved or circular literal');
    seen = new Set([...seen, node]);
    const value = child => this.literal(id, child, seen);
    if (node.type === 'Literal') return node.value;
    if (node.type === 'Identifier') return value(this.binding(id, node.name));
    if (node.type === 'UnaryExpression' && node.operator === '!') return !value(node.argument);
    if (node.type === 'UnaryExpression' && node.operator === '-') return -value(node.argument);
    if (node.type === 'ArrayExpression') return node.elements.map(value);
    if (node.type === 'ObjectExpression') return Object.fromEntries(node.properties.map(p => {
      if (p.type !== 'Property' || p.computed) throw Error('Nonliteral object property');
      return [key(p.key), value(p.value)];
    }));
    if (node.type === 'MemberExpression' && node.object.type === 'Identifier' && !node.computed) {
      const imported = this.binding(id, node.object.name);
      if (imported.type === 'CallExpression' && imported.callee.name === this.module(id).fn.params[2]?.name
          && imported.arguments[0]?.type === 'Literal') {
        const target = imported.arguments[0].value;
        return this.literal(target, this.exported(target, key(node.property)), seen);
      }
    }
    throw Error(`Nonliteral ${id}: ${this.snippet(id, node).slice(0, 120)}`);
  }
  snippet(id, node) { return this.text(this.module(id).file).slice(node.start, node.end); }
  receipt(id, node) {
    const file = this.module(id).file;
    return {path: file, sha256: hash(this.text(file)), offset: node.start, end: node.end, source: this.snippet(id, node)};
  }
}
module.exports = {Source, walk, key, hash};
