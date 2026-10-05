// Static receipts for the current Macro editor. Reference JavaScript is only
// parsed as Acorn AST data by webpack-source.cjs; it is never evaluated.
const fs = require('node:fs');
const path = require('node:path');
const { Source, hash } = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const source = new Source('synapse/macro');
const id = 58190;
const scope = source.module(id);
const bindings = ['St', 'yt', 'rt', 'z', 'En'];
const componentReceipts = Object.fromEntries(bindings.map(name => [name,
  source.receipt(id, source.binding(id, name))]));
const cssPath = '.ref/applications/synapse/macro/static/css/8190.5ef5dbbc.chunk.css';
const css = fs.readFileSync(path.join(root, cssPath), 'utf8');
const rules = [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
  .filter(match => /InputNumberCustom_|InputStepper_|CustomModal_custom_modal/.test(match[1]))
  .map(match => ({ offset: match.index, end: match.index + match[0].length, rule: match[0] }));
const dictionaries = ['NCD', 'KNn', 'zUM', 'Qei', 'oeC', 'rE$', 'a_Y'];
const localeKeys = Object.fromEntries(dictionaries.map(name => [name,
  source.literal(37927, source.exported(37927, name))]));
const assets = ['stepper_up.dcb04520.svg', 'stepper_down.349f755c.svg', 'close.1d7eff2a.svg']
  .map(name => {
    const reference = '.ref/applications/synapse/macro/static/media/' + name;
    const prepared = name.startsWith('stepper_up') ? 'assets/synapse/stepper-up.svg'
      : name.startsWith('stepper_down') ? 'assets/synapse/stepper-down.svg'
        : 'assets/synapse/macro/close.svg';
    const referenceHash = hash(fs.readFileSync(path.join(root, reference)));
    const preparedHash = hash(fs.readFileSync(path.join(root, prepared)));
    if (referenceHash !== preparedHash) throw Error('Different prepared Macro asset: ' + name);
    return { reference, prepared, sha256: referenceHash };
  });
const receipt = {
  method: 'Acorn module-local AST and static CSS parsing; no reference JavaScript execution',
  module: id,
  source: { path: scope.file, sha256: hash(source.text(scope.file)) },
  components: componentReceipts,
  localeKeys,
  css: { path: cssPath, sha256: hash(css), rules },
  assets,
  constraints: {
    delay: { step: 0.001, min: 0, max: 99999.999, decimalPlaces: 3 },
    randomizedDelay: { profileDelaySetting: 2, minFieldMax: 5, maxFieldMax: 5,
      textIntegerDigits: 3, textFractionDigits: 2, inputStep: 0.001,
      minWidth: 60, maxWidth: 60,
      crossingMinimum: 'min >= max sets max to min(min + 1, 5)',
      crossingMaximum: 'max < min sets max to min(min + 1, 5)' },
    loop: { min: 1, max: 99999, step: 1, width: 88, height: 27,
      spinnerWidth: 18, spinnerHeight: 12 },
    launch: { modalWidth: 250, padding: 20, programInputWidth: 142,
      websiteInputWidth: 164, websiteInputHeight: 27,
      programContent: 'Content0', websiteContent: 'Content1', mode: 'RadioIndex',
      programPicker: 'showFileOpenDialog("Select Launch App", false), default types []; local single-file picker and draft guards audited separately in macro-launch-current-evidence.json' }
  }
};
const target = path.join(root, 'docs/re/macro-editors-current-audit.json');
const rendered = JSON.stringify(receipt, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (fs.readFileSync(target, 'utf8') !== rendered) throw Error('Stale Macro editor receipt');
} else {
  fs.writeFileSync(target, rendered);
}
console.log('Macro editor receipts: current 58190 + CSS + 3 byte-identical assets');
