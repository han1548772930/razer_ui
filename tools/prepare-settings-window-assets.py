"""Prepare current /settings/ assets and verify shared social SVG equivalence."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import urllib.request
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument('--check', action='store_true')
args = parser.parse_args()
evidence = json.loads((ROOT / 'docs/re/settings-window-current-evidence.json').read_text(encoding='utf-8'))
manifest = json.loads((ROOT / '.ref/applications/settings/asset-manifest.json').read_text(encoding='utf-8'))

def sha(data):
    return hashlib.sha256(data).hexdigest()

def emit(path, data):
    path = ROOT / path
    if isinstance(data, str):
        data = data.encode('utf-8')
    if args.check:
        if path.read_bytes() != data:
            raise ValueError(f'Stale settings asset: {path}')
    else:
        path.write_bytes(data)

entries = []
for key, name in [('razer.svg', 'logo'), ('icon_arrow_left_thin.svg', 'back'),
                  ('icon_arrow_right_thin.svg', 'forward'), ('icon_refresh_white.svg', 'refresh')]:
    url_path = manifest['files']['static/media/' + key]
    source = ROOT / ('.ref/applications' + url_path)
    if not source.exists():
        if args.check:
            raise ValueError(f'Missing current SVG {source}')
        with urllib.request.urlopen('https://apps.razer.com' + url_path, timeout=30) as response:
            data = response.read()
        ET.fromstring(data)
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_bytes(data)
    data = source.read_bytes()
    ET.fromstring(data)
    output = 'assets/synapse/settings-window-' + name + '.svg'
    emit(output, data)
    entries.append({'source': source.relative_to(ROOT).as_posix(), 'source_sha256': sha(data),
                    'source_url': 'https://apps.razer.com' + url_path, 'output': output,
                    'sha256': sha(data), 'source_kind': 'downloaded_static_asset'})

def styles_for(sheet, name, hovered):
    result = {}
    for selector, declarations in re.findall(r'([^{}]+)\{([^}]*)\}', sheet):
        selector = re.sub(r'\s+', '', selector)
        if ':hover' in selector and not hovered:
            continue
        selector = selector.replace(':hover', '')
        if selector not in ('.' + name, '.ellipse~.social' if name == 'social' else ''):
            continue
        for key, value in re.findall(r'([\w-]+)\s*:\s*([^;]+)', declarations):
            if key not in ('transition', 'pointer-events'):
                result[key] = value.strip()
    return result

social = {item['name']: item for item in evidence['social']}
shared_css = '\n'.join(social[name]['styles'][0] for name in ('facebook', 'youtube'))
for category, hovered, expected in [
    ('ellipse', False, {'stroke': '#999999', 'stroke-width': '1', 'fill-opacity': '0'}),
    ('ellipse', True, {'stroke': '#44d62c', 'stroke-width': '1.5'}),
    ('social', False, {'fill': '#999999'}),
    ('social', True, {'fill': '#44d62c'}),
]:
    actual = styles_for(shared_css, category, hovered)
    if any(actual.get(key, '').lower() != value for key, value in expected.items()):
        raise ValueError(f'Native social transition palette changed: {category}/{hovered}')
insider_css = '\n'.join(social['insider']['styles'])
for category, hovered, expected in [
    ('container', False, {'stroke-width': '0', 'stroke': 'none'}),
    ('container', True, {'stroke-width': '1.5px', 'stroke': '#44d62c'}),
    ('grey', False, {'fill': '#8e8e8e'}),
    ('razergreen', False, {'fill': '#44d62c'}),
]:
    actual = styles_for(insider_css, category, hovered)
    if any(actual.get(key, '').lower() != value for key, value in expected.items()):
        raise ValueError(f'Native Insider transition palette changed: {category}/{hovered}')
if not all(re.search(r'transition\s*:\s*0\.2s\s*;', css) for css in (shared_css, insider_css)):
    raise ValueError('Native social transition timing changed')
shared = []
for name, item in social.items():
    css = '\n'.join(item['styles']) if name == 'insider' else shared_css
    for hovered in (False, True):
        size = (270, 50) if name == 'insider' else (28, 28)
        shapes = []
        for shape in item['shapes']:
            attrs = dict(shape['attrs'])
            attrs.update(styles_for(css, attrs.get('class', ''), hovered))
            shapes.append((shape['tag'], attrs))
        asset = f'assets/synapse/settings-social-{name}{"-hover" if hovered else ""}.svg'
        tree = ET.parse(ROOT / asset).getroot()
        expected = [(node.tag.split('}')[-1], node.attrib) for node in tree]
        if shapes != expected or tree.attrib['viewBox'] != f'0 0 {size[0]} {size[1]}':
            raise ValueError(f'Current /settings/ social SVG differs from shared bytes: {asset}')
        shared.append({'output': asset, 'sha256': sha((ROOT / asset).read_bytes()),
                       'source': item['source'], 'state': 'hover' if hovered else 'normal',
                       'transform': 'Verify all current literal shape attributes and inline shared CSS against existing SVG; transitions are rendered natively.'})
# Separate source path masks let native rendering interpolate fill/stroke and
# stroke width instead of crossfading two differently sized outline rasters.
for name, item in social.items():
    size = (270, 50) if name == 'insider' else (28, 28)
    outline = next(shape for shape in item['shapes'] if shape['tag'] in ('circle', 'rect'))
    if name == 'insider':
        required = {'x': '1', 'y': '1', 'width': '268', 'height': '48', 'rx': '4', 'ry': '4'}
    else:
        required = {'cx': '13', 'cy': '13', 'r': '13', 'transform': 'translate(1 1)'} if name == 'twitter' else {'cx': '14', 'cy': '14', 'r': '13'}
    if any(outline['attrs'].get(key) != value for key, value in required.items()):
        raise ValueError(f'Current social outline geometry changed: {name}')
    for category in (('razergreen', 'grey') if name == 'insider' else ('social',)):
        svg = ET.Element('svg', {'xmlns': 'http://www.w3.org/2000/svg',
                                'viewBox': f'0 0 {size[0]} {size[1]}'})
        paths = [shape for shape in item['shapes'] if shape['attrs'].get('class') == category]
        if not paths or any(shape['tag'] != 'path' for shape in paths):
            raise ValueError(f'Unexpected social mask geometry: {name}/{category}')
        for shape in paths:
            attrs = {key: value for key, value in shape['attrs'].items() if key not in ('class', 'id')}
            attrs['fill'] = '#ffffff'
            ET.SubElement(svg, 'path', attrs)
        data = ET.tostring(svg, encoding='utf-8', xml_declaration=True)
        output = f'assets/synapse/settings-window-social-{name}-{category}-mask.svg'
        emit(output, data)
        entries.append({'source': item['source']['path'], 'source_sha256': item['source']['sha256'],
                        'output': output, 'sha256': sha(data),
                        'source_kind': 'current_inline_svg_path_mask', 'class': category,
                        'transform': 'Preserve literal path geometry; split by source class; white alpha mask for native animated source fill.'})
emit('assets/synapse/settings-window-assets.json', json.dumps({'entries': entries, 'verified_shared': shared}, ensure_ascii=False, indent=2) + '\n')
embedded = '// Generated by tools/prepare-settings-window-assets.py\n&[\n'
for item in entries:
    name = Path(item['output']).name
    embedded += f'    ("synapse/{name}", include_bytes!("{name}")),\n'
embedded += ']\n'
emit('assets/synapse/settings-window-embedded.rs', embedded)
print(f'Settings assets {"checked" if args.check else "prepared"}: {len(entries)} SVGs, {len(shared)} shared states verified')
