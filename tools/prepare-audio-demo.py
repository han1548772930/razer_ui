"""Prepare current demo poster data; never run an application or vendor code."""
import hashlib
import base64
import io
import json
import re
import sys
import xml.etree.ElementTree as ET
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import Request, urlopen

from PIL import Image
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen

ROOT = Path(__file__).resolve().parents[1]
CHECK = sys.argv[1:] == ['--check']
if sys.argv[1:] not in ([], ['--check']):
    raise SystemExit('Usage: prepare-audio-demo.py [--check]')
def emit(path, data):
    if CHECK:
        assert path.read_bytes() == data, f'Stale prepared audio demo resource: {path}'
    else:
        path.write_bytes(data)

digest = lambda data: hashlib.sha256(data).hexdigest()
specs = json.loads((ROOT / 'crates/razer-pages/src/features/audio_demo_data.json').read_text(encoding='utf-8'))
source_hashes = set()
for spec in specs:
    poster = spec['poster']
    module = ROOT / poster['module_file']
    assert digest(module.read_bytes()) == poster['module_sha256']
    manifest = ROOT / spec['manifest']['path']
    assert digest(manifest.read_bytes()) == spec['manifest']['sha256']
    source = ROOT / poster['source']
    assert source.resolve().is_relative_to((ROOT / '.ref/devices').resolve())
    receipt = source.with_name(source.name + '.http.json')
    if not source.exists():
        assert not CHECK, f'Missing current source: {source}'
        with urlopen(Request(poster['url'], headers={'User-Agent':'razer-ui-static-resource-audit'}), timeout=30) as response:
            data = response.read()
            metadata = {'source_url':poster['url'], 'final_url':response.url,
                        'http_status':response.status, 'content_type':response.headers.get('Content-Type'),
                        'fetched_at_utc':datetime.now(timezone.utc).isoformat(),
                        'sha256':digest(data), 'bytes':len(data)}
        source.parent.mkdir(parents=True, exist_ok=True)
        source.write_bytes(data)
        receipt.write_text(json.dumps(metadata, indent=2) + '\n', encoding='utf-8')
    data = source.read_bytes()
    assert json.loads(receipt.read_text(encoding='utf-8'))['sha256'] == digest(data)
    source_hashes.add(digest(data))
assert len(source_hashes) == 1, 'The three current demo posters differ'
output = ROOT / 'assets/synapse/audio-demo-poster.png'
with Image.open(ROOT / specs[0]['poster']['source']) as image:
    prepared = io.BytesIO()
    image.convert('RGBA').save(prepared, format='PNG', optimize=True)
    emit(output, prepared.getvalue())
    width, height = image.size
poster = specs[0]['poster']
record = {**poster, 'source_sha256':next(iter(source_hashes)),
          'output':output.relative_to(ROOT).as_posix(), 'output_sha256':digest(output.read_bytes()),
          'width':width, 'height':height,
          'equivalent_sources':[s['poster']['source'] for s in specs]}
evidence = json.loads((ROOT / 'docs/re/audio-demo-current-evidence.json').read_text(encoding='utf-8'))
product = next(p for p in evidence['products'] if p['product_id'] == 3942)
paths = [(component, jsx) for page in product['pages'] for component in page['components']
         for jsx in component['jsx'] if jsx['component'] == '"path"'
         and jsx['props'].get('d') == 'M0,16V0L12,8Z']
assert len(paths) == 1
component, jsx = paths[0]
svg = ET.Element('svg', {'xmlns':'http://www.w3.org/2000/svg', 'viewBox':'0 0 20 20'})
ET.SubElement(svg, 'path', {**jsx['props'], 'fill':'#fff'})
play = ROOT / 'assets/synapse/audio-demo-play.svg'
emit(play, (ET.tostring(svg, encoding='unicode') + '\n').encode('utf-8'))
records = [record, {'source':component['path'], 'source_sha256':digest((ROOT / component['path']).read_bytes()),
                   'output':play.relative_to(ROOT).as_posix(), 'output_sha256':digest(play.read_bytes()),
                   'component_offset':component['offset'], 'width':20, 'height':20}]
font_hashes = set()
fonts = []
for spec in specs:
    for css in spec['css']:
        text = (ROOT / css['path']).read_text(encoding='utf-8')
        assert digest((ROOT / css['path']).read_bytes()) == css['sha256']
        match = re.search(r'@font-face\{font-family:video-react;font-style:normal;font-weight:400;src:url\(data:application/font-woff;base64,([^)]*)\)', text)
        if match:
            data = base64.b64decode(match.group(1), validate=True)
            font_hashes.add(digest(data))
            fonts.append({'source':css['path'], 'source_sha256':css['sha256'],
                          'font_offset':match.start(), 'font_sha256':digest(data)})
            for codepoint, marker in [(0xF200, '.video-react-icon-play-arrow:before'),
                                      (0xF215, '.video-react-icon-fullscreen:before')]:
                matching = [rule for rule in text.split('}') if marker in rule
                            and f'content:"{chr(codepoint)}"' in rule]
                assert len(matching) == 1, f'Current source glyph changed: {marker}'
assert len(fonts) == 3 and len(font_hashes) == 1, 'Current three video-react fonts differ'
font = TTFont(io.BytesIO(data))
glyphs = font.getGlyphSet()
units = font['head'].unitsPerEm
for codepoint, name in [(0xF200, 'play'), (0xF215, 'fullscreen')]:
    glyph_name = font.getBestCmap()[codepoint]
    pen = SVGPathPen(glyphs)
    glyphs[glyph_name].draw(pen)
    glyph_svg = ET.Element('svg', {'xmlns':'http://www.w3.org/2000/svg',
                                  'viewBox':f'0 0 {units} {units}'})
    ET.SubElement(glyph_svg, 'path', {'d':pen.getCommands(), 'fill':'#fff',
                                    'transform':f'translate(0 {units}) scale(1 -1)'})
    output = ROOT / f'assets/synapse/audio-demo-control-{name}.svg'
    emit(output, (ET.tostring(glyph_svg, encoding='unicode') + '\n').encode('utf-8'))
    records.append({**fonts[0], 'equivalent_sources':fonts,
                    'codepoint':codepoint, 'glyph_name':glyph_name,
                    'output':output.relative_to(ROOT).as_posix(),
                    'output_sha256':digest(output.read_bytes()),
                    'width':units, 'height':units})
emit(ROOT / 'assets/synapse/audio-demo-manifest.json', (json.dumps(records, indent=2)+'\n').encode('utf-8'))
embedded = '''// Prepared from current 1392/1442/3942 video-react font outlines.
pub(super) fn audio_demo_controls_load(path: &str) -> Option<&'static [u8]> {
    match path {
        "synapse/audio-demo-control-play.svg" => Some(include_bytes!("audio-demo-control-play.svg")),
        "synapse/audio-demo-control-fullscreen.svg" => Some(include_bytes!("audio-demo-control-fullscreen.svg")),
        _ => None,
    }
}
pub(super) fn audio_demo_controls_list(path: &str) -> Vec<gpui_kit::SharedString> {
    if path == "synapse" || path == "synapse/" || path.is_empty() {
        vec!["synapse/audio-demo-control-play.svg".into(), "synapse/audio-demo-control-fullscreen.svg".into()]
    } else {
        Vec::new()
    }
}
'''
emit(ROOT / 'assets/synapse/audio-demo-controls-embedded.rs', embedded.encode('utf-8'))
print(f'Prepared current audio demo poster ({width} x {height}); all three source copies match.')
