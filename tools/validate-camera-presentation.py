"""Statically validate camera source receipts, group wiring and prepared SVGs."""
import hashlib
import json
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(name):
    return json.loads((ROOT / name).read_text(encoding='utf-8'))


def sha(data):
    return hashlib.sha256(data).hexdigest()


def verify(receipt):
    source = (ROOT / receipt['path']).read_text(encoding='utf-8')
    # Acorn offsets are UTF-16 units, including in bundles with astral text.
    excerpt = source.encode('utf-16-le')[receipt['offset'] * 2:receipt['end'] * 2].decode('utf-16-le')
    assert excerpt == receipt['source'], receipt['path']
    assert sha(excerpt.encode()) == receipt['sha256'], receipt['path']


data = read('crates/razer-pages/src/features/source_controls_data.json')
evidence = read('docs/re/camera-presentation-current-evidence.json')
assert evidence['scanner_sha256'] == sha((ROOT / 'tools/camera-presentation.cjs').read_bytes())
totals = Counter()
for product in evidence['products']:
    pid = product['product_id']
    descriptor = next(p for p in data if p['product_id'] == pid)
    assert sha((ROOT / product['source']).read_bytes()) == product['sha256']
    assert sha((ROOT / product['css']['path']).read_bytes()) == product['css']['sha256']
    for name in ('wrapper', 'tooltip', 'processing_mount'):
        verify(product[name])
    receipts = {g['group']['key']: g for g in product['groups']}
    for page in descriptor['pages']:
        if page['key'] not in ('CAMERA', 'IMAGE', 'PROCESSING'):
            continue
        controls = {c['key']: c for s in page['sections'] for c in s['controls']}
        visited = []
        for group in page['camera_groups']:
            assert receipts[group['key']]['group'] == group, group['key']
            verify(receipts[group['key']]['jsx'])
            visited.extend(group['controls'])
            for field, kind in [('header_switch', 'switch'), ('header_stepper', 'slider'),
                                ('header_checkbox', 'toggle'), ('header_reset', 'reset')]:
                if field in group:
                    assert group[field] in group['controls']
                    assert controls[group[field]]['kind'] == kind
            for key in ('tooltip', 'warning', 'description'):
                if key in group:
                    for locale in ('en', 'zh-CN'):
                        assert group[key] in read(f'locales/{locale}.json'), group[key]
            totals['groups'] += 1
            totals['collapsible'] += group['collapsible']
            totals['tooltips'] += bool(group.get('tooltip'))
            totals['conditional_warnings'] += bool(group.get('warning'))
        assert len(visited) == len(set(visited)) and set(visited) == set(controls), (pid, page['key'])
    keys = {c['key'] for p in descriptor['pages'] for s in p['sections'] for c in s['controls']}
    if pid == 3592:
        assert '3592:processing-resolution' in keys
    if pid == 3594:
        assert not any(key in keys for key in ('3594:auto-quality', '3594:quality-mode', '3594:manual-quality'))
    if pid == 3595:
        assert '3595:auto-quality' in keys and '3595:hdr' in keys
        assert '3595:quality-mode' not in keys and '3595:manual-quality' not in keys
for asset in read('docs/re/camera-section-assets-current-evidence.json')['assets']:
    assert sha((ROOT / asset['prepared_path']).read_bytes()) == asset['sha256']
    for source in asset['sources']:
        assert sha((ROOT / source['path']).read_bytes()) == asset['sha256'] == source['sha256']
        assert sha((ROOT / source['manifest_path']).read_bytes()) == source['manifest_sha256']
        assert read(source['manifest_path'])['files'][source['manifest_key']].removeprefix('./') in source['path']
print(dict(totals))
