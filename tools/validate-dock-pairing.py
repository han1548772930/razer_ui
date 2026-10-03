"""Validate current dock source receipts and resources without running the UI."""
import hashlib
import json
import re
from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
def read(path):
    return (ROOT / path).read_text(encoding='utf-8')
def digest(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()

specs = json.loads(read('src/features/dock_pairing_data.json'))
evidence = json.loads(read('docs/re/dock-pairing-current-evidence.json'))
assert evidence['generator_sha256'] == digest('tools/extract-dock-pairing.cjs')
assert [s['product_id'] for s in specs] == [164, 241]
for spec, product in zip(specs, evidence['products']):
    assert spec['product_id'] == product['product_id']
    assert spec['page'] == ('TAB_CUSTOMIZE' if spec['product_id'] == 164 else 'TAB_PAIRING')
    for receipt in [*product['source_files'], product['label_source'], product['manifest'],
                    *product['locale_receipts'], *product['css']]:
        assert receipt['path'].startswith(f'.ref/devices/{spec["product_id"]}/')
        assert digest(receipt['path']) == receipt['sha256'], receipt['path']
    for component in product['components']:
        # Acorn offsets count UTF-16 units, including astral characters in logs.
        source = read(component['path']).encode('utf-16-le')
        excerpt = source[component['offset'] * 2:component['end'] * 2].decode('utf-16-le')
        excerpt = re.sub(r'data:image/[^"\s]+', '[embedded bitmap omitted; retained in hashed source]', excerpt)
        assert excerpt == component['source'], (spec['product_id'], component['offset'])
    locales = spec['translations']
    assert len(locales) == 10
    for locale, labels in locales.items():
        assert set(labels) == set(locales['en']), (spec['product_id'], locale)
        assert all(isinstance(label, str) for label in labels.values())
    declared = {v.removeprefix('./') for v in json.loads(read(product['manifest']['path']))['files'].values()}
    for asset in spec['assets']:
        assert asset['source'].removeprefix(f'.ref/devices/{spec["product_id"]}/') in declared

assets = json.loads(read('assets/synapse/dock-pairing-manifest.json'))
assert {a['output'] for s in specs for a in s['assets']} == {a['output'] for a in assets}
assert len(assets) == len({a['output'] for a in assets}) == 27
for asset in assets:
    assert digest(asset['source']) == asset['source_sha256']
    assert digest(asset['output']) == asset['output_sha256']
    if asset['output'].endswith('.svg'):
        svg = ET.parse(ROOT / asset['output']).getroot()
        assert svg.tag.endswith('svg') and not any(n.tag.endswith('script') for n in svg.iter())
common = json.loads(read('locales/en.json'))
files = [ROOT / 'src/features/dock_pairing.rs', *sorted((ROOT / 'src/features/dock_pairing').glob('*.rs'))]
for file in files:
    for key in re.findall(r'\.text\("([A-Z_0-9]+)"\)', file.read_text(encoding='utf-8')):
        assert key in common or any(key in s['translations']['en'] for s in specs), (file.name, key)
print('Dock pairing: two current sources, 20 locale maps and 27 resources validated.')
