"""Prepare declared wireless ARGB assets without running vendor code.

Run with the documented resource environment
(`.work/resource-env/Scripts/python.exe`) so the declared bitmaps can be
re-encoded; the derived SVG interaction layers need no encoder.
"""
import base64
import copy
import hashlib
import io
import json
import re
import xml.etree.ElementTree as ET
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import Request, urlopen
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
digest = lambda data: hashlib.sha256(data).hexdigest()
specs = json.loads((ROOT / 'src/features/wireless_argb_data.json').read_text(encoding='utf8'))
records = []
decoded = {}
for spec in specs:
    for asset in spec['assets']:
        output = ROOT / asset['output']
        assert output.resolve().is_relative_to((ROOT / 'assets/synapse').resolve())
        source_path = asset['bundle'] if 'data' in asset else asset['source']
        source = ROOT / source_path
        assert source.resolve().is_relative_to((ROOT / f'.ref/devices/{spec["product_id"]}').resolve())
        if 'data' in asset:
            content = base64.b64decode(asset['data'].split(',', 1)[1], validate=True)
        else:
            if not source.exists():
                with urlopen(Request(asset['url'], headers={'User-Agent': 'razer-ui-static-resource-audit'}), timeout=30) as response:
                    content = response.read()
                    assert response.status == 200
                    receipt = {'source_url': asset['url'], 'final_url': response.url, 'sha256': digest(content),
                               'http_status': response.status, 'fetched_at_utc': datetime.now(timezone.utc).isoformat()}
                source.parent.mkdir(parents=True, exist_ok=True)
                source.write_bytes(content)
                source.with_name(source.name + '.http.json').write_text(json.dumps(receipt, indent=2)+'\n', encoding='utf8')
            content = source.read_bytes()
        if output.suffix == '.svg':
            root = ET.fromstring(content)
            assert root.tag.endswith('svg') and not any(n.tag.endswith('script') for n in root.iter())
            output.write_bytes(content)
        else:
            with Image.open(io.BytesIO(content)) as bitmap:
                bitmap.convert('RGBA').save(output, optimize=True)
        record = {**{k:v for k,v in asset.items() if k != 'data'},
                  'source': source_path, 'source_sha256': digest(source.read_bytes()),
                  'output_sha256': digest(output.read_bytes())}
        if 'data' in asset:
            record['embedded_sha256'] = digest(content)
        records.append(record)
        if asset['output'].endswith(('-auto_off.svg', '-auto_on.svg', '-refresh.svg', '-remove.svg',
                                     '-warning.svg', '-power.svg', '-power_off.svg')):
            decoded[(spec['product_id'], output.stem.rsplit('-', 1)[-1])] = (content, source_path, digest(source.read_bytes()))

# The source recolors these icons from `:hover`/`:active` rules and animates the
# auto-detection glyph, both of which only exist in the embedded stylesheet.
# Split every interactive icon into the layer the source paints statically and
# the layer its interaction rules target, keeping the audited geometry verbatim
# and replacing only the recolored paint with `currentColor`; the native UI
# supplies the audited colors and keyframe timing recorded beside each output.
SVG_NS = 'http://www.w3.org/2000/svg'
def layer_root(source):
    # Keep the audited root attributes (viewport, viewBox and class list) so the
    # layer renders with the same user-coordinate scaling as the source icon.
    return ET.Element(source.tag, dict(source.attrib))
def sourced(target, name):
    return next(node for node in target.iter() if node.get('class') == name)
def layer_element(node, **attributes):
    element = copy.deepcopy(node)
    element.attrib.pop('class', None)
    element.attrib.update(attributes)
    return element
def write_layer(pid, name, root, receipt, **metadata):
    content, source_path, source_sha = receipt
    data = ET.tostring(root, encoding='utf-8')
    output = ROOT / f'assets/synapse/wireless-argb-{pid}-{name}.svg'
    output.write_bytes(data)
    records.append({'source': source_path, 'source_sha256': source_sha,
                    'output': output.relative_to(ROOT).as_posix(), 'output_sha256': digest(data), **metadata})
def keyframe(css, name):
    match = re.search(r'@keyframes\s+' + name + r'\s*\{(?:[^{}]|\{[^{}]*\})*\}', css)
    assert match, name
    return match.group(0)

for spec in specs:
    pid = spec['product_id']
    css_path = next((ROOT / f'.ref/devices/{pid}/static/css').glob('main.*.css'))
    css = css_path.read_text(encoding='utf-8')
    # Interaction receipts: the colors below are the audited declarations, and
    # the keyframe numbers are parsed from the same stylesheet.
    for declaration in [
        '.icon-detection:hover .detect-b{fill:#44d62c}',
        '.icon-detection:active .detect-b{fill:#39a029}',
        '.icon-detection--active:hover .detect-a{stroke:#96ef89}',
        '.icon-detection--active:active .detect-a{stroke:#39a029}',
        '.icon-detection--active .detect-a{stroke:#44d62c;stroke-width:10px}',
        '.icon-refreshing:hover path{fill:#44d62c}',
        '.icon-refreshing:active path{fill:#39a029}',
        '.icon-power:hover rect{fill:#7de36c}',
        '.icon-power--power-off:hover rect{fill:#d97077}',
        '.icon-warning:hover path{fill:#feab59}',
        '.icon-warning:active path{fill:#b15e0c}',
    ]:
        assert declaration in css, (pid, declaration)
    zoomout, zoomoutc, zoomoutf, zoomin = (keyframe(css, name) for name in ('zoomout', 'zoomoutc', 'zoomoutf', 'zoomin'))
    assert 'scale(.8)' in zoomout and 'fill:#ccc' in zoomout and 'scale(1)' in zoomout
    assert re.search(r'0%\{fill:#44d62c;[^}]*scale\(\.4\)', zoomoutc) and re.search(r'to\{fill:(?:#0000|transparent);[^}]*scale\(2\)', zoomoutc)
    assert re.search(r'0%\{fill:#666;[^}]*scale\(\.2\)', zoomoutf) and re.search(r'to\{fill:(?:#0000|transparent);[^}]*scale\(1\.4\)', zoomoutf)
    assert '0%{opacity:0' in zoomin and 'scale(1.8)' in zoomin and 'to{opacity:1' in zoomin and 'scale(1)' in zoomin
    off, on = decoded[(pid, 'auto_off')], decoded[(pid, 'auto_on')]
    off_svg, on_svg = ET.fromstring(off[0]), ET.fromstring(on[0])
    # Inactive icon: only `detect-b` is painted; `detect-c`/`detect-f` start the
    # `zoomoutc`/`zoomoutf` expansion and the two unclassed rects inherit the
    # root's transparent fill.
    root = layer_root(off_svg)
    root.append(layer_element(sourced(off_svg, 'detect-b'), fill='currentColor'))
    write_layer(pid, 'auto_off-glyph', root, off, keyframes=[zoomout], animation_ms=50, scale_from=0.8,
                scale_to=1.0, idle='#ccc', hover='#44d62c', pressed='#39a029')
    root = layer_root(off_svg)
    root.append(layer_element(sourced(off_svg, 'detect-c'), fill='#44d62c'))
    write_layer(pid, 'auto_off-green', root, off, keyframes=[zoomoutc], cycle_ms=700, scale_from=0.4, scale_to=2.0)
    root = layer_root(off_svg)
    root.append(layer_element(sourced(off_svg, 'detect-f'), fill='#666'))
    write_layer(pid, 'auto_off-gray', root, off, keyframes=[zoomoutf], cycle_ms=700, scale_from=0.2, scale_to=1.4)
    # Active icon: the glyph is fixed `#111`, and only the `detect-a` group is
    # stroked, so the hover/active rules recolor the ring alone.
    root = layer_root(on_svg)
    root.append(layer_element(sourced(on_svg, 'detect-b'), fill='#111'))
    write_layer(pid, 'auto_on-glyph', root, on, keyframes=[zoomin], animation_ms=100, scale_from=1.8,
                scale_to=1.0, opacity_from=0.0)
    root = layer_root(on_svg)
    root.append(layer_element(sourced(on_svg, 'detect-e'), fill='none', stroke='currentColor', **{'stroke-width': '10'}))
    write_layer(pid, 'auto_on-ring', root, on, idle='#44d62c', hover='#96ef89', pressed='#39a029')
    # Refresh/remove/warning paint exactly one path; the power button paints a
    # glyph on a rounded rect, and the rect alone carries the hover color.
    def painted(node):
        return next(n for n in node.iter() if n.tag.endswith('path') and n.get('fill') not in (None, 'none'))
    def filled_rect(node):
        return next(n for n in node.iter() if n.tag.endswith('rect'))
    for name, key, target, idle, hover, pressed in (
        ('refresh-hover', 'refresh', painted, '#ccc', '#44d62c', '#39a029'),
        ('remove-hover', 'remove', painted, '#ccc', '#c8323c', None),
        ('warning-hover', 'warning', painted, '#fd8611', '#feab59', '#b15e0c'),
    ):
        source = decoded[(pid, key)]
        source_svg = ET.fromstring(source[0])
        root = layer_root(source_svg)
        root.append(layer_element(target(source_svg), fill='currentColor'))
        write_layer(pid, name, root, source, idle=idle, hover=hover, pressed=pressed)
    for name, key, idle, hover in (('power-hover', 'power', '#44d62c', '#7de36c'),
                                   ('power_off-hover', 'power_off', '#c8323c', '#d97077')):
        source = decoded[(pid, key)]
        source_svg = ET.fromstring(source[0])
        root = layer_root(source_svg)
        root.append(layer_element(filled_rect(source_svg), fill='currentColor'))
        write_layer(pid, name, root, source, idle=idle, hover=hover)
    # The power glyph keeps the source's `.icon-power path{fill:#111}` rule, so
    # its base is the extracted SVG without the recolored rect.
    for name, key in (('power-base', 'power'), ('power_off-base', 'power_off')):
        content, source_path, source_sha = decoded[(pid, key)]
        root = ET.fromstring(content)
        rect = filled_rect(root)
        for parent in root.iter():
            if rect in list(parent):
                parent.remove(rect)
        write_layer(pid, name, root, (content, source_path, source_sha), glyph='#111')
(ROOT / 'assets/synapse/wireless-argb-manifest.json').write_text(json.dumps(records, indent=2)+'\n', encoding='utf8')
print(f'Prepared {len(records)} wireless ARGB assets.')
