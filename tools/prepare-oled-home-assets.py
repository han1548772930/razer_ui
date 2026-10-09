"""Prepare current OLED home presets without evaluating vendor JavaScript."""
import base64
import hashlib
import io
import json
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import urlopen

from PIL import Image, ImageSequence

ROOT = Path(__file__).resolve().parents[1]
BASE = ROOT / '.ref/devices/691'
OUT = ROOT / 'assets/synapse'
digest = lambda data: hashlib.sha256(data).hexdigest()


def webp_animation_metadata(content):
    """Read RIFF animation metadata directly, independently of Pillow."""
    assert content[:4] == b'RIFF' and content[8:12] == b'WEBP'
    assert int.from_bytes(content[4:8], 'little') + 8 == len(content)
    dimensions, loop, delays = None, None, []
    offset = 12
    while offset < len(content):
        tag = content[offset:offset + 4]
        length = int.from_bytes(content[offset + 4:offset + 8], 'little')
        start, end = offset + 8, offset + 8 + length
        assert end <= len(content)
        payload = content[start:end]
        if tag == b'VP8X':
            assert len(payload) == 10 and payload[0] & 2
            dimensions = (int.from_bytes(payload[4:7], 'little') + 1,
                          int.from_bytes(payload[7:10], 'little') + 1)
        elif tag == b'ANIM':
            assert len(payload) == 6
            loop = int.from_bytes(payload[4:6], 'little')
        elif tag == b'ANMF':
            assert dimensions and len(payload) > 16
            delay = int.from_bytes(payload[12:15], 'little')
            assert delay > 0
            delays.append(delay)
        offset = end + (length & 1)
    assert offset == len(content) and dimensions and loop is not None and delays
    return dimensions, loop, delays


data_path = ROOT / 'crates/razer-pages/src/features/keyboard_oled_editor_data.json'
data = json.loads(data_path.read_text(encoding='utf-8'))
module = ROOT / data['source']['path']
assert digest(module.read_bytes()) == data['source']['sha256']
manifest_path = BASE / 'asset-manifest.json'
manifest = json.loads(manifest_path.read_text(encoding='utf-8'))['files']
declared = {v.removeprefix('/synapse/products/691/ui/'): v for v in manifest.values()}
records = []
for kind in ('animations', 'images', 'visualizers'):
    for preset in data[kind]:
        src = preset['src']
        source = module
        if src.startswith('data:image/png;base64,'):
            content = base64.b64decode(src.split(',', 1)[1], validate=True)
            assert src in module.read_text(encoding='utf-8')
        else:
            assert src in declared and src.startswith('static/media/')
            source = (BASE / src).resolve()
            assert source.is_relative_to(BASE.resolve())
            if not source.exists():
                with urlopen('https://apps.razer.com' + declared[src], timeout=30) as response:
                    content = response.read()
                    source.parent.mkdir(parents=True, exist_ok=True)
                    source.write_bytes(content)
                    source.with_suffix(source.suffix + '.http.json').write_text(json.dumps({
                        'source_url': response.url, 'sha256': digest(content),
                        'fetched_at_utc': datetime.now(timezone.utc).isoformat()
                    }, indent=2) + '\n', encoding='utf-8')
            content = source.read_bytes()
        animated = kind in ('animations', 'visualizers')
        target = OUT / ('oled-home-' + preset['id'] + ('.webp' if animated else '.png'))
        # The current media visualizer GIFs are 3x rasters (696x132), while the
        # mounted CSS box is 232x44. Keep the original pixels and let native image
        # layout scale them, as the browser does; do not invent a resize filter.
        expected_size = (696, 132) if kind == 'visualizers' else (232, 64)
        with Image.open(io.BytesIO(content)) as image:
            source_loop = image.info.get('loop', 0)
            # The source HomeScreenDisplay container applies grayscale(100%).
            frames = []
            for frame in ImageSequence.Iterator(image):
                opaque = Image.alpha_composite(Image.new('RGBA', frame.size, (0, 0, 0, 255)),
                                               frame.convert('RGBA'))
                frames.append(opaque.convert('RGB').convert('L', (0.2126, 0.7152, 0.0722, 0)).convert('RGBA'))
            assert all(frame.size == expected_size for frame in frames), (preset['id'], frames[0].size)
            if animated:
                delays = [frame.info['duration'] for frame in ImageSequence.Iterator(image)]
                assert len(frames) > 1 and all(delay > 0 for delay in delays)
                frames[0].save(target, save_all=True, append_images=frames[1:], duration=delays,
                               loop=source_loop, lossless=True, method=6)
            else:
                assert len(frames) == 1
                frames[0].save(target)
        record = {'preset_id': preset['id'], 'source': source.relative_to(ROOT).as_posix(),
                  'source_sha256': digest(source.read_bytes()), 'output': target.relative_to(ROOT).as_posix(),
                  'output_sha256': digest(target.read_bytes()), 'width': expected_size[0], 'height': expected_size[1],
                  'module_file': module.relative_to(ROOT).as_posix(), 'module_sha256': data['source']['sha256'],
                  'descriptor_sha256': digest(data_path.read_bytes()), 'grayscale': True}
        if kind == 'visualizers':
            record.update(display_width=232, display_height=44)
        with Image.open(target) as native:
            if animated:
                actual, elapsed, source_ix, source_end = [], 0, 0, delays[0]
                for frame in ImageSequence.Iterator(native):
                    frame.load()
                    while elapsed >= source_end:
                        source_ix += 1
                        source_end += delays[source_ix]
                    assert frame.convert('RGBA').tobytes() == frames[source_ix].tobytes()
                    actual.append(frame.info['duration'])
                    elapsed += actual[-1]
                assert elapsed == sum(delays)
                dimensions, actual_loop, riff_delays = webp_animation_metadata(target.read_bytes())
                assert dimensions == expected_size and actual_loop == source_loop
                assert riff_delays == actual and sum(riff_delays) == sum(delays)
                record.update(loop=actual_loop, frames=len(riff_delays),
                              frame_durations_ms=riff_delays,
                              source_frames=len(delays), source_frame_durations_ms=delays)
            else:
                assert native.convert('RGBA').tobytes() == frames[0].tobytes()
        records.append(record)
(OUT / 'oled-home-assets-manifest.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')
print(f'Prepared and validated {len(records)} OLED home presets.')
