"""Prepare manifest-declared Studio SVG bytes; no vendor code execution."""
import concurrent.futures
import hashlib
import json
import sys
import xml.etree.ElementTree as ET
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parents[1]
check = sys.argv[1:] == ['--check']
if sys.argv[1:] not in ([], ['--check']):
    raise ValueError('Unknown argument')
evidence = json.loads((ROOT/'docs/re/chroma-studio-source.json').read_text(encoding='utf8'))
digest = lambda data: hashlib.sha256(data).hexdigest()
manifest_path = ROOT/evidence['manifest']['path']
assert digest(manifest_path.read_bytes()) == evidence['manifest']['sha256']
manifest = json.loads(manifest_path.read_text(encoding='utf8'))['files']

def write(path, data):
    if check:
        assert path.read_bytes() == data, f'Stale {path}'
    else:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)

def prepare(item):
    source = (ROOT/item['source']).resolve()
    assert source.is_relative_to(manifest_path.parent.resolve())
    assert manifest[item['manifest_key']].removeprefix('./') == source.relative_to(manifest_path.parent).as_posix()
    if not source.exists():
        assert not check, f'Missing {source}'
        request = Request(item['url'], headers={'Cache-Control': 'no-cache'})
        with urlopen(request, timeout=30) as response:
            data = response.read()
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_bytes(data)
            source.with_suffix(source.suffix+'.http.json').write_text(json.dumps({
                'url': response.url, 'status': response.status,
                'sha256': digest(data), 'fetched_at_utc': datetime.now(timezone.utc).isoformat(),
            }, indent=2)+'\n', encoding='utf8')
    data = source.read_bytes()
    svg = ET.fromstring(data)
    assert svg.tag.endswith('svg')
    out = ROOT/item['output']
    write(out, data)
    return {'id': item['name'], 'source': item['source'], 'source_sha256': digest(data),
            'output': item['output'], 'output_sha256': digest(data), 'method': 'Original SVG bytes',
            'viewBox': svg.attrib.get('viewBox'), 'width': svg.attrib.get('width'), 'height': svg.attrib.get('height')}

with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
    records = list(pool.map(prepare, evidence['assets']))
write(ROOT/'assets/synapse/chroma-studio-assets.json', (json.dumps(records, indent=2)+'\n').encode())
entries = '\n'.join(f'    ("{r["output"].removeprefix("assets/")}", include_bytes!("{Path(r["output"]).name}")),' for r in records)
write(ROOT/'assets/synapse/chroma-studio-embedded.rs', ('// Generated from current Studio manifest.\n&[\n'+entries+'\n]\n').encode())
print(f'Studio: {len(records)} original SVG assets validated.')
