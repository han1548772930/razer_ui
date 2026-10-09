#!/usr/bin/env node
'use strict';
// Static external-source review. Never imports, evaluates or runs downloaded code.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const ROOT = path.resolve(__dirname, '..');
const CACHE = path.join(ROOT, '.work/openlogi-static-reference');
const OUTPUT = path.join(ROOT, 'docs/re/openlogi-device-communication-review.json');
const COMMIT = '1505c6525470bc0a38ae3ba79d70e950347d2532';
const REPO = 'AprilNEA/OpenLogi';
const sha256 = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const gitBlob = bytes => crypto.createHash('sha1').update(`blob ${bytes.length}\0`).update(bytes).digest('hex');
const selected = item => item.type === 'blob' && (/^(README.md|Cargo.toml|Cargo.lock|LICENSE)$/.test(item.path) ||
  (/^crates\/(openlogi-hid|openlogi-hidpp|openlogi-device|openlogi-device-registry)\/(Cargo.toml|README.md|src\/.+\.rs)$/.test(item.path) && !/\/tests|\/replay|scripted.rs/.test(item.path)));
const sections = [
  ['scope-readme', 'README.md', 35, 85],
  ['platform-readme', 'README.md', 160, 194],
  ['dependencies', 'Cargo.toml', 32, 58],
  ['async-hid-lockfile', 'Cargo.lock', 5605, 5625],
  ['windows-dependencies', 'crates/openlogi-hid/Cargo.toml', 14, 47],
  ['hid-backend-role', 'crates/openlogi-hid/src/lib.rs', 1, 42],
  ['collection-filter', 'crates/openlogi-hid/src/transport.rs', 174, 231],
  ['enumeration-filter', 'crates/openlogi-hid/src/transport.rs', 275, 330],
  ['native-backend-open', 'crates/openlogi-hid/src/transport/native.rs', 116, 186],
  ['windows-composite-open', 'crates/openlogi-hid/src/transport/windows.rs', 63, 157],
  ['windows-composite-read', 'crates/openlogi-hid/src/transport/windows.rs', 279, 325],
  ['windows-write-fallback', 'crates/openlogi-hid/src/transport/windows_hid.rs', 10, 114],
  ['receiver-identities', 'crates/openlogi-device-registry/src/receiver.rs', 18, 106],
  ['bolt-slot-discovery', 'crates/openlogi-device/src/inventory/probe/bolt.rs', 27, 125],
  ['unifying-arrival-discovery', 'crates/openlogi-device/src/inventory/probe/unifying.rs', 37, 161],
  ['direct-device-probe', 'crates/openlogi-device/src/inventory/probe/direct.rs', 1, 130],
  ['report-framing', 'crates/openlogi-hidpp/src/channel/message.rs', 1, 109],
  ['hidpp-v20-header', 'crates/openlogi-hidpp/src/protocol/v20.rs', 15, 136],
  ['response-lease-timeout', 'crates/openlogi-hidpp/src/channel.rs', 1, 113],
  ['feature-discovery', 'crates/openlogi-hidpp/src/feature/root.rs', 20, 82],
  ['dpi-command', 'crates/openlogi-hidpp/src/feature/adjustable_dpi.rs', 1, 63],
  ['dpi-feature-selection', 'crates/openlogi-device/src/write/dpi.rs', 25, 134],
  ['smartshift-command', 'crates/openlogi-hidpp/src/feature/smartshift.rs', 1, 74],
  ['reprog-controls-command', 'crates/openlogi-hidpp/src/feature/reprog_controls.rs', 1, 93],
  ['battery-command-event', 'crates/openlogi-hidpp/src/feature/unified_battery.rs', 1, 88],
];
function fileRecord(relative, external) {
  const bytes = fs.readFileSync(path.join(external ? path.join(CACHE, 'source') : ROOT, relative));
  return {path: relative, bytes: bytes.length, sha256: sha256(bytes), git_blob_sha1: external ? gitBlob(bytes) : null};
}
function receipt(id, relative, first, last) {
  const bytes = fs.readFileSync(path.join(CACHE, 'source', relative));
  const source = bytes.toString('utf8');
  const lines = source.match(/[^\n]*\n|[^\n]+$/g) || [];
  assert(first >= 1 && last <= lines.length, `${id}: invalid lines`);
  const start = lines.slice(0, first - 1).join('').length;
  const text = lines.slice(first - 1, last).join('');
  return {id, path: relative, sha256: sha256(bytes), start_line: first, end_line: last,
    utf16_start: start, utf16_end: start + text.length, text,
    source_url: `https://github.com/${REPO}/blob/${COMMIT}/${relative}#L${first}-L${last}`};
}
function generate() {
  const tree = JSON.parse(fs.readFileSync(path.join(CACHE, 'tree.json'), 'utf8'));
  const commit = JSON.parse(fs.readFileSync(path.join(CACHE, 'commit.json'), 'utf8'));
  assert.equal(commit.sha, COMMIT);
  assert.equal(tree.truncated, false);
  const files = tree.tree.filter(selected).map(item => {
    const record = fileRecord(item.path, true);
    assert.equal(record.git_blob_sha1, item.sha, `Git blob mismatch: ${item.path}`);
    assert.equal(record.bytes, item.size, `Size mismatch: ${item.path}`);
    return {...record, source_url: `https://raw.githubusercontent.com/${REPO}/${COMMIT}/${item.path}`};
  });
  const featureInventory = files.filter(file => /^crates\/openlogi-hidpp\/src\/feature\/[^/]+\.rs$/.test(file.path)).map(file => {
    const source = fs.readFileSync(path.join(CACHE, 'source', file.path), 'utf8');
    return {path: file.path, sha256: file.sha256,
      static_feature_attributes: [...source.matchAll(/#\[creatable\(id\s*=\s*(0x[0-9a-f]+),\s*version\s*=\s*(\d+)\)\]/gi)].map(m => ({id: m[1], version: Number(m[2]), utf16_start: m.index, utf16_end: m.index + m[0].length, text: m[0]})),
      public_async_names: [...source.matchAll(/pub async fn\s+([a-zA-Z0-9_]+)/g)].map(m => ({name: m[1], utf16_start: m.index, utf16_end: m.index + m[0].length, text: m[0]}))};
  });
  const razerFiles = [
    '.ref/host-4.0.827/electron/modules/ffi/FFIPreloadMain.js',
    '.ref/host-4.0.827/electron/modules/ffi/ffiMain.js',
    '.ref/middleware/70/7254.928934d7d583696a2c98.js',
    '.ref/middleware/70/6120.7fff31f083f844383e26.js',
    '.ref/middleware/3886/main.df6f64c941b9efded61e.js',
  ].map(relative => fileRecord(relative, false));
  const source = fs.readFileSync(path.join(ROOT, razerFiles[0].path), 'utf8');
  const begin = source.indexOf('_handleAction_ConfigureFFI=');
  const end = source.indexOf(';_handleAction_ConfigureFFI_APIToCallWhenExit=', begin);
  assert(begin >= 0 && end > begin);
  return {schema_version: 1, audit_date: '2026-10-09', scope: 'External Logitech HID++ source review; not Razer implementation authority',
    repository: REPO, branch_observed: 'master', pinned_commit: COMMIT, pinned_tree: tree.sha,
    commit_date: commit.commit.committer.date, network_acquisition: 'GitHub API tree + commit-pinned raw bytes; no downloaded code executed',
    scanner_sha256: sha256(fs.readFileSync(__filename)), files, receipts: sections.map(args => receipt(...args)),
    feature_inventory: featureInventory,
    feature_inventory_limit: 'Static attributes and public async method names only; not proof of caller activation, wire semantics, supported devices or runtime success',
    razer_comparison: {files: razerFiles, configure_ffi_receipt: {path: razerFiles[0].path, sha256: razerFiles[0].sha256, utf16_start: begin, utf16_end: end, text: source.slice(begin, end)},
      supporting_audits: ['host-ffi-current.md', 'native-factory-current.md', 'native-3886-legacy-hue-current.md', 'dll-function-inventory.md', 'usb-native-current-evidence.json', 'receiver-native-hid-current-evidence.json'],
      equivalence: 'None established between Logitech HID++ bytes/features/receiver addressing and any Razer device protocol'},
    conclusions: {transport: 'async-hid host HID backend, with explicit Win32 HID fallback',
      discovery: 'Logitech VID, vendor collection and receiver protocol predicates; discovery is not a DLL-independent universal device API',
      allowed_reference: ['Transport/protocol/inventory separation', 'Typed errors, real response validation and connection-scoped identities', 'Report endpoint selection backed by each device descriptor', 'Read/write/event classification and response correlation'],
      prohibited_inference: ['Reuse Logitech VID/PID, report IDs, feature IDs, device slots or command bytes for Razer without Razer evidence', 'Treat feature/module inventory as all features active or hardware verified', 'Remove Windows API solely because the protocol is in Rust', 'Treat DLL export or FFI declaration as a complete reverse engineering of the DLL body'],
      scope_boundary: 'No Rust/runtime/assets edits; no DLL or device execution. DLL-backed Razer writes remain deferred by AGENTS.md.'},
    unknowns: ['openlogi-async-hid dependency implementation was not acquired; lockfile identity is not an audit of its internals',
      'Desktop/agent action execution, input injection, camera/UVC and all UI details are outside this device communication subset',
      'No real hardware execution; README hardware validation claims are upstream claims only',
      'Razer transport replacement needs per-command DLL machine-code evidence, report framing, endpoint selection, request/response parsing and product applicability; this external review supplies none of those Razer facts']};
}
async function fetchPinned() {
  const previous = JSON.parse(fs.readFileSync(OUTPUT, 'utf8'));
  assert.equal(previous.pinned_commit, COMMIT);
  for (const file of previous.files) {
    const target = path.join(CACHE, 'source', file.path);
    if (fs.existsSync(target) && sha256(fs.readFileSync(target)) === file.sha256) continue;
    const response = await fetch(file.source_url);
    assert.equal(response.status, 200, file.source_url);
    const bytes = Buffer.from(await response.arrayBuffer());
    assert.equal(sha256(bytes), file.sha256, file.path);
    assert.equal(gitBlob(bytes), file.git_blob_sha1, file.path);
    fs.mkdirSync(path.dirname(target), {recursive: true});
    fs.writeFileSync(target, bytes);
  }
  for (const [name, url] of [['tree.json', `https://api.github.com/repos/${REPO}/git/trees/${COMMIT}?recursive=1`], ['commit.json', `https://api.github.com/repos/${REPO}/commits/${COMMIT}`]]) {
    const response = await fetch(url, {headers: {'User-Agent': 'razer-ui-static-source-audit'}});
    assert.equal(response.status, 200, url);
    const value = await response.json();
    assert.equal(value.sha, name === 'tree.json' ? previous.pinned_tree : COMMIT);
    fs.writeFileSync(path.join(CACHE, name), JSON.stringify(value, null, 2) + '\n');
  }
}
(async () => {
  if (process.argv.includes('--fetch')) await fetchPinned();
  const evidence = generate();
  if (process.argv.includes('--check')) assert.deepEqual(evidence, JSON.parse(fs.readFileSync(OUTPUT, 'utf8')));
  else fs.writeFileSync(OUTPUT, JSON.stringify(evidence, null, 2) + '\n');
  console.log(`OpenLogi static review ${process.argv.includes('--check') ? 'verified' : 'written'}: ${evidence.files.length} files, ${evidence.receipts.length} source receipts, ${evidence.feature_inventory.length} feature modules; commit ${COMMIT}`);
})().catch(error => {console.error(error.stack); process.exitCode = 1;});
