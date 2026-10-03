"""Fetch manifest-declared OLED artwork, then losslessly prepare native animations.

Only decodes media; no downloaded JavaScript is executed.
"""
import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import urlopen
from PIL import Image, ImageSequence

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / '.ref/devices/691'
OUT = ROOT / 'assets/synapse'
manifest = json.loads((BASE / 'asset-manifest.json').read_text(encoding='utf-8'))['files']
page = BASE / 'static/js/OLED.b7b95581.chunk.js'
source = page.read_text(encoding='utf-8')
digest = lambda data: hashlib.sha256(data).hexdigest()
records = []
for number, fragment in enumerate(['1-random-sim-ths', '2-random-sim-wordmark', '3-random-sim-sneki'], 1):
    matches = [value for value in manifest.values() if fragment in value]
    assert len(matches) == 1, fragment
    url_path = matches[0]
    relative = url_path.removeprefix('/synapse/products/691/ui/')
    assert relative.startswith('static/media/') and relative in source
    original = BASE / relative
    if not original.exists():
        with urlopen('https://apps.razer.com' + url_path, timeout=60) as response:
            data = response.read()
            assert data.startswith((b'GIF87a', b'GIF89a')), relative
            original.parent.mkdir(parents=True, exist_ok=True)
            original.write_bytes(data)
            receipt = {'source_url':response.url, 'sha256':digest(data),
                       'fetched_at_utc':datetime.now(timezone.utc).isoformat()}
            original.with_suffix('.gif.http.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
    target = OUT / f'oled-screensaver-{number}.webp'
    with Image.open(original) as image:
        frames, durations = [], []
        for frame in ImageSequence.Iterator(image):
            frames.append(frame.convert('RGBA'))
            durations.append(frame.info['duration'])
        assert len(frames) > 1 and all(delay > 0 for delay in durations)
        frames[0].save(target, format='WEBP', save_all=True, append_images=frames[1:],
                       duration=durations, loop=image.info.get('loop',0), lossless=True, method=6)
    # Encoder may merge identical adjacent frames; retain its actual timings.
    with Image.open(target) as native:
        actual = []
        elapsed = 0
        original_index = 0
        original_end = durations[0]
        for frame in ImageSequence.Iterator(native):
            frame.load()
            while elapsed >= original_end:
                original_index += 1
                original_end += durations[original_index]
            assert frame.convert('RGBA').tobytes() == frames[original_index].tobytes()
            actual.append(frame.info['duration'])
            elapsed += actual[-1]
        assert sum(actual) == sum(durations)
        records.append({'source':original.relative_to(ROOT).as_posix(),
            'source_sha256':digest(original.read_bytes()),'output':target.relative_to(ROOT).as_posix(),
            'output_sha256':digest(target.read_bytes()),'width':native.width,'height':native.height,
            'loop':native.info.get('loop',0),'frames':len(actual),'frame_durations_ms':actual,
            'module_file':page.relative_to(ROOT).as_posix(),'module_sha256':digest(page.read_bytes()),
            'source_url':'https://apps.razer.com'+url_path})
(OUT / 'oled-assets-manifest.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8')
print('Prepared OLED animations:', [(r['frames'],r['width'],r['height']) for r in records])
