// Audit the local implementation of the product-side `displayMode` roots.
//
// Static only: reads the generated root table, the audited display-mode scan and
// the local sources; it never runs the application or any product bundle.
//
//   node tools/audit-display-mode-roots.cjs           # report + write receipt
//   node tools/audit-display-mode-roots.cjs --check    # fail on drift
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');

const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const digest = text => crypto.createHash('sha256').update(text).digest('hex');
const check = process.argv.includes('--check');

const tablePath = 'src/features/display-mode-roots.json';
const table = JSON.parse(read(tablePath));
const audit = JSON.parse(read(table.source));
const receiptPath = 'docs/re/display-mode-roots-audit.json';

const problems = [];
const assert = (condition, message) => {
  if (!condition) problems.push(message);
};

// 1. The generated table must be the audit's root branches, product id for product id.
const audited = {};
for (const product of audit.products) {
  for (const mode of product.modes) {
    if (mode.kind !== 'root') continue;
    (audited[mode.mode] ??= []).push(product.product_id);
  }
}
for (const key of Object.keys(audited)) audited[key] = [...new Set(audited[key])].sort((a, b) => a - b);
assert(
  JSON.stringify(audited) === JSON.stringify(table.modes),
  'display-mode-roots.json no longer matches docs/re/display-mode-audit.json'
);
assert(table.source_sha256 === digest(read(table.source)), 'root table source hash is stale');
assert(table.generator_sha256 === digest(read('tools/generate-display-mode-roots.cjs')), 'root table generator hash is stale');

// 2. Which modes have a local root, and where that root is documented.
const implementations = {
  chromaApp: {
    status: 'implemented',
    local: [
      'src/features/chroma_product.rs (popup root, table-driven selection)',
      'src/features/product_workspace.rs (chroma_device_page / has_chroma_device_page)',
      'src/shell/chroma_page.rs (.box-item-device card opens the popup without a gate when a lighting page exists)',
    ],
    rule: 'product has a root chromaApp branch AND a local lighting page (adapter tab set or a source TAB_LIGHTING page in a family renderer)',
    boundary: 'audio/accessory families and products without a local lighting page keep the explicit unavailable state',
  },
  multiDevicePairing: {
    status: 'implemented',
    local: [
      'src/shell/display_window.rs (named window + policy flags)',
      'src/shell/pairing_window.rs (root view)',
      'src/shell/pairing_page.rs (embedded /synapse/multipairing/ page)',
    ],
    rule: 'Dashboard device box opens the named `multi-device-pairing*` window',
    boundary: 'keep as the audited second-window exception; device services stay unconnected',
  },
  macro: {
    status: 'implemented-elsewhere',
    local: ['src/shell/macro_page.rs (macro application window)'],
    rule: 'the product-side macro branch only appears inside the macro application iframe, so the same page is opened there',
    boundary: 'macro service, recording and device binding remain unconnected',
  },
  armory: {
    status: 'implemented',
    local: [
      'src/features/workspace.rs (armory_mapping_page: mapping surface without product chrome)',
      'src/features/product_workspace.rs (armory_device_page / has_armory_device_page)',
      'src/shell/armory_page/device_root.rs (device panel, iframe height by device type)',
      'src/shell.rs (share entry opens the device panel next to the share form)',
    ],
    rule:
      'product has a root armory branch AND a local mapping page (adapter Customize tab or a source TAB_CUSTOMIZE page in a family renderer)',
    boundary:
      'the panel message handshake is recorded but not exchanged (both sides run in this process); ' +
      'audio/accessory families and products without a local mapping page keep the entry closed',
  },
};
for (const [mode, ids] of Object.entries(table.modes)) {
  assert(implementations[mode], `display mode ${mode} has no local implementation record`);
  assert(ids.length === audited[mode].length, `${mode} product count drifted`);
}

// 3. The chromaApp popup must stay table-driven and gated on a real local page.
const chroma = read('src/features/chroma_product.rs');
const rootsModule = read('src/features/display_mode_roots.rs');
assert(
  /include_str!\("display-mode-roots\.json"\)/.test(rootsModule),
  'display mode roots module no longer reads the generated table'
);
assert(/pub\(crate\) fn has_root_branch/.test(rootsModule), 'root branch helper is missing');
assert(/pub\(crate\) fn has_chroma_app_root/.test(chroma), 'chromaApp membership helper is missing');
assert(
  /has_root_branch\(DisplayModeRoot::ChromaApp, product_id\)/.test(chroma),
  'chromaApp membership no longer consults the generated table'
);
const workspace = read('src/features/product_workspace.rs');
assert(/has_chroma_app_root\(device\.product_id\)/.test(workspace), 'workspace does not consult the chromaApp table');
assert(/supports_lighting_page\(\)/.test(workspace), 'workspace no longer requires a local lighting page');
const chromaPage = read('src/shell/chroma_page.rs');
assert(/has_chroma_device_page\(cx\)/.test(chromaPage), 'chroma device card does not use the page capability');
assert(
  /min-width: unset/.test(chroma),
  'chroma popup no longer records the source min-width override'
);
assert(/\.min_w_0\(\)/.test(chroma), 'chroma popup does not apply the source min-width override');
assert(/\.px\(css\(20\.\)\)/.test(chromaPage) && /\.pb\(css\(20\.\)\)/.test(chromaPage), '.box-item-device padding drifted');
assert(/opacity\(0\.3\)/.test(chromaPage), '.box-item-device .disabled branch is missing');

// 4. The armory device panel must keep the audited frame heights and entry.
const deviceRoot = read('src/shell/armory_page/device_root.rs');
assert(/DeviceCategory::Keypad => 460\./.test(deviceRoot), 'KEYPAD frame height drifted from 460px');
assert(
  /DeviceCategory::Headset \| DeviceCategory::Audio => 340\./.test(deviceRoot),
  'HEADSET/AUDIO frame height drifted from 340px'
);
assert(/_ => 420\./.test(deviceRoot), 'default frame height drifted from 420px');
const productWorkspace = read('src/features/product_workspace.rs');
assert(
  /has_root_branch\(DisplayModeRoot::Armory/.test(productWorkspace),
  'armory root is not table-driven'
);
assert(
  /workspace\.armory_mapping_page\(window, cx\)/.test(productWorkspace),
  'armory root no longer mounts the mapping surface'
);
const shell = read('src/shell.rs');
assert(
  /page\.open_device_root\(entity\.clone\(\), window, cx\)/.test(shell),
  'the share entry no longer opens the armory device panel'
);

const receipt = {
  schema_version: 1,
  source: table.source,
  source_sha256: table.source_sha256,
  root_table: tablePath,
  root_table_sha256: digest(read(tablePath)),
  generator: 'tools/generate-display-mode-roots.cjs',
  generator_sha256: table.generator_sha256,
  modes: Object.fromEntries(
    Object.entries(table.modes).map(([mode, ids]) => [
      mode,
      {root_products: ids.length, ...implementations[mode]},
    ])
  ),
  problems,
};
const text = JSON.stringify(receipt, null, 2) + '\n';

if (problems.length) {
  for (const problem of problems) console.error(`display-mode root audit: ${problem}`);
  process.exit(1);
}
if (check) {
  const current = fs.existsSync(path.join(root, receiptPath)) ? read(receiptPath) : '';
  if (current !== text) throw Error(`${receiptPath} is stale; rerun tools/audit-display-mode-roots.cjs`);
} else {
  fs.writeFileSync(path.join(root, receiptPath), text);
}
console.log(
  Object.entries(receipt.modes)
    .map(([mode, entry]) => `${mode}: ${entry.root_products} products, ${entry.status}`)
    .join('; ')
);
