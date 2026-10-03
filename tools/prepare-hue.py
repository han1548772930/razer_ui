"""Prepare declared Hue SVG resources and literal archetype paths, without execution."""
import hashlib
import json
import xml.etree.ElementTree as ET
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import Request, urlopen
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
digest = lambda data: hashlib.sha256(data).hexdigest()
spec = json.loads((ROOT / 'src/features/hue_data.json').read_text(encoding='utf-8'))
evidence = json.loads((ROOT / 'docs/re/hue-current-evidence.json').read_text(encoding='utf-8'))
records = []
for asset in spec['assets']:
    source, output = ROOT / asset['source'], ROOT / asset['output']
    assert source.resolve().is_relative_to((ROOT / '.ref/devices/769').resolve())
    assert output.resolve().is_relative_to((ROOT / 'assets/synapse').resolve())
    assert digest((ROOT / asset['context']).read_bytes()) == asset['context_sha256']
    receipt = source.with_name(source.name + '.http.json')
    if not source.exists():
        with urlopen(Request(asset['url'], headers={'User-Agent':'razer-ui-static-resource-audit'}), timeout=30) as response:
            content = response.read()
            assert response.status == 200
            metadata = {'source_url':asset['url'], 'final_url':response.url,
                        'http_status':response.status, 'content_type':response.headers.get('Content-Type'),
                        'fetched_at_utc':datetime.now(timezone.utc).isoformat(),
                        'sha256':digest(content), 'bytes':len(content)}
        if source.suffix == '.svg':
            ET.fromstring(content)
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_bytes(content)
        receipt.write_text(json.dumps(metadata, indent=2)+'\n', encoding='utf-8')
    content = source.read_bytes()
    # Existing resources are also validated as data; don't invent HTTP receipts.
    if receipt.exists():
        assert json.loads(receipt.read_text(encoding='utf-8'))['sha256'] == digest(content)
    if source.suffix == '.svg':
        svg = ET.fromstring(content)
        assert svg.tag.endswith('svg')
        assert not any(el.tag.endswith('script') for el in svg.iter())
        output.write_bytes(content)
    else:
        assert source.suffix == '.avif'
        with Image.open(source) as image:
            image.convert('RGBA').save(output, optimize=True)
    records.append({**asset, 'source_sha256':digest(content), 'output_sha256':digest(output.read_bytes())})

source = ROOT / evidence['archetypes']['path']
assert digest(source.read_bytes()) == evidence['archetypes']['sha256']
for archetype in spec['archetypes']:
    name = archetype['name']
    assert all(c.isalnum() or c in '-_' for c in name), name
    output = ROOT / f'assets/synapse/hue-light-{name}.svg'
    svg = ET.Element('svg', {'xmlns':'http://www.w3.org/2000/svg', 'viewBox':'0 0 16 28'})
    ET.SubElement(svg, 'path', {'fill':'#AFAFAF','d':archetype['d']})
    content = (ET.tostring(svg, encoding='unicode')+'\n').encode('utf-8')
    output.write_bytes(content)
    records.append({'source':source.relative_to(ROOT).as_posix(), 'source_sha256':digest(source.read_bytes()),
                    'output':output.relative_to(ROOT).as_posix(), 'output_sha256':digest(content),
                    'archetype':name, 'source_offset':evidence['archetypes']['offset']})
for icon in spec['bridge_icons']:
    output = ROOT / f'assets/synapse/hue-bridge-{icon["name"]}.svg'
    svg = ET.Element('svg', {'xmlns':'http://www.w3.org/2000/svg', 'viewBox':'0 0 20 20'})
    parent = svg
    for group in icon['groups']:
        parent = ET.SubElement(parent, 'g', group)
    for attributes in icon['paths']:
        attributes = dict(attributes)
        style = attributes.pop('style', {})
        if attributes.pop('className', None) == 'b':
            attributes['fill'] = '#707070'  # Inline .b definition in the same source component.
        attributes.update(style)
        ET.SubElement(parent, 'path', attributes)
    content = (ET.tostring(svg, encoding='unicode')+'\n').encode('utf-8')
    output.write_bytes(content)
    records.append({'source':source.relative_to(ROOT).as_posix(), 'source_sha256':digest(source.read_bytes()),
                    'output':output.relative_to(ROOT).as_posix(), 'output_sha256':digest(content),
                    'source_offset':icon['offset'], 'source_end':icon['end']})
(ROOT / 'assets/synapse/hue-manifest.json').write_text(json.dumps(records,indent=2)+'\n', encoding='utf-8')
print(f'Prepared {len(records)} current Hue resources.')
