"""Validate source/data/artwork receipts statically; no app or vendor code runs."""
import base64
import hashlib
import json
import re
import struct
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
load = lambda p: json.loads((ROOT / p).read_text(encoding='utf8'))
digest = lambda b: hashlib.sha256(b).hexdigest()
specs = load('src/features/wireless_argb_data.json')
audit = load('docs/re/wireless-argb-current-evidence.json')
assets = load('assets/synapse/wireless-argb-manifest.json')
assert audit['generator_sha256'] == digest((ROOT / 'tools/extract-wireless-argb.cjs').read_bytes())
assert [s['product_id'] for s in specs] == [3884, 3886]
assert len(assets) == 30
for spec, evidence in zip(specs, audit['products']):
    pid = spec['product_id']
    assert spec['page'] == 'TAB_CUSTOMIZE'
    assert len(spec['translations']) == 10
    assert spec['fan_values'] == [6, 8, 9, 10, 12, 15, 16, 18, 20, 22, 24, 25, 32, 40]
    for receipt in evidence['source_files'] + evidence['css'] + [evidence['manifest']]:
        assert digest((ROOT / receipt['path']).read_bytes()) == receipt['sha256'], receipt['path']
    bundle = (ROOT / evidence['source_files'][0]['path']).read_text(encoding='utf8')
    # Acorn offsets count UTF-16 units, so slice with the same coordinate system.
    utf16 = bundle.encode('utf-16-le')
    for component in evidence['components']:
        assert utf16[component['offset']*2:component['end']*2].decode('utf-16-le') == component['source']
    for key in spec['labels'].values():
        assert key in spec['translations']['en'], (pid, key)
    for asset in spec['assets']:
        receipt = next(r for r in assets if r['output'] == asset['output'])
        source_path = asset['bundle'] if 'data' in asset else asset['source']
        assert receipt['source'] == source_path
        assert (ROOT / source_path).resolve().is_relative_to((ROOT / f'.ref/devices/{pid}').resolve())
        assert digest((ROOT / source_path).read_bytes()) == receipt['source_sha256']
        content = base64.b64decode(asset['data'].split(',', 1)[1], validate=True) if 'data' in asset else (ROOT / asset['source']).read_bytes()
        if 'data' in asset:
            assert receipt['bundle'] == asset['bundle']
            assert digest(content) == receipt['embedded_sha256']
        else:
            assert digest(content) == receipt['source_sha256']
        output = (ROOT / asset['output']).read_bytes()
        assert digest(output) == receipt['output_sha256']
        if asset['output'].endswith('.svg'):
            assert output == content
            svg = ET.fromstring(output)
            assert svg.tag.endswith('svg') and not any(n.tag.endswith('script') for n in svg.iter())
            assert not re.search(r'(?:href|src)\s*=\s*[\"\'](?:https?:|javascript:)', output.decode())
        else:
            assert output.startswith(b'\x89PNG\r\n\x1a\n')
            width, height = struct.unpack('>II', output[16:24])
            assert width > 0 and height > 0
    if pid == 3884:
        assert spec['metadata']['minimumLedValue'] == 1
        assert 'return E>240' in bundle
    else:
        assert spec['metadata']['allowedLedsPerPort'] == 120
        assert '!r&&i&&!u.type===k_.BLE_MOBIL&&this.renderPorts(a)' in bundle
        assert 'Math.max(n,40)' in bundle
print('Validated both wireless ARGB products, 10 locales each, source offsets and 30 artwork receipts.')
