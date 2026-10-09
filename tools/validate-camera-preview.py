"""Validate current mounted preview AST receipts, metadata and native UI wiring."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(name):
    return json.loads((ROOT / name).read_text(encoding='utf-8'))


def sha(data):
    return hashlib.sha256(data).hexdigest()


def verify(row):
    source = (ROOT / row['path']).read_text(encoding='utf-8')
    excerpt = source.encode('utf-16-le')[row['offset'] * 2:row['end'] * 2].decode('utf-16-le')
    assert excerpt == row['source'], row['path']
    if 'sha256' in row:
        assert sha(excerpt.encode()) == row['sha256'], row['path']


evidence = read('docs/re/camera-preview-current-evidence.json')
assert evidence['scanner_sha256'] == sha((ROOT / 'tools/camera-preview.cjs').read_bytes())
data = read('crates/razer-pages/src/features/source_controls_data.json')
for product in evidence['products']:
    pid = product['product_id']
    descriptor = next(row for row in data if row['product_id'] == pid)
    page = next(row for row in descriptor['pages'] if row['key'] == 'CAMERA')
    if pid == 3592:
        assert not product['mounted'] and 'camera_preview' not in page
        assert not any(g.get('presentation') for g in page['camera_groups'])
        continue
    assert product['mounted'] and pid in (3594, 3595, 3596)
    assert page['camera_preview'] == product['spec']
    assert product['spec']['activation'] == (pid != 3596)
    assert [g.get('presentation') for g in page['camera_groups'][:2]] == ['camera-promo', 'preview-source']
    assert page['camera_groups'][2]['title'] == 'PREVIEW_RESOLUTION_2'
    assert product['initial'] == {
        'isPreviewEnabled': None, 'isCamoStudioInstalled': None, 'cameraDeviceList': [],
        'selectedCameraDevice': 0, 'isThirdPartySelected': False, 'refreshCamera': False,
        'reconnectCamera': False, 'isHigherGenCableRequired': False,
    }
    for field in ('component', 'root', 'reducer', 'video', 'link'):
        verify(product[field])
    css = product['css']
    assert sha((ROOT / css['path']).read_bytes()) == css['sha256']
    css_source = (ROOT / css['path']).read_text(encoding='utf-8')
    for row in css['rules']:
        assert css_source[row['offset']:row['offset'] + len(row['source'])] == row['source']
    for locale in ('en', 'zh-CN'):
        translations = read(f'locales/{locale}.json')
        for key, value in product['spec'].items():
            if key == 'activation':
                continue
            for label in value if isinstance(value, list) else [value]:
                assert label in translations, (pid, locale, label)
for row in evidence['assets']:
    assert sha((ROOT / row['output']).read_bytes()) == row['sha256']
    assert len(row['sources']) == 3
    for source in row['sources']:
        verify(source)
native = (ROOT / 'crates/razer-pages/src/features/source_controls/camera_preview.rs').read_text(encoding='utf-8')
for token in ('enabled: None', 'SelectState::new(Vec::new()', 'SourceControlsHelpRequested',
              'SourceControlsPreviewRefreshRequested', 'https://www.razer.com/software/camo',
              'this.set_camera_preview_enabled', 'camera-preview-unable.svg'):
    assert token in native, token
print('Validated preview mounts for 3594/3595/3596, intentional 3592 absence, source initial state, labels, SVGs and UI wiring.')
