"""Project every current AvailableDevices row as identity data, never observations."""
import argparse
from collections import defaultdict
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
AVAILABLE = ROOT / '.ref/applications/synapse/dashboard/AvailableDevices.json'
DEVELOPMENT = ROOT / '.ref/discovery/catalogs/inDevelopmentDevices.json'
DUAL_LINK = ROOT / '.ref/applications/synapse/dashboard/DualDongleCompatibleDevices.json'
# Per-product middleware DeviceInfo, statically extracted for every product by
# tools/audit-middleware-device-bindings.cjs. It is the source the original
# runtime itself reads for claimInterface/dongleId/bleId/category, so identity
# facts the dashboard catalog omits come from here for *all* products instead of
# being special-cased per product id.
MIDDLEWARE_BINDINGS = ROOT / 'docs/re/middleware-device-bindings-current.json'
FIELDS = {
    'productId': 'product_id', 'repId': 'rep_id', 'dongleId': 'dongle_id',
    'bleId': 'ble_id', 'xBoxId': 'xbox_id', 'wiredId': 'wired_id',
    'psModeIds': 'ps_mode_ids', 'monitorPortIds': 'monitor_port_ids',
    'isNotChromaDevice': 'is_not_chroma_device',
}
ARRAY_ONLY = {'psModeIds', 'monitorPortIds'}
SCALAR_ONLY = {'productId', 'repId'}


def relative(path):
    return path.relative_to(ROOT).as_posix()


def source(path):
    data = path.read_bytes()
    receipt = json.loads(path.with_name(path.name + '.http.json').read_text('utf-8'))
    digest = hashlib.sha256(data).hexdigest()
    url = 'https://apps.razer.com/synapse/dashboard/' + path.name
    assert receipt['source_url'] == receipt['final_url'] == url
    assert receipt['http_status'] == 200 and receipt['bytes'] == len(data)
    assert receipt['sha256'] == digest
    return json.loads(data), dict(path=relative(path), sha256=digest, acquisition=receipt)


def middleware_identity():
    """Per-product DeviceInfo facts, keyed by product id, from the AST audit."""
    data = MIDDLEWARE_BINDINGS.read_bytes()
    binding = json.loads(data)
    assert binding['scope_products'] >= 300, 'Re-run tools/audit-middleware-device-bindings.cjs'
    facts, unresolved = {}, 0
    for row in binding['products']:
        product_id = row.get('product_id')
        candidates = row.get('device_info_candidates') or []
        if len(candidates) != 1:
            unresolved += 1
            continue
        values = candidates[0].get('values') or {}
        if values.get('productId') != product_id:
            unresolved += 1
            continue
        entry = {}
        if type(values.get('dongleId')) is int:
            entry['dongle_id'] = values['dongleId']
        if type(values.get('bleId')) is int:
            entry['ble_id'] = values['bleId']
        if type(values.get('claimInterface')) is int:
            entry['claim_interface'] = values['claimInterface']
        if isinstance(values.get('category'), str):
            entry['category'] = values['category']
        if entry:
            entry['receipt'] = candidates[0]['receipt']
            facts[product_id] = entry
    receipt = dict(path=relative(MIDDLEWARE_BINDINGS), sha256=hashlib.sha256(data).hexdigest(),
                   method=binding['method'], scope_products=binding['scope_products'])
    return facts, receipt, unresolved

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    available, available_receipt = source(AVAILABLE)
    development, development_receipt = source(DEVELOPMENT)
    dual_link, dual_link_receipt = source(DUAL_LINK)
    middleware_facts, middleware_receipt, middleware_unresolved = middleware_identity()
    # Current host removes inDevelopment from available before classification.
    # This current catalog is empty; refuse silently changing those semantics.
    assert development == [], 'Re-audit current inDevelopment precedence before publishing'
    assert isinstance(available, list) and len(available) == 330
    rows = []
    usb_aliases, peer_aliases = defaultdict(list), defaultdict(list)
    for index, entry in enumerate(available):
        assert isinstance(entry, dict) and 'productId' in entry
        assert not (set(entry) - set(FIELDS)), f'Unreviewed identity field at row {index}'
        for key, value in entry.items():
            if key == 'isNotChromaDevice':
                assert type(value) is bool
                continue
            values = value if isinstance(value, list) else [value]
            assert all(type(pid) is int and 0 < pid <= 65535 for pid in values)
            assert key not in ARRAY_ONLY or isinstance(value, list)
            assert key not in SCALAR_ONLY or type(value) is int
        # Optional fields stay absent; scalar-versus-array and source order stay exact.
        row = {FIELDS[key]: value for key, value in entry.items()}
        # Middleware-level facts (claimInterface/dongleId/bleId/category) are kept
        # in a separate sub-object: the dashboard projection above stays lossless,
        # and the original's host HD versus middleware distinction stays visible.
        facts = middleware_facts.get(entry['productId'])
        if facts is not None:
            row['middleware'] = facts
        rows.append(row)
        aliases = set()
        for key in ('productId', 'repId', 'dongleId', 'bleId', 'xBoxId', 'wiredId', 'psModeIds'):
            value = entry.get(key)
            aliases.update(value if isinstance(value, list) else ([] if value is None else [value]))
        for raw in sorted(aliases):
            usb_aliases[raw].append(dict(catalog_index=index, product_id=entry['productId']))
        # he uses strict scalar equality; arrays are deliberately not flattened here.
        if type(entry.get('dongleId')) is int:
            peer_aliases[entry['dongleId']].append(dict(catalog_index=index, product_id=entry['productId']))
    output = ROOT / 'src/backend/discovery_catalog.json'
    data = (json.dumps(rows, ensure_ascii=False, indent=2) + '\n').encode('utf-8')
    receipts = dict(
        method='Lossless field projection of all current AvailableDevices rows; no runtime observations. Host HD aliases and middleware he strict scalar matches remain separate.',
        sources=[available_receipt, development_receipt], fields=FIELDS,
        output=relative(output), sha256=hashlib.sha256(data).hexdigest(), entries=len(rows),
        scalar_dongle_rows=sum(type(r.get('dongleId')) is int for r in available),
        array_dongle_rows=sum(isinstance(r.get('dongleId'), list) for r in available),
        usb_alias_count=len(usb_aliases), peer_alias_count=len(peer_aliases),
        usb_ambiguities={str(pid): rows for pid, rows in sorted(usb_aliases.items()) if len(rows) > 1},
        peer_ambiguities={str(pid): rows for pid, rows in sorted(peer_aliases.items()) if len(rows) > 1},
        middleware=middleware_receipt,
        middleware_rows=sum('middleware' in row for row in rows),
        middleware_unresolved_rows=middleware_unresolved,
        middleware_dongle_rows=sum('dongle_id' in row.get('middleware', {}) for row in rows),
        middleware_claim_interface_rows=sum('claim_interface' in row.get('middleware', {}) for row in rows),
        # Rows the dashboard catalog leaves without a dongleId but the product's own
        # middleware DeviceInfo declares one; identity is generated, never per-product.
        middleware_only_dongle_rows=sorted(
            row['product_id'] for row in rows
            if 'dongle_id' in row.get('middleware', {}) and type(row.get('dongle_id')) is not int),
    )
    evidence = ROOT / 'docs/re/discovery-catalog-current-evidence.json'
    evidence_data = (json.dumps(receipts, ensure_ascii=False, indent=2) + '\n').encode('utf-8')
    # Current middleware 34340/he chooses EID 0, falling back to ProductInfo[0],
    # for a descriptive name only when connectedDeviceInfo has no metadata.
    # Keep this catalog separate from actual edition/layout/serial observations.
    publishing = json.loads((ROOT / 'docs/re/receiver-publishing-review-current-evidence.json').read_text('utf-8'))
    naming = next(receipt for receipt in publishing['receipts'] if receipt.get('binding') == 'he')
    naming_source = (ROOT / naming['path']).read_bytes()
    assert hashlib.sha256(naming_source).hexdigest() == naming['sha256']
    assert naming_source.decode('utf-8').encode('utf-16-le')[naming['offset'] * 2:naming['end'] * 2].decode('utf-16-le') == naming['source']
    assert 'editionId:0' in naming['source'] and 't.ProductInfo.find((e=>e.EID===i.editionId))||t.ProductInfo[0]' in naming['source']
    peer_rows = []
    for entry in dual_link['Devices']:
        info = entry['ProductInfo']
        assert info and type(entry['DeviceDonglePid']) is int
        assert entry['DeviceType'] in {'MOUSE', 'KEYBOARD', 'ACCESSORY', 'MOUSEMAT'}
        selected = next((row for row in info if row['EID'] == 0), info[0])
        assert isinstance(selected['Name'], str) and isinstance(selected['CHSName'], str)
        peer_rows.append(dict(dongle_id=entry['DeviceDonglePid'], category=entry['DeviceType'],
                              product_name={'en': selected['Name'], 'zh-cn': selected['CHSName']}))
    assert len({row['dongle_id'] for row in peer_rows}) == len(peer_rows)
    peer_output = ROOT / 'src/backend/receiver_peer_catalog.json'
    peer_data = (json.dumps(peer_rows, ensure_ascii=False, indent=2) + '\n').encode('utf-8')
    peer_evidence = ROOT / 'docs/re/receiver-peer-names-current-evidence.json'
    peer_evidence_data = (json.dumps(dict(method='Static current catalog fallback names, never edition or connection observations.',
        source=dual_link_receipt, naming=naming, output=relative(peer_output),
        sha256=hashlib.sha256(peer_data).hexdigest(), entries=len(peer_rows)), ensure_ascii=False, indent=2) + '\n').encode('utf-8')
    if args.check:
        assert output.read_bytes() == data, 'Stale complete identity projection'
        assert evidence.read_bytes() == evidence_data, 'Stale identity projection receipt'
        assert peer_output.read_bytes() == peer_data, 'Stale receiver descriptive names'
        assert peer_evidence.read_bytes() == peer_evidence_data, 'Stale receiver naming evidence'
    else:
        output.write_bytes(data)
        evidence.write_bytes(evidence_data)
        peer_output.write_bytes(peer_data)
        peer_evidence.write_bytes(peer_evidence_data)
    print(f'Identity catalog: {len(rows)} complete rows, {len(usb_aliases)} USB aliases, {len(peer_aliases)} scalar peer aliases; no connected devices generated')
    print(f'Receiver fallback names: {len(peer_rows)} source descriptions; no edition/layout/serial generated')

if __name__ == '__main__':
    main()
