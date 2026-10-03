"""Prepare statically extracted product 784 SVGs without evaluating vendor code."""
import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path
from urllib.request import Request, urlopen
from datetime import datetime, timezone
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
digest = lambda data: hashlib.sha256(data).hexdigest()
evidence = json.loads((ROOT / 'docs/re/aether-strip-current-evidence.json').read_text(encoding='utf-8'))
records = []
for asset in evidence['assets']:
    source, output = ROOT / asset['source'], ROOT / asset['output']
    assert source.resolve().is_relative_to((ROOT / '.ref/devices/784').resolve())
    assert output.resolve().is_relative_to((ROOT / 'assets/synapse').resolve())
    content = source.read_bytes()
    text = content.decode('utf-8')
    # Acorn offsets count UTF-16 code units, rather than Python code points.
    fragment = text.encode('utf-16-le')[asset['offset'] * 2:asset['end'] * 2].decode('utf-16-le')
    assert digest(fragment.encode('utf-8')) == asset['source_sha256']
    svg = ET.fromstring(asset['svg'])
    assert svg.tag.endswith('svg') and svg.get('viewBox')
    assert not any(el.tag.endswith('script') for el in svg.iter())
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(asset['svg'] + '\n', encoding='utf-8')
    records.append({'source':asset['source'], 'output':asset['output'],
                    'source_sha256':digest(content), 'output_sha256':digest(output.read_bytes()),
                    'fragment_sha256':asset['source_sha256'], 'offset':asset['offset'], 'end':asset['end'],
                    'method':'Static JSX SVG reconstruction; action path fill comes from current CSS'})
    if asset['name'] in ('identify', 'refresh'):
        hover = output.with_name(output.stem + '-hover.svg')
        hover.write_text(asset['svg'].replace('fill="#ccc"', 'fill="#44d62c"') + '\n', encoding='utf-8')
        records.append({**records[-1], 'output':hover.relative_to(ROOT).as_posix(),
                        'output_sha256':digest(hover.read_bytes()), 'method':'Current CSS hover fill applied to extracted SVG'})
for asset in evidence['media']:
    source, output = ROOT / asset['source'], ROOT / asset['output']
    assert source.resolve().is_relative_to((ROOT / '.ref/devices/784').resolve())
    assert output.resolve().is_relative_to((ROOT / 'assets/synapse').resolve())
    assert digest((ROOT / asset['context']).read_bytes()) == asset['context_sha256']
    receipt = source.with_name(source.name + '.http.json')
    if not source.exists():
        with urlopen(Request(asset['url'], headers={'User-Agent':'razer-ui-static-resource-audit'}), timeout=30) as response:
            content = response.read()
            assert response.status == 200
            metadata = {'source_url':asset['url'], 'final_url':response.url, 'http_status':response.status,
                        'content_type':response.headers.get('Content-Type'), 'sha256':digest(content),
                        'fetched_at_utc':datetime.now(timezone.utc).isoformat()}
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_bytes(content)
        receipt.write_text(json.dumps(metadata, indent=2) + '\n', encoding='utf-8')
    content = source.read_bytes()
    if receipt.exists():
        assert json.loads(receipt.read_text(encoding='utf-8'))['sha256'] == digest(content)
    if source.suffix == '.svg':
        svg = ET.fromstring(content)
        assert svg.tag.endswith('svg') and not any(el.tag.endswith('script') for el in svg.iter())
        output.write_bytes(content)
    else:
        assert source.suffix == '.avif'
        with Image.open(source) as artwork:
            artwork.convert('RGBA').save(output, optimize=True)
    records.append({**asset, 'source_sha256':digest(content), 'output_sha256':digest(output.read_bytes())})
(ROOT / 'assets/synapse/aether-strip-manifest.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')
print(f'Prepared {len(records)} current Aether Light Strip SVGs.')
