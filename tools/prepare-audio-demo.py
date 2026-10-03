"""Prepare current demo poster data; never run an application or vendor code."""
import hashlib
import json
import xml.etree.ElementTree as ET
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import Request, urlopen

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
digest = lambda data: hashlib.sha256(data).hexdigest()
specs = json.loads((ROOT / 'src/features/audio_demo_data.json').read_text(encoding='utf-8'))
source_hashes = set()
for spec in specs:
    poster = spec['poster']
    module = ROOT / poster['module_file']
    assert digest(module.read_bytes()) == poster['module_sha256']
    manifest = ROOT / spec['manifest']['path']
    assert digest(manifest.read_bytes()) == spec['manifest']['sha256']
    source = ROOT / poster['source']
    assert source.resolve().is_relative_to((ROOT / '.ref/devices').resolve())
    receipt = source.with_name(source.name + '.http.json')
    if not source.exists():
        with urlopen(Request(poster['url'], headers={'User-Agent':'razer-ui-static-resource-audit'}), timeout=30) as response:
            data = response.read()
            metadata = {'source_url':poster['url'], 'final_url':response.url,
                        'http_status':response.status, 'content_type':response.headers.get('Content-Type'),
                        'fetched_at_utc':datetime.now(timezone.utc).isoformat(),
                        'sha256':digest(data), 'bytes':len(data)}
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_bytes(data)
        receipt.write_text(json.dumps(metadata, indent=2) + '\n', encoding='utf-8')
    data = source.read_bytes()
    assert json.loads(receipt.read_text(encoding='utf-8'))['sha256'] == digest(data)
    source_hashes.add(digest(data))
assert len(source_hashes) == 1, 'The three current demo posters differ'
output = ROOT / 'assets/synapse/audio-demo-poster.png'
with Image.open(ROOT / specs[0]['poster']['source']) as image:
    image.convert('RGBA').save(output, optimize=True)
    width, height = image.size
poster = specs[0]['poster']
record = {**poster, 'source_sha256':next(iter(source_hashes)),
          'output':output.relative_to(ROOT).as_posix(), 'output_sha256':digest(output.read_bytes()),
          'width':width, 'height':height,
          'equivalent_sources':[s['poster']['source'] for s in specs]}
evidence = json.loads((ROOT / 'docs/re/audio-demo-current-evidence.json').read_text(encoding='utf-8'))
product = next(p for p in evidence['products'] if p['product_id'] == 3942)
paths = [(component, jsx) for page in product['pages'] for component in page['components']
         for jsx in component['jsx'] if jsx['component'] == '"path"'
         and jsx['props'].get('d') == 'M0,16V0L12,8Z']
assert len(paths) == 1
component, jsx = paths[0]
svg = ET.Element('svg', {'xmlns':'http://www.w3.org/2000/svg', 'viewBox':'0 0 20 20'})
ET.SubElement(svg, 'path', {**jsx['props'], 'fill':'#fff'})
play = ROOT / 'assets/synapse/audio-demo-play.svg'
play.write_text(ET.tostring(svg, encoding='unicode') + '\n', encoding='utf-8')
records = [record, {'source':component['path'], 'source_sha256':digest((ROOT / component['path']).read_bytes()),
                   'output':play.relative_to(ROOT).as_posix(), 'output_sha256':digest(play.read_bytes()),
                   'component_offset':component['offset'], 'width':20, 'height':20}]
(ROOT / 'assets/synapse/audio-demo-manifest.json').write_text(json.dumps(records, indent=2)+'\n', encoding='utf-8')
print(f'Prepared current audio demo poster ({width} x {height}); all three source copies match.')
