"""Prepare audited ARGB raster/SVG resources without executing source code."""
import base64
import copy
import hashlib
import io
import json
import xml.etree.ElementTree as ET
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import Request, urlopen

from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
digest = lambda data: hashlib.sha256(data).hexdigest()
specs = json.loads((ROOT / 'src/features/wired_argb_data.json').read_text(encoding='utf-8'))
records = []
for spec in specs:
    for asset in spec['assets']:
        source, output = ROOT / asset['source'], ROOT / asset['output']
        assert source.resolve().is_relative_to((ROOT / f'.ref/devices/{spec["product_id"]}').resolve())
        assert output.resolve().is_relative_to((ROOT / 'assets/synapse').resolve())
        assert digest((ROOT / asset['context']).read_bytes()) == asset['context_sha256']
        receipt = source.with_name(source.name + '.http.json')
        if asset.get('inline_svg'):
            content = asset['inline_svg'].encode('utf-8')
        elif asset.get('embedded'):
            # Acorn offsets count UTF-16 code units. Embedded PNGs occur before
            # non-BMP strings, but slice explicitly in the same representation.
            text = source.read_text(encoding='utf-8').encode('utf-16-le')
            literal = text[asset['source_offset'] * 2:asset['source_end'] * 2].decode('utf-16-le')
            data_url = json.loads(literal)
            assert data_url.startswith('data:image/png;base64,')
            content = base64.b64decode(data_url.split(',', 1)[1], validate=True)
        else:
            if not source.exists():
                with urlopen(Request(asset['url'], headers={'User-Agent': 'razer-ui-static-resource-audit'}), timeout=30) as response:
                    content = response.read()
                    assert response.status == 200
                    metadata = {'source_url': asset['url'], 'final_url': response.url,
                                'http_status': response.status, 'content_type': response.headers.get('Content-Type'),
                                'fetched_at_utc': datetime.now(timezone.utc).isoformat(),
                                'sha256': digest(content), 'bytes': len(content)}
                source.parent.mkdir(parents=True, exist_ok=True)
                source.write_bytes(content)
                receipt.write_text(json.dumps(metadata, indent=2) + '\n', encoding='utf-8')
            content = source.read_bytes()
            if receipt.exists():
                assert json.loads(receipt.read_text(encoding='utf-8'))['sha256'] == digest(content)
        output.parent.mkdir(parents=True, exist_ok=True)
        if output.suffix == '.svg':
            svg = ET.fromstring(content)
            assert svg.tag.endswith('svg')
            assert not any(node.tag.endswith('script') for node in svg.iter())
            output.write_bytes(content)
        else:
            with Image.open(io.BytesIO(content)) as image:
                image.convert('RGBA').save(output, optimize=True)
        records.append({**asset, 'source_sha256': digest(source.read_bytes()), 'output_sha256': digest(output.read_bytes())})

# GPUI's image decoder does not run SMIL. Preserve the source's animated SVG
# as static layers, then animate opacity and the frame selection in native UI.
source = ROOT / '.ref/devices/3871/static/media/detecting.f2110e0d.svg'
svg = ET.fromstring(source.read_bytes())
tag = lambda node: node.tag.rsplit('}', 1)[-1]
targets = [node for node in svg.iter() if (node.get('id') or '').startswith('led-')]
assert len(targets) == 18
ring = next(node for node in svg.iter() if node.get('id') == 'ringimage')
ring_animation = next(node for node in svg.iter() if node.get('id') == 'animate_ring')
assert ring_animation.get('dur') == '2s' and ring_animation.get('to') == '360'
for side in ('left', 'right'):
    for index in range(1, 10):
        animation = next(node for node in svg.iter() if node.get('id') == f'animate_{side}{index}_1')
        assert animation.get('dur') == '0.5s'
        assert animation.get('begin').split(';')[0] == ('0s' if index == 1 else f'0.{index - 1}s')

def retain_layer(node, keep):
    if node.get('id') in keep or tag(node) in ('style', 'defs', 'clipPath', 'mask'):
        return True
    for child in list(node):
        if not retain_layer(child, keep):
            node.remove(child)
    return len(node) > 0

def remove_animation(node):
    for child in list(node):
        if tag(child).startswith('animate'):
            node.remove(child)
        else:
            remove_animation(child)

def write_derived(name, root, **metadata):
    output = ROOT / f'assets/synapse/wired-argb-3871-{name}.svg'
    data = ET.tostring(root, encoding='utf-8')
    output.write_bytes(data)
    records.append({'source': source.relative_to(ROOT).as_posix(), 'source_sha256': digest(source.read_bytes()),
                    'output': output.relative_to(ROOT).as_posix(), 'output_sha256': digest(data), **metadata})

base = copy.deepcopy(svg)
remove_animation(base)
for node in base.iter():
    for child in list(node):
        if child.get('id') == 'ringimage':
            node.remove(child)
write_derived('detecting-base', base)
for target in targets:
    layer = copy.deepcopy(svg)
    retain_layer(layer, {target.get('id')})
    remove_animation(layer)
    node = next(node for node in layer.iter() if node.get('id') == target.get('id'))
    node.set('style', 'opacity:1')
    write_derived('detecting-' + target.get('id'), layer, cycle_ms=1800, fade_ms=500,
                  delay_ms=(int(target.get('id')[-1]) - 1) * 100)

# Keep the bitmap and clip path unchanged; only apply the original transform
# origin and rotation. 120 frames cover one 2-second turn at native 60 Hz.
for frame in range(120):
    layer = copy.deepcopy(svg)
    retain_layer(layer, {'ringimage'})
    remove_animation(layer)
    layer.set('viewBox', '103 121 52 52')
    layer.set('width', '52')
    layer.set('height', '52')
    for node in layer.iter():
        if node.get('id') == 'ringimage':
            node.set('transform', f'rotate({frame * 3} 129 147)')
    write_derived(f'detecting-ring-{frame:03}', layer, cycle_ms=2000, frame=frame, frames=120)

# Source #multipleBrightness .dot-background and .custom-dim-corner. This is a
# data translation of the two 22px CSS gradients and their radial overlay.
for spec in specs:
    css = next((ROOT / f'.ref/devices/{spec["product_id"]}/static/css').glob('main.*.css'))
    text = css.read_text(encoding='utf-8')
    assert 'background-size:22px 22px' in text and 'background:radial-gradient(#0000,#222)' in text
    output = ROOT / f'assets/synapse/wired-argb-{spec["product_id"]}-dots.svg'
    content = b'<svg xmlns="http://www.w3.org/2000/svg" width="1210" height="400" viewBox="0 0 1210 400"><defs><pattern id="dots" width="22" height="22" x="594" y="189" patternUnits="userSpaceOnUse"><rect width="22" height="22" fill="#222"/><rect x="20" y="20" width="2" height="2" fill="#5d5d5d"/></pattern><radialGradient id="dim" r="70.710678%"><stop stop-color="#222" stop-opacity="0"/><stop offset="1" stop-color="#222"/></radialGradient></defs><rect width="1210" height="400" fill="url(#dots)"/><rect width="1210" height="400" fill="url(#dim)"/></svg>'
    output.write_bytes(content)
    records.append({'source': css.relative_to(ROOT).as_posix(), 'source_sha256': digest(css.read_bytes()),
                    'output': output.relative_to(ROOT).as_posix(), 'output_sha256': digest(content)})
(ROOT / 'assets/synapse/wired-argb-manifest.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')
print(f'Prepared {len(records)} wired ARGB resources.')
