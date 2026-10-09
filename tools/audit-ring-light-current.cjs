// Read the current production bundle as Acorn data; never execute vendor code.
const fs = require('fs'), path = require('path'), assert = require('assert');
const acorn = require('acorn');
const {walk, hash, key} = require('./webpack-source.cjs');
const {parseCSS} = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const sourcePath = '.ref/applications/natalie/static/js/main.35e04e8c.chunk.js';
const cssPath = '.ref/applications/natalie/static/css/main.fb527a4d.chunk.css';
const inputs = [sourcePath, cssPath, '.ref/applications/natalie/index.html',
  '.ref/applications/natalie/asset-manifest.json', '.ref/applications/natalie/manifest.json'];
const sources = inputs.map(file => {
  const body = fs.readFileSync(path.join(root, file));
  const receipt = JSON.parse(fs.readFileSync(path.join(root, file + '.http.json'), 'utf8'));
  assert.equal(hash(body), receipt.sha256, file);
  assert.equal(receipt.http_status, 200, file);
  return {path: file, sha256: hash(body), bytes: body.length,
    source_url: receipt.source_url, final_url: receipt.final_url};
});
const source = fs.readFileSync(path.join(root, sourcePath), 'utf8');
const css = fs.readFileSync(path.join(root, cssPath), 'utf8');
const ast = acorn.parse(source, {ecmaVersion: 'latest', sourceType: 'script'});
let moduleBody, moduleFn, moduleArray;
function locate(node, ancestors = []) {
  if (!node?.type) return;
  if (node.type === 'VariableDeclarator' && node.id.name === 'nc') {
    assert(!moduleBody, 'Ambiguous application binding');
    moduleBody = [...ancestors].reverse().find(n => n.type === 'BlockStatement');
    moduleFn = [...ancestors].reverse().find(n => n.type === 'FunctionExpression');
    moduleArray = [...ancestors].reverse().find(n => n.type === 'ArrayExpression' && n.elements.includes(moduleFn));
  }
  for (const value of Object.values(node)) {
    if (Array.isArray(value)) value.forEach(child => locate(child, [...ancestors, node]));
    else if (value?.type) locate(value, [...ancestors, node]);
  }
}
locate(ast);
assert.equal(moduleArray?.elements.indexOf(moduleFn), 321);
assert.equal(moduleFn.body, moduleBody);
const bindings = new Map();
for (const statement of moduleBody.body) {
  if (statement.type === 'VariableDeclaration') {
    for (const declaration of statement.declarations) {
      if (declaration.id.type === 'Identifier') bindings.set(declaration.id.name, declaration);
    }
  } else if (statement.type === 'FunctionDeclaration') bindings.set(statement.id.name, statement);
}
const names = ['nc','Ko','Zo','qo','Li','Vo','ko','go','tc','ec','Qo','$o','Ia','Na','Ja',
  'Ca','Pa','La','ja','Ua','Xa','ha','Wa','Ya','nt','rt','Se','at','ht','St','At','Nt',
  'Da','wa','ba','Co','Po','Do','Ei','mo','Vt','Wt','Bi','vo','to','lo','fo','Ro','so','co',
  'S','N','T','A','h','p','O','g','b','Ne','le','ir','or','cr','sr','ur','lr','Er',
  'fa','pa','Ta','Oa','Ci','Pi','Ie','he','me','ye','Fo','Ut','Oe','si','ui','ci','Ce','Pe'];
const anchors = names.map(name => {
  const node = bindings.get(name); assert(node, `Missing module-local binding ${name}`);
  return {name, module: 321, path: sourcePath, sha256: sources[0].sha256,
    offset: node.start, end: node.end, source: source.slice(node.start, node.end)};
});
const hostAnchors = [], hostSources = [];
for (const file of ['preload.js','main.js','constants.js']) {
  const filePath = '.ref/host-4.0.827/electron/' + file;
  const body = fs.readFileSync(path.join(root, filePath)), text = body.toString('utf8');
  const digest = hash(body), hostAst = acorn.parse(text, {ecmaVersion: 'latest'});
  hostSources.push({path: filePath, sha256: digest, bytes: body.length});
  const receipt = (name, node) => hostAnchors.push({name, path: filePath,
    sha256: digest, offset: node.start, end: node.end, source: text.slice(node.start, node.end)});
  if (file === 'preload.js') {
    for (const statement of hostAst.body) {
      if (statement.type !== 'VariableDeclaration') continue;
      for (const declaration of statement.declarations) {
        if (declaration.id.name === 'a' && declaration.init?.type === 'ObjectExpression') {
          assert(!declaration.init.properties.some(p => key(p.key) === 'GET_MONITOR_INFO'));
          receipt('preload_action_enum_missing_monitor_key', declaration);
        }
      }
    }
    walk(hostAst, node => {
      if (node.type === 'CallExpression' && key(node.callee.property) === 'exposeInMainWorld') {
        assert.notEqual(node.arguments[0]?.value, 'FFILibrary');
        if (node.arguments[0]?.value === 'apiElectron') {
          const object = node.arguments[1];
          for (const property of object.properties) {
            if (['getMonitorInfo','getAllDisplays','doDLLAction','doDLLActionAsync',
              'doDLLMainAction','doDLLMainActionAsync'].includes(key(property.key))) receipt('apiElectron_' + key(property.key), property);
          }
        }
      }
    });
  } else if (file === 'constants.js') {
    walk(hostAst, node => {
      if (node.type === 'AssignmentExpression' && key(node.left.property) === 'actionEnum') {
        assert(!node.right.properties.some(p => key(p.key) === 'GET_MONITOR_INFO'));
        receipt('host_action_enum_missing_monitor_key', node);
      }
    });
  } else walk(hostAst, node => {
    if (node.type === 'SwitchCase' && node.test?.type === 'MemberExpression'
      && node.test.object.name === 'D' && key(node.test.property) === 'GET_MONITOR_INFO') receipt('electron_monitor_case_with_undefined_enum_key', node);
  });
}
assert.equal(hostAnchors.length, 9);
const procDeclarations = [];
walk(bindings.get('Ca'), node => {
  if (node.type !== 'CallExpression' || key(node.callee.property) !== 'getProc') return;
  const values = node.arguments.map(n => n.value);
  assert(values.every(value => typeof value === 'string'));
  procDeclarations.push({name: values[0], encoding: values[1], architecture: values[2],
    offset: node.start, end: node.end, source: source.slice(node.start, node.end)});
});
assert(procDeclarations.some(d => d.name === 'GetCameraList' && d.encoding === 'b'));
assert(procDeclarations.some(d => d.name === 'GetCameraList' && d.encoding === 'c'));
assert(anchors.find(a => a.name === 'Pa').source.includes('ha.natalie.init()'));
const rules = parseCSS(css);
const fonts = [];
for (const match of css.matchAll(/@font-face\s*\{[^{}]*\}/g)) {
  fonts.push({offset: match.index, end: match.index + match[0].length, source: match[0]});
}
const result = {schema_version: 1, scope: 'Current Virtual Ring Light route/window/pod/ring/license/settings/storage/legacy FFI source anchors; partial semantic review, not full application recovery',
  source_version_date: '2026-10-02', scanner_sha256: hash(fs.readFileSync(__filename)),
  sources, host_sources: hostSources, host_compatibility_anchors: hostAnchors,
  module: {id: 321, offset: moduleFn.start, end: moduleFn.end}, anchors,
  native_proc_declarations: procDeclarations,
  css: {path: cssPath, sha256: sources[1].sha256, rules, font_faces: fonts},
  limitations: ['All offsets are UTF-16 ranges, not file byte offsets.',
    'FFILibrary encodings are preserved verbatim; host/framework decoding and DLL ABI/body are not proved by these declarations.',
    'Settings subcomponents, subscription/licensing/updater/framework backend and ring rendering are only partially semantically traced.',
    'CSS declarations do not establish browser cascade, computed fonts, layout or visual parity.',
    'Native app binary acquisition and native function bodies remain unverified here; no source defaults substituted for observations.'],
  runtime_validation: 'not_run', complete_semantic_application_claimed: false};
const output = path.join(root, 'docs/re/ring-light-current-evidence.json');
const encoded = JSON.stringify(result, null, 2) + '\n';
if (process.argv.includes('--check')) assert.equal(hash(fs.readFileSync(output)), hash(Buffer.from(encoded)), 'Stale ring light evidence');
else fs.writeFileSync(output, encoded);
console.log(JSON.stringify({anchors: anchors.length, native_proc_declarations: procDeclarations.length,
  css_rules: rules.length, font_faces: fonts.length, runtime_validation: 'not_run'}));
