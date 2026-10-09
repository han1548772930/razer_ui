"""Statically validate current 1383 artwork source provenance and native bindings."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
def read(name): return json.loads((ROOT / name).read_text(encoding='utf8'))
def sha(data): return hashlib.sha256(data).hexdigest()
files = {}
def verify(row):
    if row['path'] not in files:
        data = (ROOT / row['path']).read_bytes()
        files[row['path']] = (sha(data), data.decode('utf8').encode('utf-16-le'))
    digest, source = files[row['path']]
    assert digest == row['sha256'], row['path']
    assert source[row['offset'] * 2:row['end'] * 2].decode('utf-16-le') == row['source']

evidence = read('docs/re/audio-oled-artwork-current-evidence.json')
data = read('crates/razer-pages/src/features/audio_oled_artwork_data.json')
assert data['product_id'] == 1383 and data['fps'] == 15
assert len(data['animations']) == 6 and len(data['images']) == 10 and len(data['emotes']) == 104
assert data['default_emote'] == 'face-tongue-animated'
assert len({r['id'] for r in data['emotes']}) == 104
assert sha((ROOT / evidence['manifest']['path']).read_bytes()) == evidence['manifest']['sha256']
for row in evidence['receipts']: verify(row)
tooltip_module = next(row for row in evidence['receipts'] if row['module'] == 58837 and row['symbol'] == 'module/defaultProps')
assert 'h.defaultProps={position:d,className:""}' in tooltip_module['source']
for row in evidence['assets']:
    verify(row['receipt']); verify(row['loader'])
for row in evidence['icons']: verify(row['receipt'])
for row in evidence['css']:
    assert sha((ROOT / row['path']).read_bytes()) == row['sha256']
for name in ('CustomizeAnimation_tooltip__WEWaQ', 'CustomizeImage_tooltip__haCIB'):
    rule = next(row for row in evidence['css'] if row['selector'] == '.' + name)
    properties = {item['property']: item['value'] for item in rule['properties']}
    assert properties['margin-left'] == '270px' and properties['float'] == 'left'
for locale in ('en', 'zh-CN'):
    translations = read(f'locales/{locale}.json')
    assert all(key in translations for key in data['labels'].values())
assets = read('assets/synapse/audio-oled-artwork-assets.json')
assert len(assets) == 111
for row in assets:
    assert sha((ROOT / row['source']).read_bytes()) == row['source_sha256']
    assert sha((ROOT / row['output']).read_bytes()) == row['output_sha256']
registered = {row['output'] for row in assets}
for row in data['emotes']:
    assert 'assets/' + row['asset'] in registered
for row in data['animations'] + data['images']:
    assert (ROOT / 'assets' / row['asset']).is_file()
native = (ROOT / 'crates/razer-pages/src/features/audio_oled_artwork.rs').read_text(encoding='utf8')
crop = (ROOT / 'crates/razer-pages/src/features/audio_oled_artwork_crop.rs').read_text(encoding='utf8')
for marker in ('pub(super) fn default_value', 'pub(super) fn normalize', 'pub(super) fn preview',
               'enabled_count == 1', 'self.draft["selectedIdx"] = json!(selected)',
               'owner.draft["oledHome"][key] = value', 'cx.emit(AudioProductChanged)',
               'media_decode::preview_source', 'local_crop', 'isDataMatch', 'max_width(800.)', 'super::tooltip::artwork'):
    assert marker in native, marker
for marker in ('prompt_for_paths', 'media_decode::load_preset', 'import_generation != generation',
               'CropCanvas::new', 'crop.canvas.zoom_by', 'crop.initial_canvas',
               'item["local_crop"] = json!(crop.canvas)', 'item["size"] = json!(0)',
               'window.on_mouse_event'):
    assert marker in crop, marker
assert 'size, preview' not in crop and 'SET_OLED_DISPLAY_' not in crop
print('1383 artwork validated: 6/10 slots, 104 emotes, source defaults/CSS/locales, 111 assets, local crop and Apply bindings.')
