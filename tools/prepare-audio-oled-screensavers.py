"""Prove current 1383 screensaver GIFs equal the prepared WebP source GIFs."""
import concurrent.futures
import hashlib
import json
import sys
from pathlib import Path
from urllib.parse import urljoin, urlparse
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / '.ref/devices/1383'
CHECK = sys.argv[1:] == ['--check']
if sys.argv[1:] not in ([], ['--check']):
    raise ValueError('Only --check is supported')
sha = lambda data: hashlib.sha256(data).hexdigest()
manifest_path = BASE / 'asset-manifest.json'
manifest = json.loads(manifest_path.read_text(encoding='utf8'))
prepared = json.loads((ROOT / 'assets/synapse/manifest.json').read_text(encoding='utf8'))['entries']


def verify(index):
    output = f'assets/synapse/oled-screensaver-{index}.webp'
    entries = [entry for entry in prepared if entry['output'] == output]
    assert len(entries) == 1, output
    entry = entries[0]
    filename = Path(entry['source']).name
    declared = {value for value in manifest['files'].values() if value.endswith('/' + filename)}
    assert len(declared) == 1, filename
    resource = declared.pop()
    relative = resource[resource.index('static/'):]
    target = (BASE / relative).resolve()
    assert target.is_relative_to(BASE.resolve())
    url = urljoin('https://apps.razer.com/synapse/products/1383/ui/', resource)
    assert urlparse(url).hostname == 'apps.razer.com'
    if not target.exists():
        assert not CHECK, f'Missing current source: {relative}'
        with urlopen(url, timeout=30) as response:
            data = response.read()
        assert data.startswith((b'GIF87a', b'GIF89a')), url
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    data = target.read_bytes()
    assert data == (ROOT / entry['source']).read_bytes(), f'Different current GIF: {filename}'
    assert sha((ROOT / output).read_bytes()) == entry['sha256'], output
    return {'tile': index, 'source': target.relative_to(ROOT).as_posix(), 'source_url': url,
            'source_sha256': sha(data), 'shared_source': entry['source'],
            'shared_source_sha256': sha(data), 'output': output, 'output_sha256': entry['sha256'],
            'comparison': 'Current product GIF equals the prepared conversion source byte for byte; output is WebP.'}


with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:
    rows = list(pool.map(verify, [1, 2, 3]))
receipt = {'manifest': {'path': manifest_path.relative_to(ROOT).as_posix(),
                        'sha256': sha(manifest_path.read_bytes())}, 'screensavers': rows}
target = ROOT / 'docs/re/audio-oled-screensaver-assets.json'
serialized = json.dumps(receipt, indent=2) + '\n'
if CHECK:
    assert target.read_text(encoding='utf8') == serialized, 'Stale screensaver receipt'
else:
    target.write_text(serialized, encoding='utf8')
print('Validated 3 current 1383 GIFs against the prepared screensaver conversion sources.')
