"""Validate ARGB source receipts and resources. Does not run the application."""
import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
load = lambda path: json.loads((ROOT / path).read_text(encoding='utf-8'))
digest = lambda path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest()
specs = load('src/features/wired_argb_data.json')
evidence = load('docs/re/wired-argb-current-evidence.json')
manifest = load('assets/synapse/wired-argb-manifest.json')
assert evidence['generator_sha256'] == digest('tools/extract-wired-argb.cjs')
assert [spec['product_id'] for spec in specs] == [778, 3871]
text_cache = {}

def source_text(path):
    if path not in text_cache:
        text_cache[path] = (ROOT / path).read_text(encoding='utf-8').encode('utf-16-le')
    return text_cache[path]

def validate_receipts(value):
    if isinstance(value, dict):
        if {'path', 'sha256'} <= value.keys():
            assert value['path'].startswith('.ref/devices/'), value['path']
            assert digest(value['path']) == value['sha256'], value['path']
        if {'path', 'offset', 'end', 'source'} <= value.keys():
            original = source_text(value['path'])[2 * value['offset']:2 * value['end']].decode('utf-16-le')
            assert original == value['source'], (value['path'], value['offset'])
        for child in value.values():
            validate_receipts(child)
    elif isinstance(value, list):
        for child in value:
            validate_receipts(child)

validate_receipts(evidence)
outputs = {record['output'] for record in manifest}
assert len(outputs) == len(manifest), 'Duplicate prepared outputs'
for spec, product in zip(specs, evidence['products']):
    assert spec['minimum_leds'] == 1
    assert spec['fan_counts'] == [6, 8, 9, 10, 12, 15, 16, 18, 20, 22, 24, 25, 32, 40]
    assert spec['page'] == 'TAB_CUSTOMIZE'
    assert len(spec['translations']) == 10
    assert {'en', 'zh-CN', 'zh-TW'} <= spec['translations'].keys()
    assert all('GLITTER_NO_OF_LED' in locale and 'TEXT_LED_STRIP' in locale for locale in spec['translations'].values())
    assert product['state_scope']['owner'] == 'device/localstorage'
    assert 'ports' not in product['state_scope']['default_profile']['source']
    assert product['profile_bar']['sync_icon_enabled_page'] == 'TAB_LIGHTING'
    consumers = '\n'.join(receipt['source'] for receipt in product['profile_bar']['consumers'])
    assert 'this.props.enableSwitchProfile' in consumers
    assert 'this.enableSwitchProfile()?"":"disabled"' in consumers
    assert 'loader disable' in consumers
    for asset in spec['assets']:
        assert asset['output'] in outputs
        assert digest(asset['context']) == asset['context_sha256']
    if spec['product_id'] == 778:
        assert spec['mainboard_ports'] == [2147483651, 2147483652, 2147483656]
        assert {'product-0', 'product-128', 'product-129'} <= {asset['name'] for asset in spec['assets']}

for record in manifest:
    assert (ROOT / record['source']).resolve().is_relative_to((ROOT / '.ref/devices').resolve())
    assert (ROOT / record['output']).resolve().is_relative_to((ROOT / 'assets/synapse').resolve())
    assert digest(record['source']) == record['source_sha256'], record['source']
    assert digest(record['output']) == record['output_sha256'], record['output']
    if record['output'].endswith('.svg'):
        svg = ET.fromstring((ROOT / record['output']).read_bytes())
        assert svg.tag.endswith('svg')
        assert not any(node.tag.endswith('script') for node in svg.iter())
        assert not any(key.lower().startswith('on') for node in svg.iter() for key in node.attrib)

for frame in range(120):
    assert f'assets/synapse/wired-argb-3871-detecting-ring-{frame:03}.svg' in outputs
for side in ('left', 'right'):
    for number in range(1, 10):
        assert f'assets/synapse/wired-argb-3871-detecting-led-{side}{number}.svg' in outputs
print(f'Validated 2 current ARGB pages, 20 locale dictionaries, source receipts and {len(manifest)} resources; application not executed.')
