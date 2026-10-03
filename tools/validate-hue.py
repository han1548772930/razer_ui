"""Validate Hue source receipts, resources and literal contracts. No UI/test execution."""
import hashlib
import json
import re
from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
digest = lambda path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest()
spec = json.loads((ROOT / 'src/features/hue_data.json').read_text(encoding='utf-8'))
evidence = json.loads((ROOT / 'docs/re/hue-current-evidence.json').read_text(encoding='utf-8'))
assert evidence['generator_sha256'] == digest('tools/extract-hue.cjs')
for record in [evidence['source'], evidence['css'], evidence['manifest']]:
    assert digest(record['path']) == record['sha256'], record['path']
source = (ROOT / evidence['source']['path']).read_text(encoding='utf-8')
for receipt in [evidence['dictionaries'], evidence['archetypes'], evidence['palette'], evidence['brightness'],
                *evidence['defaults'], *evidence['constants'], *evidence['lighting'],
                *evidence['effect_components'], *evidence['common_labels']]:
    assert digest(receipt['path']) == receipt['sha256']
    assert source[receipt['offset']:receipt['end']] == receipt['source']
assert [e['id'] for e in spec['lighting']['effects']] == [11, 12, 2, 3, 1]
assert spec['initial']['brightness'] == {'isEnabled':False,'value':0}
assert spec['initial']['hue']['isPaired'] is False
assert len(spec['archetypes']) == len({a['name'] for a in spec['archetypes']}) == 48

common = json.loads((ROOT / 'locales/en.json').read_text(encoding='utf-8'))
custom = spec['translations']['en']
for path in [ROOT / 'src/features/hue.rs', *sorted((ROOT / 'src/features/hue').glob('*.rs'))]:
    if path.name == 'tests.rs':
        continue
    content = path.read_text(encoding='utf-8')
    keys = re.findall(r'(?:text|i18n::t)\("([A-Z_0-9]+)"\)',content)
    for key in keys:
        assert key in common or key in custom, (path.name,key)
for locale, labels in spec['translations'].items():
    assert all(isinstance(value,str) for value in labels.values()), locale
    assert set(labels) == set(custom), locale

manifest = json.loads((ROOT / 'assets/synapse/hue-manifest.json').read_text(encoding='utf-8'))
outputs = {entry['output'] for entry in manifest}
assert len(outputs) == len(manifest) == 62
for entry in manifest:
    assert digest(entry['source']) == entry['source_sha256']
    assert digest(entry['output']) == entry['output_sha256']
    if entry['output'].endswith('.svg'):
        svg = ET.parse(ROOT / entry['output']).getroot()
        assert svg.tag.endswith('svg')
        assert not any(node.tag.endswith('script') for node in svg.iter())
for path in [ROOT / 'src/features/hue.rs', *sorted((ROOT / 'src/features/hue').glob('*.rs'))]:
    for asset in re.findall(r'"(synapse/hue-[a-zA-Z0-9_-]+\.(?:svg|png))"',path.read_text(encoding='utf-8')):
        assert 'assets/'+asset in outputs, (path.name,asset)
print('Hue: current receipts, 10 locale maps, 5 quick effects and 62 resources validated.')
