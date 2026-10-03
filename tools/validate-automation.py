"""Static current-source, resource and category validation; no UI execution."""
import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
load=lambda p:json.loads((ROOT/p).read_text(encoding='utf8'))
digest=lambda b:hashlib.sha256(b).hexdigest()
spec=load('src/features/automation_data.json')
audit=load('docs/re/automation-current-evidence.json')
assets=load('assets/synapse/automation-manifest.json')
assert audit['generator_sha256']==digest((ROOT/'tools/extract-automation.cjs').read_bytes())
assert spec['product_id']==3946 and spec['page']=='TAB_CUSTOMIZE'
assert [a['id'] for a in spec['actions']]==list(range(6))
assert len(spec['translations'])==10 and len(assets)==12
assert {e['id'] for e in spec['effects']}=={1,2,3,4,7,8,12}
for record in audit['source_files']+audit['css']+[audit['manifest']]:assert digest((ROOT/record['path']).read_bytes())==record['sha256']
source=(ROOT/audit['source_files'][0]['path']).read_text(encoding='utf8').encode('utf-16-le')
for record in audit['components']+[audit['actions']]:assert source[record['offset']*2:record['end']*2].decode('utf-16-le')==record['source']
for asset in assets:
    content=(ROOT/asset['source']).read_bytes()
    output=(ROOT/asset['output']).read_bytes()
    assert digest(content)==asset['source_sha256'] and digest(output)==asset['output_sha256']
    svg=ET.fromstring(output)
    assert svg.tag.endswith('svg') and not any(n.tag.endswith('script') for n in svg.iter())
for key in ['AUTOMATIONS','AUTOMATION_DESC','ADD_AUTOMATION','PICKING_UP_HEAD_SET','PUTTING_DOWN_HEAD_SET','ADD_GLOBAL_SHORTCUT_TO_START','ADD_GAME_TO_START','ADD_MACRO_TO_START']:
    assert key in spec['translations']['en']
print('Validated 3946 automation source, six categories, seven effects, ten locales and twelve SVGs.')
