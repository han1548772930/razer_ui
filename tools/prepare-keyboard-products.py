"""Compact data already extracted from current JS by the static Acorn reader."""
import json
import hashlib
import math
import re
import shutil
import xml.etree.ElementTree as ET
from pathlib import Path
from fontTools.pens.basePen import BasePen
from fontTools.pens.boundsPen import BoundsPen
from fontTools.pens.transformPen import TransformPen
from fontTools.misc.transform import Transform
from fontTools.svgLib.path import parse_path
from keyboard_geometry import geometry_for
from keyboard_product_controls import controls_for

ROOT = Path(__file__).resolve().parents[1]


class CanonicalPen(BasePen):
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

    def _curveToOne(self, p1, p2, p3):
        self.commands.append({'op': 'curve', 'points': [list(p1), list(p2), list(p3)]})
        self.bounds.curveTo(p1, p2, p3)

    def _closePath(self):
        self.commands.append({'op': 'close'})
        self.bounds.closePath()

    def _endPath(self):
        self.bounds.endPath()


def transform_value(text):
    transform = Transform()
    for name, arguments in re.findall(r'(\w+)\(([^)]+)\)', text):
        values = [float(x) for x in re.findall(r'[-+]?(?:\d*\.\d+|\d+\.?\d*)(?:[eE][-+]?\d+)?', arguments)]
        if name == 'matrix':
            transform = transform.transform(Transform(*values))
        elif name == 'translate':
            transform = transform.translate(values[0], values[1] if len(values) > 1 else 0)
        elif name == 'scale':
            transform = transform.scale(values[0], values[1] if len(values) > 1 else values[0])
        elif name == 'rotate':
            origin = values[1:] if len(values) == 3 else [0, 0]
            transform = transform.translate(*origin).rotate(math.radians(values[0])).translate(-origin[0], -origin[1])
        elif name == 'skewX':
            transform = transform.transform(Transform(1, 0, math.tan(math.radians(values[0])), 1, 0, 0))
        elif name == 'skewY':
            transform = transform.transform(Transform(1, math.tan(math.radians(values[0])), 0, 1, 0, 0))
        else:
            raise ValueError(f'Unaudited transform {name}')
    return transform


def draw_shape(key, pen):
    if 'd' in key:
        parse_path(key['d'], pen)
    elif 'r' in key:
        x,y,r = float(key['cx']),float(key['cy']),float(key['r'])
        parse_path(f'M{x+r} {y} A{r} {r} 0 1 0 {x-r} {y} A{r} {r} 0 1 0 {x+r} {y} Z',pen)
    elif all(name in key for name in ('x','y','width','height')):
        x,y,w,h = [float(key[name]) for name in ('x','y','width','height')]
        rx,ry = float(key.get('rx',key.get('ry',0))),float(key.get('ry',key.get('rx',0)))
        rx,ry = min(rx,w/2),min(ry,h/2)
        if rx and ry:
            parse_path(f'M{x+rx} {y} H{x+w-rx} A{rx} {ry} 0 0 1 {x+w} {y+ry} V{y+h-ry} A{rx} {ry} 0 0 1 {x+w-rx} {y+h} H{x+rx} A{rx} {ry} 0 0 1 {x} {y+h-ry} V{y+ry} A{rx} {ry} 0 0 1 {x+rx} {y} Z',pen)
        else:
            parse_path(f'M{x} {y} H{x+w} V{y+h} H{x} Z',pen)
    else:
        raise ValueError('No geometric shape')


def shape_for(key, xml_shape=None):
    pen = CanonicalPen()
    if xml_shape is None:
        if not any(field in key for field in ['d','r','width']):
            return None
        draw_shape(key, TransformPen(pen, transform_value(key.get('transform',''))))
    else:
        node, transform = xml_shape
        draw_shape(node.attrib, TransformPen(pen,transform))
    bounds = pen.bounds.bounds
    if not bounds:
        return None
    x,y,right,bottom = bounds
    return dict(id=key['inputID'],label=str(key.get('counter',key.get('defaultValue',key['inputID']))),
                enabled=key.get('isEnabled',True),functions=key.get('functionList',[]),
                bounds=[x,y,right-x,bottom-y],geometry={'kind':'path','commands':pen.commands})


def svg_nodes(node, transform=Transform()):
    transform = transform.transform(transform_value(node.get('transform','')))
    if node.get('id'):
        yield node.get('id'), (node,transform)
    for child in node:
        yield from svg_nodes(child,transform)


def main():
    products = []
    artwork = {a['product_id']:a for a in json.loads((ROOT/'docs/re/keyboard-product-artwork.json').read_text(encoding='utf8'))}
    svg_ids = json.loads((ROOT/'assets/synapse/keyboard-svg-ids.json').read_text(encoding='utf8'))
    audit = []
    pages = json.loads((ROOT/'docs/re/keyboard-product-pages.json').read_text(encoding='utf8'))
    for file in sorted((ROOT / 'assets/synapse/keyboard-products').glob('*.json'), key=lambda p: int(p.stem)):
        raw = json.loads(file.read_text(encoding='utf8'))
        config = raw['config']
        keys = [key for group in raw['default_groups']['groups'] for key in group['group']['buttonList']]
        fields = ('inputID', 'inputType', 'analogInputID', 'counter', 'defaultValue', 'functionList', 'isEnabled', 'HID', 'pageID', 'assignment', 'assignmentValue')
        pid = raw['product_id']
        asset = artwork[pid]
        svg = {}
        image = None
        if asset.get('svg'):
            root = ET.parse(ROOT/asset['source']).getroot()
            assert all(n.tag.split('}')[-1] not in ('script','foreignObject') for n in root.iter())
            svg = dict(svg_nodes(root))
            viewbox = [float(x) for x in root.attrib['viewBox'].split()][2:]
            output = ROOT/f'assets/synapse/keyboard-product-{pid}.svg'
            shutil.copyfile(ROOT/asset['source'],output)
            image = 'synapse/'+output.name
            asset.update(output=output.relative_to(ROOT).as_posix(),output_sha256=hashlib.sha256(output.read_bytes()).hexdigest())
        else:
            viewbox = [350,450] if config['DeviceInfo'].get('category') == 'KEYPAD' else [730,387]
            if pid == 691:
                viewbox = [730,340] # 6375.a5fed9ed.chunk.js, explicit MapKeyboard minHeight:340.
            if asset.get('output'):
                image = 'synapse/'+Path(asset['output']).name
        shapes,missing = [],[]
        for key in keys:
            key_id = key['inputID']
            xml_shape = svg.get(svg_ids.get(str(pid),{}).get('ids',{}).get(key_id,''))
            shape = shape_for(key,xml_shape)
            if shape:
                shapes.append(shape)
            else:
                missing.append(key_id)
        products.append({
            'product_id': raw['product_id'], 'name': raw['name'], 'config': config,
            'pages': sorted({item['name']['value'] for nav in raw['navigation'] for item in nav['items']}),
            'keys': [{field: key[field] for field in fields if field in key} for key in keys],
            'shapes': shapes, 'viewbox': viewbox, 'image': image,
            'image_size': [asset.get('width',viewbox[0]*3)/3,asset.get('height',viewbox[1]*3)/3],
            'source': raw['source_config'], 'source_files': raw['source_files'],
            'controls': controls_for(raw, pages, ROOT),
        })
        audit.append({'product_id':pid,'keys':len(keys),'shapes':len(shapes),'non_geometric_inputs':missing,'viewbox':viewbox,'source_geometry':raw['default_groups']['source'],'svg_source':asset.get('source')if svg else None})
    output = ROOT / 'crates/razer-pages/src/features/keyboard_products_data.json'
    output.write_text(json.dumps(products, ensure_ascii=False, separators=(',', ':')), encoding='utf8')
    (ROOT/'docs/re/keyboard-product-geometry.json').write_text(json.dumps(audit,indent=2),encoding='utf8')
    (ROOT/'assets/synapse/keyboard-product-manifest.json').write_text(json.dumps(list(artwork.values()),indent=2),encoding='utf8')
    print(f'{len(products)} current keyboard/keypad products -> {output.relative_to(ROOT)}')


if __name__ == '__main__':
    main()
