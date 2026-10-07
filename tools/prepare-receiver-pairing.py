"""Fetch/prepare only 179's manifest-declared pairing artwork; execute no vendor code."""
import hashlib
import json
from pathlib import Path
from urllib.request import Request, urlopen
from datetime import datetime, timezone
import xml.etree.ElementTree as ET
import sys
from copy import deepcopy

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / '.ref/devices/179'
manifest = json.loads((SOURCE / 'asset-manifest.json').read_text(encoding='utf-8'))
names = ['icon_category_mouse.svg', 'icon_category_keyboard.svg', 'icon_mouse_ensure.svg',
         'icon_kb_ensure.svg', 'icon-progress_spinner.svg',
         'icon_marching_ants_connecting_top_to_bottom.svg',
         'icon_marching_ants_not_connected_top_to_bottom.svg', 'icon_marching_ants_master_line.svg',
         'uma_pairing.d84abdc7.avif', 'Skeletonsvg.svg']
digest = lambda data: hashlib.sha256(data).hexdigest()
parent_only = '--parent-only' in sys.argv
target = ROOT / 'assets/synapse/receiver-pairing-manifest.json'
records = json.loads(target.read_text(encoding='utf-8')) if parent_only else []
if parent_only:
    names = ['Skeletonsvg.svg']
    records = [record for record in records if '/connection-skeleton' not in record['output']]
for name in names:
    source = SOURCE / manifest['files']['static/media/' + name].removeprefix('./')
    assert source.resolve().is_relative_to(SOURCE.resolve())
    url = 'https://apps.razer.com/synapse/products/179/ui/' + source.relative_to(SOURCE).as_posix()
    if not source.exists():
        with urlopen(Request(url, headers={'User-Agent': 'razer-ui-static-resource-audit'}), timeout=30) as response:
            content = response.read()
            assert response.status == 200
            metadata = dict(source_url=url, final_url=response.url, http_status=response.status,
                            content_type=response.headers.get('Content-Type'), sha256=digest(content),
                            fetched_at_utc=datetime.now(timezone.utc).isoformat())
        if source.suffix == '.svg':
            ET.fromstring(content)
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_bytes(content)
        source.with_name(source.name + '.http.json').write_text(json.dumps(metadata, indent=2) + '\n', encoding='utf-8')
    content = source.read_bytes()
    if source.suffix == '.svg':
        svg = ET.fromstring(content)
        assert not any(node.tag.endswith('script') for node in svg.iter())
        if name == 'Skeletonsvg.svg':
            output = ROOT / 'assets/synapse/receiver/connection-skeleton.svg'
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(content)
            ns = '{http://www.w3.org/2000/svg}'
            animation = svg.find('.//' + ns + 'animateTransform')
            assert animation is not None and animation.attrib == dict(
                id='ani', attributeName='transform', attributeType='XML', type='translate',
                **{'from': '-80 0', 'to': '80 0', 'begin': '0s;ani.end+0.5s',
                   'dur': '1.5s', 'fill': 'freeze'})
            assert svg.attrib['viewBox'] == '0 0 80 16'
            ET.register_namespace('', ns[1:-1])
            for layer, removed in [('base', 'shimmer'), ('sweep', 'text')]:
                prepared_svg = deepcopy(svg)
                prepared_svg.remove(next(node for node in prepared_svg if node.get('id') == removed))
                for parent in prepared_svg.iter():
                    for node in list(parent):
                        if node.tag == ns + 'animateTransform':
                            parent.remove(node)
                layer_output = output.with_name('connection-skeleton-' + layer + '.svg')
                layer_output.write_bytes(ET.tostring(prepared_svg, encoding='utf-8', xml_declaration=True))
                records.append(dict(source=source.relative_to(ROOT).as_posix(), source_url=url,
                                    source_sha256=digest(content), output=layer_output.relative_to(ROOT).as_posix(),
                                    sha256=digest(layer_output.read_bytes()),
                                    preparation='Source SVG ' + layer + ' layer, SMIL removed; native 1.5s translation and 0.5s hold'))
        else:
            output = ROOT / ('assets/synapse/dock-164-' + name)
            # Reuse only independently fetched byte-for-byte matching resources.
            assert output.read_bytes() == content, (source, output)
    else:
        from PIL import Image
        output = ROOT / 'assets/synapse/receiver/uma-pairing.png'
        output.parent.mkdir(parents=True, exist_ok=True)
        with Image.open(source) as image:
            image.convert('RGBA').save(output, optimize=True)
    records.append(dict(source=source.relative_to(ROOT).as_posix(), source_url=url,
                        source_sha256=digest(content), output=output.relative_to(ROOT).as_posix(),
                        sha256=digest(output.read_bytes()), preparation='Byte-identical existing SVG or AVIF to RGBA PNG; no resize'))
    if output.suffix == '.png':
        from PIL import Image
        with Image.open(output) as prepared:
            records[-1].update(width=prepared.width, height=prepared.height)
(ROOT / 'assets/synapse/receiver-pairing-manifest.json').write_text(json.dumps(records, indent=2) + '\n', encoding='utf-8')
print(f'Prepared current 179 skeleton and native animation layers; manifest contains {len(records)} resources.'
      if parent_only else f'Prepared {len(records)} current 179 pairing resources; 8 existing SVGs independently matched.')
