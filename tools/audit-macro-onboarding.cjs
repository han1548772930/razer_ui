// Parse current vendor source as data; never evaluate downloaded JavaScript.
const fs = require('node:fs');
const path = require('node:path');
const { Source, hash } = require('./webpack-source.cjs');
const { parseCSS } = require('./css-source.cjs');
const root = path.resolve(__dirname, '..');
const source = new Source('synapse/macro');
const manifestPath = `${source.directory}/asset-manifest.json`;
const manifestText = fs.readFileSync(path.join(root, manifestPath), 'utf8');
const manifest = JSON.parse(manifestText);
const bindings = Object.fromEntries(['Ya', 'qa', 'Fr', 'Me'].map(name => [
  `58190.${name}`, source.receipt(58190, source.binding(58190, name)),
]));
let firstTime = source.exported(25572, 'An');
if (firstTime.type === 'Identifier') firstTime = source.binding(25572, firstTime.name);
bindings['25572.An'] = source.receipt(25572, firstTime);
const css = Object.values(manifest.files)
  .filter(file => /^\.\/static\/css\/(main|8190)\..*\.css$/.test(file))
  .map(file => {
    const name = `${source.directory}/${file.slice(2)}`;
    const text = fs.readFileSync(path.join(root, name), 'utf8');
    return {
      file: name,
      sha256: hash(text),
      rules: parseCSS(text).filter(rule => /MacroContent/.test(rule.selector)
        && /onboarding|step_[123]|recording|disabled/.test(rule.selector)),
    };
  });
const result = {
  route: 'synapse/macro',
  method: 'Current manifest, scoped Acorn bindings and static CSS; no runtime/visual certification',
  manifest: { path: manifestPath, sha256: hash(manifestText) },
  bindings,
  css,
};
const target = path.join(root, 'docs/re/macro-onboarding-current-evidence.json');
if (process.argv.includes('--check')) {
  if (JSON.stringify(JSON.parse(fs.readFileSync(target, 'utf8'))) !== JSON.stringify(result)) {
    throw Error('Stale current Macro onboarding evidence');
  }
} else {
  fs.writeFileSync(target, JSON.stringify(result, null, 2) + '\n');
}
console.log('Current Macro onboarding: 5 scoped bindings and CSS receipts verified');
