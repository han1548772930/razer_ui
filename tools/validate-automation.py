"""Static current-source, resource and category validation; no UI execution."""
import hashlib
import json
import re
import xml.etree.ElementTree as ET
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
load=lambda p:json.loads((ROOT/p).read_text(encoding='utf8'))
digest=lambda b:hashlib.sha256(b).hexdigest()
spec=load('src/features/automation_data.json')
audit=load('docs/re/automation-current-evidence.json')
assets=load('assets/synapse/automation-manifest.json')
registered={entry['output']:entry for entry in load('assets/synapse/manifest.json')['entries']}
embedded=(ROOT/'assets/synapse/embedded.rs').read_text(encoding='utf8')
assert audit['generator_sha256']==digest((ROOT/'tools/extract-automation.cjs').read_bytes())
assert spec['product_id']==3946 and spec['page']=='TAB_CUSTOMIZE'
assert [a['id'] for a in spec['actions']]==list(range(6))
assert len(spec['translations'])==10 and len(assets)==24
assert {e['id'] for e in spec['effects']}=={1,2,3,4,7,8,12}
for record in audit['source_files']+audit['css']+[audit['manifest']]:assert digest((ROOT/record['path']).read_bytes())==record['sha256']
source=(ROOT/audit['source_files'][0]['path']).read_text(encoding='utf8').encode('utf-16-le')
for record in audit['components']+[audit['actions']]:assert source[record['offset']*2:record['end']*2].decode('utf-16-le')==record['source']
color_audit=audit['quick_color_parameters']
assert {record['name'] for record in color_audit['components']}=={'_l','vl','Ql','AI','uP','yT'}
for record in color_audit['components']+color_audit['aliases']+color_audit['lane_mounts']+[color_audit['palette'],color_audit['mounted_by'],color_audit['number_input'],color_audit['color_picker_render'],color_audit['lane_connector']]:
    assert source[record['offset']*2:record['end']*2].decode('utf-16-le')==record['source']
assert 'Breathing_Effect:o=cl' in color_audit['mounted_by']['source']
assert 'Static_Effect:o=Hl' in color_audit['mounted_by']['source']
assert 'Starlight_Effect:o=NP' in color_audit['mounted_by']['source']
assert 'Wave_Effect:o=CP.isWaveWithSpeed?eI:Jl' in color_audit['mounted_by']['source']
assert 'Audio_Meter_Effect:o=dI' in color_audit['mounted_by']['source']
assert spec['metadata']['WAVE_DIRECTION']=='UpDown'
assert not spec['metadata'].get('isWaveWithSpeed')
assert not spec['metadata'].get('ble',{}).get('quickEffectUseHardware')
parameters={c['name']:c['source'] for c in color_audit['components']}
assert 'min:1,max:3,step:1' in parameters['uP'] and 'noTip:!0' in parameters['uP']
assert 'case"UpDown":t=3===this.props.effectSetting.direction?4:3' in parameters['Ql']
assert 'this.props.updateEffect(this.props.effectId,{colorBoost:parseFloat(e)},this.props.regionId)' in parameters['AI']
assert 'maxLength:4,maxValue:4,minValue:.25,allowDecimals:!0,roundUpDecimals:!0,stepValue:.25' in parameters['AI']
assert color_audit['number_input']['module']==44230
assert 'this.props.stepperRegex' not in color_audit['number_input']['source']
assert 'this.props.toggleTab()' in parameters['yT']
assert 'className:"color-drop-label"' in color_audit['color_picker_render']['source']
assert len(color_audit['lane_mounts'])==2
assert 'chromaPickUpData:' in color_audit['lane_mounts'][0]['source'] and 'chromaPutDownData:' not in color_audit['lane_mounts'][0]['source']
assert 'chromaPutDownData:' in color_audit['lane_mounts'][1]['source'] and 'chromaPickUpData:' not in color_audit['lane_mounts'][1]['source']
assert 'chromaPickUpData:' not in color_audit['lane_connector']['source'] and 'chromaPutDownData:' not in color_audit['lane_connector']['source']
for key in color_audit['parameter_labels'].values():
    assert key in spec['translations']['en']
palette=spec['quick_color_palette']
assert len(palette)==41 and palette[-1]=='no-color'
native_palette=(ROOT/'src/features/lighting_color.rs').read_text(encoding='utf8').split('const PRESETS: [u32; 40] = [',1)[1].split('];',1)[0]
assert palette[:-1]==['#'+color.lower() for color in re.findall(r'0x([a-fA-F0-9]{6})',native_palette)]
for asset in assets:
    content=(ROOT/asset['source']).read_bytes()
    output=(ROOT/asset['output']).read_bytes()
    assert digest(content)==asset['source_sha256'] and digest(output)==asset['output_sha256']
    entry=registered[asset['output']]
    assert entry['sha256']==asset['output_sha256'] and entry['source_sha256']==asset['source_sha256']
    assert f'"{asset["output"].removeprefix("assets/")}"' in embedded
    if 'inline_svg' in asset:
        fragment=content.decode('utf8').encode('utf-16-le')[asset['source_offset']*2:asset['source_end']*2].decode('utf-16-le')
        assert fragment==asset['source_fragment'] and output.decode('utf8')==asset['inline_svg']
    svg=ET.fromstring(output)
    assert svg.tag.endswith('svg') and not any(n.tag.endswith('script') for n in svg.iter())
for key in ['AUTOMATIONS','AUTOMATION_DESC','ADD_AUTOMATION','PICKING_UP_HEAD_SET','PUTTING_DOWN_HEAD_SET','ADD_GLOBAL_SHORTCUT_TO_START','ADD_GAME_TO_START','ADD_MACRO_TO_START']:
    assert key in spec['translations']['en']
print('Validated 3946 automation source, six categories, seven effects, mounted parameters, ten locales and 24 SVGs.')
