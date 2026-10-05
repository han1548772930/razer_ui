"""Prepare the current Cooling Pad's rotated axis text from font outlines.

No application runs. Roboto is the current bundled source font. CJK glyphs use
the named installed Windows fallback fonts, explicitly recorded as local font
fallbacks rather than claimed source assets. No whole system font is bundled.
"""
from pathlib import Path
import hashlib
import json
import sys
from fontTools.ttLib import TTFont
from fontTools.pens.svgPathPen import SVGPathPen
from fontTools.pens.transformPen import TransformPen

ROOT = Path(__file__).resolve().parents[1]
CHECK = '--check' in sys.argv
ROBOTO = ROOT / 'assets/synapse/Roboto-Regular.ttf'
FALLBACK = {
    'zh-CN': ('C:/Windows/Fonts/msyh.ttc', 0),
    'zh-TW': ('C:/Windows/Fonts/msjh.ttc', 0),
    'ja': ('C:/Windows/Fonts/YuGothR.ttc', 0),
    'kr': ('C:/Windows/Fonts/malgun.ttf', 0),
    'ru': ('C:/Windows/Fonts/segoeui.ttf', 0),
}

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def emit(path, text):
    data = text.encode('utf8')
    if CHECK:
        if path.read_bytes() != data:
            raise ValueError(f'Stale resource: {path}')
    else:
        path.write_bytes(data)

def kern(font, first, second):
    if 'GPOS' not in font:
        return 0
    table = font['GPOS'].table
    indices = set()
    for record in table.FeatureList.FeatureRecord:
        if record.FeatureTag == 'kern':
            indices.update(record.Feature.LookupListIndex)
    adjustment = 0
    for index in sorted(indices):
        lookup = table.LookupList.Lookup[index]
        if lookup.LookupType != 2:
            continue
        for sub in lookup.SubTable:
            if first not in sub.Coverage.glyphs:
                continue
            if sub.Format == 1:
                pairs = sub.PairSet[sub.Coverage.glyphs.index(first)].PairValueRecord
                value = next((pair.Value1 for pair in pairs if pair.SecondGlyph == second), None)
            elif sub.Format == 2:
                a = sub.ClassDef1.classDefs.get(first, 0)
                b = sub.ClassDef2.classDefs.get(second, 0)
                value = sub.Class1Record[a].Class2Record[b].Value1
            else:
                raise ValueError('Unexpected GPOS pair format')
            adjustment += getattr(value, 'XAdvance', 0) or 0
    return adjustment

regular = TTFont(ROBOTO)
records = []
labels = {}
current = json.loads((ROOT / 'docs/re/armory-remaining-roots-current-evidence.json').read_text(encoding='utf8'))
axis_source = {item['locale']: item for product in current['products'] if product['product_id'] == 3907
               for item in product['axis_labels']}

def prepare(name, text, locale, font_size, line_height):
    fallback = None
    cmap = regular.getBestCmap()
    if any(ord(char) not in cmap for char in text):
        path, font_number = FALLBACK[locale]
        fallback = TTFont(path, fontNumber=font_number)
    faces = []
    for char in text:
        font = regular if ord(char) in cmap else fallback
        glyph = font.getBestCmap().get(ord(char))
        if glyph is None:
            raise ValueError(f'Missing actual glyph for {locale}:{char}')
        faces.append((font, glyph))
    paths = []
    x = 0.
    for index, (font, glyph) in enumerate(faces):
        scale = font_size / font['head'].unitsPerEm
        if index and faces[index - 1][0] is font:
            x += kern(font, faces[index - 1][1], glyph) * scale
        asc, desc = font['hhea'].ascent * scale, font['hhea'].descent * scale
        baseline = (line_height - (asc - desc)) / 2 + asc
        glyphs = font.getGlyphSet()
        pen = SVGPathPen(glyphs)
        glyphs[glyph].draw(TransformPen(pen, (scale, 0, 0, -scale, x, baseline)))
        paths.append(pen.getCommands())
        x += font['hmtx'].metrics[glyph][0] * scale
    svg = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{line_height}" height="{x:.6f}" '
           f'viewBox="0 0 {line_height} {x:.6f}"><g transform="translate(0 {x:.6f}) rotate(-90)">'
           + ''.join(f'<path d="{path}"/>' for path in paths if path) + '</g></svg>\n')
    output = f'assets/synapse/armory-cooling-axis-{name}.svg'
    emit(ROOT / output, svg)
    fonts = [{'path': 'assets/synapse/Roboto-Regular.ttf', 'sha256': digest(ROBOTO), 'kind': 'current bundled source font'}]
    if fallback:
        path, font_number = FALLBACK[locale]
        fonts.append({'path': path, 'sha256': digest(Path(path)), 'font_number': font_number,
                      'kind': 'installed Windows fallback; source browser fallback identity is not asserted'})
    item = {'asset': output, 'text': text, 'font_size': font_size, 'line_height': line_height,
            'advance': round(x, 6), 'sha256': hashlib.sha256(svg.encode()).hexdigest(),
            'fonts': fonts, 'rotation_degrees': 270}
    if name in axis_source:
        source = axis_source[name]
        if source['value'] != text or digest(ROOT / source['path']) != source['sha256']:
            raise ValueError(f'Current product locale proof changed: {locale}')
        item['current_product_locale'] = source
    records.append(item)
    return {'asset': output.removeprefix('assets/'), 'advance': round(x, 6)}

for file in sorted((ROOT / 'locales').glob('*.json')):
    locale = file.stem
    translations = json.loads(file.read_text(encoding='utf8'))
    labels[locale] = prepare(locale, translations['FAN_SPEED'], locale, 12, 14)
units = {key: prepare(key, text, 'en', 14, 16) for key, text in [('percent','%'),('rpm','RPM')]}
data = {'labels': labels, 'units': units}
emit(ROOT / 'src/features/armory_product/cooling_axis_data.json', json.dumps(data, ensure_ascii=False, indent=2)+'\n')
emit(ROOT / 'docs/re/armory-cooling-axis-resources.json', json.dumps({
    'method': 'Static TrueType glyph outlines, GPOS kern pair adjustment, current source font sizes and rotation. No application execution.',
    'source_evidence': 'docs/re/armory-remaining-roots-current-evidence.json',
    'records': records,
}, ensure_ascii=False, indent=2)+'\n')

manifest_path = ROOT / 'assets/synapse/manifest.json'
manifest = json.loads(manifest_path.read_text(encoding='utf8'))
embedded_path = ROOT / 'assets/synapse/embedded.rs'
embedded = embedded_path.read_text(encoding='utf8')
for record in records:
    target = record['asset']
    existing = next((item for item in manifest['entries'] if item['output'] == target), None)
    entry = {'source': 'assets/synapse/Roboto-Regular.ttf', 'source_sha256': digest(ROBOTO),
             'output': target, 'sha256': record['sha256'], 'source_kind': 'font_glyph_outline',
             'evidence': 'docs/re/armory-cooling-axis-resources.json', 'text': record['text'],
             'transform': 'Current source axis text, font size and 270-degree rotation; named local system fallback where required.'}
    asset = target.removeprefix('assets/')
    if CHECK:
        if not existing or existing['sha256'] != record['sha256'] or f'"{asset}"' not in embedded:
            raise ValueError(f'Missing current registration: {target}')
    else:
        if existing:
            existing.update(entry)
        else:
            manifest['entries'].append(entry)
        if f'"{asset}"' not in embedded:
            line = f'    ("{asset}", include_bytes!("{Path(target).name}") as &[u8]),\n'
            embedded = embedded.rstrip().removesuffix(']') + line + ']\n'
if not CHECK:
    manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2)+'\n', encoding='utf8')
    embedded_path.write_text(embedded, encoding='utf8')
print(f'Cooling axis: {len(labels)} locale labels and two units prepared and verified.')
