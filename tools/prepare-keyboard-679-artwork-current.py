"""Convert source image bytes and source literal geometry; never execute vendor code."""
import hashlib
import json
import re
import xml.etree.ElementTree as ET
from pathlib import Path
from PIL import Image
from fontTools.pens.basePen import BasePen
from fontTools.pens.boundsPen import BoundsPen
from fontTools.svgLib.path import parse_path

ROOT = Path(__file__).resolve().parents[1]


def sha(data):
    return hashlib.sha256(data).hexdigest()


class GeometryPen(BasePen):
    def __init__(self):
        super().__init__(None)
        self.commands = []
        self.bounds = BoundsPen(None)

    def _moveTo(self, point):
        self.commands.append({'op': 'move', 'points': list(point)})
        self.bounds.moveTo(point)

    def _lineTo(self, point):
        self.commands.append({'op': 'line', 'points': list(point)})
        self.bounds.lineTo(point)

    def _curveToOne(self, a, b, c):
        self.commands.append({'op': 'curve', 'points': [list(a), list(b), list(c)]})
        self.bounds.curveTo(a, b, c)

    def _closePath(self):
        self.commands.append({'op': 'close'})
        self.bounds.closePath()


def key_shape(key):
    if key.get('transform'):
        raise ValueError('New source transform requires independent decoding')
    if 'd' in key:
        pen = GeometryPen()
        parse_path(key['d'], pen)
        if not pen.bounds.bounds:
            raise ValueError('Empty source key path')
        x, y, right, bottom = pen.bounds.bounds
        geometry = {'kind': 'path', 'commands': pen.commands}
        bounds = [x, y, right-x, bottom-y]
    elif 'r' in key:
        x, y, r = [float(key[name]) for name in ('cx','cy','r')]
        geometry = {'kind': 'circle', 'center': [x, y], 'radius': r}
        bounds = [x-r, y-r, r*2, r*2]
    elif all(name in key for name in ('x', 'y', 'width', 'height')):
        if key.get('rx') or key.get('ry'):
            raise ValueError('Rounded source rectangle needs decoding')
        x, y, w, h = [float(key[name]) for name in ('x', 'y', 'width', 'height')]
        geometry = {'kind': 'rect', 'origin': [x,y], 'size': [w,h]}
        bounds = [x, y, w, h]
    else:
        return None  # Source dial sub-inputs are drawn in separate special popovers.
    return {'id': key['inputID'], 'source_button_key': key['buttonKey'],
            'label': str(key.get('counter', key.get('defaultValue', key['inputID']))),
            'enabled': key.get('isEnabled', True), 'functions': key.get('functionList', []),
            'bounds': bounds, 'geometry': geometry}


def shapes(groups):
    return [shape for group in groups for key in group['group']['buttonList']
            if (shape := key_shape(key)) is not None]


def main():
    raw = json.loads((ROOT/'.work/keyboard-679-artwork/raw.json').read_text(encoding='utf8'))
    out = ROOT/'assets/synapse/keyboard679-current'
    out.mkdir(parents=True, exist_ok=True)
    images = []
    assets = {}
    for image in raw['images']:
        source = ROOT/'local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui'/image['media']
        if not source.is_file():
            raise FileNotFoundError(source)
        source_hash = sha(source.read_bytes())
        asset = 'synapse/keyboard679-current/'+source.stem+'.png'
        output = ROOT/'assets'/asset
        with Image.open(source) as original:
            converted = original.convert('RGBA')
            converted.save(output, optimize=True)
            size = [converted.width, converted.height]
        assets[asset] = {'asset': asset, 'source': source.relative_to(ROOT).as_posix(),
                         'source_sha256': source_hash, 'output': output.relative_to(ROOT).as_posix(),
                         'output_sha256': sha(output.read_bytes()), 'pixel_size': size,
                         'conversion': 'Pillow RGBA lossless PNG, original dimensions, no crop or rescale'}
        images.append({**image, **assets[asset], 'asset_source':assets[asset]['source'],
                       'source':image['source']})
    # SM receives these original SVGs as props. Keep original XML, no rasterization.
    for receipt in raw['receipts']:
        if receipt['label'] not in ['inline-artwork-'+name for name in ('vM','GM','yM','gM','HM','fM','mM','PM','MM')]:
            continue
        media = re.search(r'static/media/[^"\\]+\.svg', receipt['source']).group(0)
        source = ROOT/'local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui'/media
        asset = 'synapse/keyboard679-current/'+source.name
        output = ROOT/'assets'/asset
        output.write_bytes(source.read_bytes())
        svg = ET.fromstring(source.read_bytes())
        viewbox = svg.attrib.get('viewBox', '').split()
        def svg_size(name, fallback):
            raw_size = svg.attrib.get(name, fallback)
            if not re.fullmatch(r'[\d.]+(?:px)?', str(raw_size)):
                raise ValueError('Unresolved source SVG dimension '+str(raw_size))
            return float(str(raw_size).removesuffix('px'))
        size = [svg_size('width', viewbox[2] if len(viewbox)==4 else ''),
                svg_size('height', viewbox[3] if len(viewbox)==4 else '')]
        assets[asset] = {'asset':asset, 'source':source.relative_to(ROOT).as_posix(),
                         'source_sha256':sha(source.read_bytes()), 'output':output.relative_to(ROOT).as_posix(),
                         'output_sha256':sha(output.read_bytes()), 'pixel_size':size,
                         'conversion':'Original SVG XML bytes, unchanged'}
    # Current CSS last background URL for .wrist-rest wins, regardless of edition.
    wrist_rule = next(row for row in raw['css'] if row['source'].startswith('.wrist-rest{'))
    wrist = next(image for image in images if image['role']=='wrist' and image['media'].endswith('wrist-without-gap.8e806975.avif'))
    native = {'product_id': raw['product_id'], 'source_file': raw['source_file'], 'source_sha256':raw['source_sha256'],
              'viewbox':[730,387], 'image_css_size':[730,340],
              'images': [{k:image[k] for k in ('role','edition_id','layout_id','resolution','asset','pixel_size')} for image in images],
              'default_keys':shapes(raw['default_groups']),
              'layouts':[{'layout_id':layout['layout_id'], 'keys':shapes(layout['groups'])} for layout in raw['layouts']],
              'wrist':{'asset':wrist['asset'],'bounds':[-0.5,255,731,154], 'transform_y':5}}
    (ROOT/'crates/razer-pages/src/features/keyboard_source_artwork_data.json').write_text(json.dumps(native,separators=(',',':'))+'\n',encoding='utf8')
    receipt = {key:raw[key] for key in ('product_id','source_file','source_sha256','receipts','css')}
    receipt['images']=images
    receipt['source_svg_assets'] = [value for value in assets.values() if value['output'].endswith('.svg')]
    receipt['geometry_preparation']='Original source path/circle/rect geometry; invisible sub-inputs stay absent from main image. Layout resolver default is source US group, independently captured.'
    (ROOT/'docs/re/keyboard-679-source-artwork-current.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf8')
    (ROOT/'assets/synapse/keyboard679-current-embedded.rs').write_text('&[\n'+''.join(f'    ("{asset}", include_bytes!("keyboard679-current/{Path(asset).name}") as &[u8]),\n' for asset in sorted(assets))+']\n',encoding='utf8')
    (ROOT/'assets/synapse/keyboard679-current-manifest.json').write_text(json.dumps(list(assets.values()),indent=2)+'\n',encoding='utf8')
    print(json.dumps({'prepared_images':len(images),'unique_assets':len(assets),'layouts':len(native['layouts']),'default_keys':len(native['default_keys']),'wrist_rule_utf16_range':wrist_rule['utf16_range']}))


if __name__=='__main__':
    main()
