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
    status: 'partial-ui',
    local: [
      'src/features/chroma_product.rs (popup root, table-driven selection)',
      'src/features/product_workspace.rs (chroma_device_page / has_chroma_device_page)',
      'src/shell/chroma_page.rs (.box-item-device card opens the popup without a gate when a lighting page exists)',
    ],
    rule: 'product has a root chromaApp branch AND a local lighting page; 28 Audio products additionally require the generated root-to-own-Lighting component match (including the LIGHTING key of 1465)',
    boundary: 'accessory families and unaudited Audio roots remain unavailable; connected Audio bodies retain the known incomplete controls listed in product-mode-integration-2026-10-05.md',
  },
  multiDevicePairing: {
    status: 'entry-wired-service-unconnected',
    local: [
      'src/shell/display_window.rs (source-derived pairing tab identity)',
      'src/shell/host_tabs.rs (named product pairing tabs)',
      'src/shell/pairing_page.rs (host-tab page for /synapse/multipairing/)',
    ],
    rule: 'Dashboard device box opens/reuses a named host tab using source container/product/serial identity',
    boundary: 'device services stay unconnected; missing allMasters remains the original empty state',
  },
  macro: {
    status: 'entry-elsewhere-product-bodies-unreviewed',
    local: ['src/shell/macro_page.rs (macro application window)'],
    rule: 'the product-side macro branch only appears inside the macro application iframe, so the same page is opened there',
    boundary: 'macro service, recording and device binding remain unconnected',
  },
  armory: {
    status: 'partial-ui',
    local: [
      'src/features/workspace.rs (armory_mapping_page: mapping surface without product chrome)',
      'src/features/product_workspace.rs (armory_device_page / has_armory_device_page)',
      'src/features/armory_product.rs (independently audited 1303/1304/1313/3893 roots and category-limited 3894 root)',
      'src/shell/armory_page/device_root.rs (device panel, iframe height by device type)',
      'src/shell.rs (share entry opens the device panel next to the share form)',
    ],
    rule:
      'product has a root armory branch AND either a local mapping page or the independently audited armory_product renderer; 3894 requires Accessory/Mousepad category',
    boundary:
      'the panel message handshake is recorded but not exchanged (both sides run in this process); ' +
      'other audio/accessory roots remain unimplemented; five independent roots still lack original product artwork, and 3894 DEFAULT / 3907 remain closed',
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
const audioChroma = JSON.parse(read('src/features/audio_chroma_modes.json'));
const audioModeEvidence = JSON.parse(read('docs/re/product-mode-current-components.json'));
assert(audioChroma.length === 28, 'audio chroma root-to-page scope changed without a new audit');
for (const route of audioChroma) {
  const evidence = audioModeEvidence.products.find(product => product.product_id === route.product_id);
  assert(evidence?.mounts_normal_lighting && evidence.normal_lighting.key === route.page
    && evidence.body_min_width === route.body_min_width,
    `audio chroma route ${route.product_id} does not match its component/CSS receipt`);
}
assert(/supports_chroma_lighting_page\(self\.device\.product_id\)/.test(read('src/features/source_workspace.rs')),
  'audio mode selection no longer checks the audited product list');
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
  count_meaning: 'root_products counts branches found in source, not completed local UIs or per-product fidelity checks. Status describes shared entry coverage only.',
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
    .map(([mode, entry]) => `${mode}: ${entry.root_products} source roots; ${entry.status} (not a completed-product count)`)
    .join('; ')
);
