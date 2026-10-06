"""Prepare only current 1383 OLED artwork. No vendor code is executed."""
import base64
import hashlib
import io
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import urlopen
from PIL import Image, ImageSequence

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / '.ref/devices/1383'
OUT = ROOT / 'assets'
digest = lambda data: hashlib.sha256(data).hexdigest()
check = sys.argv[1:] == ['--check']
if sys.argv[1:] not in ([], ['--check']):
    raise ValueError('Unknown argument')
receipt = json.loads((ROOT / 'docs/re/audio-oled-home-source.json').read_text(encoding='utf8'))
manifest = json.loads((BASE / 'asset-manifest.json').read_text(encoding='utf8'))['files']
declared = {v[v.index('static/'):]: v for v in manifest.values() if 'static/' in v}
records = []
def write_or_check(target, data):
    if check:
        assert target.read_bytes() == data, f'Stale {target}'
    else:
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
def verify_receipt(item):
    p = item['receipt']
    raw = (ROOT / p['path']).read_bytes()
    assert digest(raw) == p['sha256']
    text = raw.decode('utf8').encode('utf-16-le')
    assert text[p['offset'] * 2:p['end'] * 2].decode('utf-16-le') == p['source']

for item in receipt['assets']:
    verify_receipt(item)
    src = item['src']
    if src.startswith('data:image/'):
        content = base64.b64decode(src.split(',', 1)[1], validate=True)
        assert src in item['receipt']['source']
        source = item['receipt']['path']
    else:
        assert src in declared and src.startswith('static/media/')
        target = (BASE / src).resolve()
        assert target.is_relative_to(BASE.resolve())
        if not target.exists():
            assert not check, f'Missing source {src}'
            with urlopen('https://apps.razer.com' + declared[src], timeout=30) as response:
                content = response.read()
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(content)
                target.with_suffix(target.suffix + '.http.json').write_text(json.dumps({
                    'url': response.url, 'sha256': digest(content),
                    'fetched_at_utc': datetime.now(timezone.utc).isoformat()
                }, indent=2) + '\n', encoding='utf8')
        content = target.read_bytes()
        source = target.relative_to(ROOT).as_posix()
    with Image.open(io.BytesIO(content)) as image:
        frames, delays = [], []
        loop = image.info.get('loop', 0)
        for frame in ImageSequence.Iterator(image):
            rgba = frame.convert('RGBA')
            # CSS grayscale(100%) uses sRGB luminance; preserve source alpha.
            gray = rgba.convert('RGB').convert('L', (0.2126, 0.7152, 0.0722, 0)).convert('RGBA')
            gray.putalpha(rgba.getchannel('A'))
            frames.append(gray)
            delays.append(frame.info.get('duration', 0))
        encoded = io.BytesIO()
        if item['animated']:
            assert len(frames) > 1 and all(d > 0 for d in delays)
            frames[0].save(encoded, format='WEBP', save_all=True, append_images=frames[1:], duration=delays, loop=loop, lossless=True, method=6)
        else:
            assert len(frames) == 1
            frames[0].save(encoded, format='PNG')
        write_or_check(OUT / item['output'], encoded.getvalue())
        record = {'id': item['id'], 'source': source, 'source_sha256': item['receipt']['sha256'] if src.startswith('data:') else digest(content), 'output': 'assets/' + item['output'], 'output_sha256': digest(encoded.getvalue()), 'width': image.width, 'height': image.height, 'grayscale': True}
        if src.startswith('data:'):
            record['inline_sha256'] = digest(content)
        if item['animated']:
            elapsed, index, end, actual_delays = 0, 0, delays[0], []
            with Image.open(io.BytesIO(encoded.getvalue())) as native:
                for frame in ImageSequence.Iterator(native):
                    frame.load()
                    while elapsed >= end:
                        index += 1
                        end += delays[index]
                    assert frame.convert('RGBA').tobytes() == frames[index].tobytes()
                    delay = frame.info['duration']
                    actual_delays.append(delay)
                    elapsed += delay
                assert elapsed == sum(delays)
            record.update(source_frame_durations_ms=delays, frame_durations_ms=actual_delays, loop=loop)
        records.append(record)
for icon in receipt['icons']:
    verify_receipt(icon)
    encoded = icon['svg'].encode('utf8')
    write_or_check(OUT / icon['output'], encoded)
    records.append({'id': icon['id'], 'source': icon['receipt']['path'], 'source_sha256': icon['receipt']['sha256'], 'output': 'assets/' + icon['output'], 'output_sha256': digest(encoded), 'transform': 'Literal React SVG tree serialization'})
write_or_check(OUT / 'synapse/audio-oled-home-assets.json', (json.dumps(records, indent=2) + '\n').encode())
entries = '\n'.join(f'    ("{r["output"].removeprefix("assets/")}", include_bytes!("{Path(r["output"]).name}")),' for r in records)
write_or_check(OUT / 'synapse/audio-oled-home-embedded.rs', ('// Generated by tools/prepare-audio-oled-home-assets.py\n&[\n' + entries + '\n]\n').encode())
shared=[]
for item in receipt['shared']:
    src=item['src']
    assert src in declared
    target=(BASE / src).resolve()
    assert target.is_relative_to(BASE.resolve())
    if not target.exists():
        assert not check
        with urlopen('https://apps.razer.com' + declared[src], timeout=30) as response:
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(response.read())
    content=target.read_bytes()
    assert content==(ROOT / item['output']).read_bytes(), item['output']
    shared.append({'source':target.relative_to(ROOT).as_posix(),'output':item['output'],'sha256':digest(content),'method':'1383 manifest-declared resource byte-identical to shared native resource'})
write_or_check(ROOT / 'docs/re/audio-oled-home-shared-assets.json',(json.dumps(shared,indent=2)+'\n').encode())
print(f'1383 OLED: {len(records)} source assets, grayscale pixels and animation timing validated.')
