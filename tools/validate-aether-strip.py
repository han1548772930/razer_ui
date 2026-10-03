"""Static receipt, resource and source-boundary checks; no app or tests run."""
import hashlib
import json
import re
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
read = lambda path: (ROOT / path).read_text(encoding='utf-8')
digest = lambda data: hashlib.sha256(data).hexdigest()
spec = json.loads(read('src/features/aether_strip_data.json'))
evidence = json.loads(read('docs/re/aether-strip-current-evidence.json'))
assets = json.loads(read('assets/synapse/aether-strip-manifest.json'))
assert spec['product_id'] == 784 and spec['page'] == 'CUSTOMIZED'
assert spec['initial']['detectedLedCount'] == 0 and spec['initial']['bendData'] == []
assert spec['initial']['isRefreshing'] is False
assert len(spec['translations']) == 10
for locale, labels in spec['translations'].items():
    assert all(isinstance(labels[key], str) and labels[key] for key in spec['labels'].values()), locale
assert digest((ROOT / 'tools/extract-aether-strip.cjs').read_bytes()) == evidence['generator_sha256']
for source in evidence['source_files']:
    assert source['path'].startswith('.ref/devices/784/')
    assert digest((ROOT / source['path']).read_bytes()) == source['sha256'], source['path']
source_path = evidence['source_files'][0]['path']
source = read(source_path)
utf16 = source.encode('utf-16-le')
fragment = lambda start, end: utf16[start * 2:end * 2].decode('utf-16-le')
for contract in evidence['contracts']:
    assert contract in source, contract
for item in evidence['components'] + evidence['header']['fragments']:
    assert fragment(item['offset'], item['end']) == item['source'], item.get('symbol', item.get('method'))
for receipt in evidence['translationReceipts']:
    assert digest(fragment(receipt['offset'], receipt['end']).encode('utf-8')) == receipt['sha256']
assert evidence['header']['mounted_on_all_pages'] is True
assert evidence['header']['sync_disabled_pages'] == ['HELP', 'CUSTOMIZED']
assert evidence['header']['dropdown_disabled_pages'] == ['CUSTOMIZED']
assert len(assets) == 23
for asset in assets:
    source_file, output = ROOT / asset['source'], ROOT / asset['output']
    assert source_file.resolve().is_relative_to((ROOT / '.ref/devices/784').resolve())
    assert output.resolve().is_relative_to((ROOT / 'assets/synapse').resolve())
    assert digest(source_file.read_bytes()) == asset['source_sha256'], asset['source']
    assert digest(output.read_bytes()) == asset['output_sha256'], asset['output']
    if output.suffix == '.svg':
        svg = ET.fromstring(output.read_bytes())
        assert svg.tag.endswith('svg')
        assert not any(el.tag.endswith('script') or any(k.lower().startswith('on') for k in el.attrib) for el in svg.iter())
    if 'fragment_sha256' in asset:
        assert digest(fragment(asset['offset'], asset['end']).encode('utf-8')) == asset['fragment_sha256']
for svg in evidence['assets']:
    parsed = ET.fromstring(svg['svg'])
    assert parsed.get('viewBox'), svg['name']
native = read('src/features/aether_strip.rs')
state = read('src/features/aether_strip/state.rs')
preview = read('src/features/aether_strip/preview.rs')
snapshot = native[native.index('pub(crate) fn snapshot'):native.index('pub(crate) fn restore')]
assert 'bendData' in snapshot
assert not any(key in snapshot for key in ['detected', 'online', 'locked', 'power_on', 'synapse_override', 'last_request'])
assert 'Observation::default()' in native
assert 'SourceProductWorkspace::new' in preview and 'aether_preview_page' in preview
assert 'self.workspace.clone()' in preview
assert 'checked_add' in state and 'array.len() > 4' in state
assert 'stripeLedNumber' in state and 'pollTime' in state and 'sleepInterval' in state
assert 'SourceSpinner' in read('src/features/aether_strip/presentation.rs')
for path in [ROOT / 'src/features/aether_strip.rs', *sorted((ROOT / 'src/features/aether_strip').glob('*.rs'))]:
    assert not re.search(r'\b(?:rgb|rgba|hsla)\(', path.read_text(encoding='utf-8')), path
print('Aether Strip: current source receipts, 10 locales, 23 resources, mounted header policy and profile/observation separation validated statically.')
