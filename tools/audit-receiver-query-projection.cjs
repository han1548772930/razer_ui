// Static source/identity/consumer audit only. Never execute middleware or DLLs.
const fs = require('fs');
const path = require('path');
const { MiddlewareSource } = require('./middleware-source.cjs');
const { hash } = require('./webpack-source.cjs');
const root = path.resolve(__dirname, '..');
const read = file => fs.readFileSync(path.join(root, file), 'utf8');
const mw = new MiddlewareSource(179);
const receipts = ['U', 'he', 'Ne', 'ye'].map(binding => ({
  module: 34340, binding, ...mw.receipt(34340, mw.binding(34340, binding)),
}));
const source = name => receipts.find(receipt => receipt.binding === name).source;
for (const [binding, markers] of [
  ['U', ['get("containerId")']],
  ['he', ['getMultipleDeviceWirelessConnectionStatusV2()', '65535!==e.productId',
    'e.productId!==v.DeviceInfo.dongleId', 't.dongleId===e.productId',
    'productId:r.productId,dongleId:e.productId']],
  ['Ne', ['e.status===x.connected', 'deviceContainerId:U', 'dongleId:e.dongleId||e.productId']],
  ['ye', ['localStorage.setItem(Q', 'master={productId:v.DeviceInfo.productId', 'DEVICE_RUNTIME_DATA']],
]) {
  for (const marker of markers) if (!source(binding).includes(marker)) {
    throw Error(`Changed current source ${binding}: ${marker}`);
  }
}

const catalogPath = '.ref/applications/synapse/dashboard/AvailableDevices.json';
const catalogRaw = read(catalogPath);
const catalog = JSON.parse(catalogRaw);
const acquisition = JSON.parse(read(catalogPath + '.http.json'));
if (acquisition.sha256 !== hash(catalogRaw) || acquisition.http_status !== 200 ||
    acquisition.final_url !== 'https://apps.razer.com/synapse/dashboard/AvailableDevices.json') {
  throw Error('Current available-device acquisition mismatch');
}
const nativeCatalog = JSON.parse(read('src/backend/discovery_catalog.json'));
const cases = [183, 227, 2593, 60000].map(raw_product_id => {
  const sourceCandidates = catalog.filter(row => row.dongleId === raw_product_id).map(row => row.productId);
  const nativeCandidates = nativeCatalog.filter(row => row.dongle_id === raw_product_id).map(row => row.product_id);
  if (JSON.stringify(sourceCandidates) !== JSON.stringify(nativeCandidates)) {
    throw Error('Changed source/native peer identity candidates: ' + raw_product_id);
  }
  return { raw_product_id, source_candidates: sourceCandidates };
});
for (const [raw, expected] of [[183, [182]], [227, [226]], [2593, [2595, 2594]], [60000, []]]) {
  if (JSON.stringify(cases.find(item => item.raw_product_id === raw).source_candidates) !== JSON.stringify(expected)) {
    throw Error('Changed projection regression catalog case: ' + raw);
  }
}
const capabilityPath = 'assets/data/receiver-query-capabilities.json';
const capabilities = JSON.parse(read(capabilityPath)).capabilities;
const selectedCapabilities = [179, 227, 625].map(product_id => {
  const selected = capabilities.find(item => item.product_id === product_id);
  if (!selected) throw Error('Missing source query capability: ' + product_id);
  return selected;
});
if (!selectedCapabilities.find(item => item.product_id === 625).peer_match_product_id) {
  throw Error('Changed keyboard product/scalar-dongle identity rule');
}

const projectionPath = 'src/backend/discovery_receiver_projection.rs';
const projection = read(projectionPath);
const discovery = read('src/backend/discovery.rs');
for (const marker of ['project_receiver_query(pid, &container, &value)',
  'snapshot.errors.extend(peers.errors)', 'snapshot.insert(observed)']) {
  if (!discovery.includes(marker)) throw Error('Startup no longer uses shared projection: ' + marker);
}
for (const marker of ['device_identity::lookup_receiver_peer(', 'capability.peer_match_product_id',
  'raw_pid == 65535 || (raw_pid == receiver_pid && !own_dongle)',
  'ambiguous_products.contains(&peer.product_id)', 'DeviceConnectionObservation::ReceiverPeer(peer.status)',
  'physical_product_id: receiver_pid', 'peer_product_id: Some(peer.raw_product_id)',
  'container: container.to_string()', 'serial: String::new()', 'read_values: None',
  'value["device_count"].as_u64() == Some(rows.len() as u64)', 'id.eq_ignore_ascii_case(container)',
  'pairing.clone().map_err(anyhow::Error::msg)']) {
  if (!projection.includes(marker)) throw Error('Missing projection boundary: ' + marker);
}
const testPath = 'src/backend/discovery_receiver_projection_tests.rs';
const tests = [...read(testPath).matchAll(/#\[test\]\s*fn (\w+)\(/g)].map(match => match[1]);
if (tests.length !== 8) throw Error('Changed projection regression cases');
const nativePaths = ['src/backend/discovery.rs', projectionPath, testPath];
const output = {
  method: 'Current middleware AST, official catalog comparison and static native contract markers; no Rust test, vendor JavaScript, app or DLL execution.',
  generator_sha256: hash(fs.readFileSync(__filename)),
  receipts,
  catalog: { path: catalogPath, sha256: hash(catalogRaw), acquisition, cases },
  capability_dependency: { path: capabilityPath, sha256: hash(read(capabilityPath)), selected: selectedCapabilities },
  native: nativePaths.map(path => ({ path, sha256: hash(read(path)) })),
  tests,
  contract: {
    scope: 'Query name, vendor/product/interface/report size, nonzero braced container and row count must match; path and instance must be present. The native worker separately verifies the actual interface before and after I/O.',
    identity: 'Startup and incremental queries use one pure projection. Preserve raw PID/status and physical receiver container/PID; reject sentinel and standalone-self rows, retain a source-mapped product dongle peer.',
    unknown: 'Malformed frames fail. Unknown/ambiguous catalog identities and conflicting normalized rows retain errors plus unrelated resolved peers; partial absence is not disconnected.',
    metadata: 'No edition/layout/serial/profile/readiness/telemetry is synthesized. Catalog product name/category is descriptive only.',
    consumers: 'Binding payload is independent from observed peer results. UI owns accepted session/revision and topology updates; a payload error cannot erase known peers.',
    side_effects: 'Original ye writes localStorage and registers runtime listeners; the native pure projection does neither. Query status=1 is not SLAVE_CONNECT_EVENT.',
  },
};
const file = path.join(root, 'docs/re/receiver-query-projection-current-evidence.json');
const serialized = JSON.stringify(output, null, 2) + '\n';
if (process.argv.includes('--check')) {
  if (read(path.relative(root, file)) !== serialized) throw Error('Stale receiver query projection evidence');
} else fs.writeFileSync(file, serialized);
console.log(`Receiver query projection: ${receipts.length} current AST receipts, ${cases.length} catalog cases and ${tests.length} compile-only regression cases.`);
