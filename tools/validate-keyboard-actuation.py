"""Check current actuation identities, source hashes and conversion metadata.

Static validation only. Does not execute the application, tests or vendor JS.
"""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
load = lambda path: json.loads((ROOT / path).read_text(encoding='utf-8'))
products = {p['product_id']: p for p in load('crates/razer-pages/src/features/keyboard_products_data.json')}
specs = load('crates/razer-pages/src/features/keyboard_actuation_data.json')
assert len({s['product_id'] for s in specs}) == len(specs)
assert {s['product_id'] for s in specs} == {pid for pid, p in products.items() if 'ACTUATION' in p['pages']}
for spec in specs:
    pid = spec['product_id']
    info = spec['info']
    assert info == products[pid]['config']['DeviceInfo']['analogSpecs']['actuationInfo']
    assert info['unit'] > 0 and info['min'] <= info['defaultValue'] <= info['max']
    assert len(spec['excluded']) == len(set(spec['excluded']))
    for path_key, hash_key in [('source', 'source_sha256'), ('page', 'page_sha256')]:
        path = spec[path_key]
        assert path.startswith(f'.ref/devices/{pid}/static/js/')
        assert hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == spec[hash_key]
    eligible = [k for k in products[pid]['keys'] if k['inputType'] == 'AnalogInput'
                and k.get('isEnabled', True) and k['inputID'] not in spec['excluded']]
    assert eligible, pid
print(f'Validated {len(specs)} current actuation descriptors, conversion bounds and source hashes.')
