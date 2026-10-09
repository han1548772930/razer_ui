"""Static receipts, locale bindings and mounted layer operation checks."""
import hashlib
import json
from pathlib import Path

root = Path(__file__).resolve().parents[1]
evidence = json.loads((root / 'docs/re/chroma-studio-layers-source.json').read_text(encoding='utf8'))
cache = {}
for row in evidence['receipts']:
    assert row['path'].startswith('.ref/applications/synapse/chroma-studio/')
    if row['path'] not in cache:
        raw = (root / row['path']).read_bytes()
        cache[row['path']] = hashlib.sha256(raw).hexdigest(), raw.decode('utf8').encode('utf-16-le')
    digest, text = cache[row['path']]
    assert digest == row['sha256']
    assert text[row['offset'] * 2:row['end'] * 2].decode('utf-16-le') == row['source']
source = {(row['module'], row['symbol']): row['source'] for row in evidence['receipts']}
assert 'c!==e.title?Y(' in source[9286, 'T']
assert 'number:i=1' in source[9286, 'Y'] and 't.type===r.type&&t.title===e' in source[9286, 'Y']
assert 'title:(0,r.oT)(t)' in source[9286, 'I']
assert 'opened:!1' in source[9286, 'w']
assert 'f.uR.filter(e=>e!==w.name&&!ce(e)&&!f.Vg.includes(e))' in source[4264, 'J']
data = json.loads((root / 'crates/razer-pages/src/features/chroma_studio_data.json').read_text(encoding='utf8'))
assert all('CHANGE_EFFECT' in locale and 'TEXT_NEW_GROUP' in locale for locale in data['locales'].values())
native = (root / 'crates/razer-pages/src/features/chroma_studio.rs').read_text(encoding='utf8')
helper = (root / 'crates/razer-pages/src/features/chroma_studio_layers.rs').read_text(encoding='utf8')
for marker in ['title: effect.label.clone()', 'self.can_remove_layer(id)', 'self.first_visible_layer()',
               'label(&layer.title)', 'label("CHANGE_EFFECT")', '.submenu(', '.disabled(!can_remove)']:
    assert marker in native, marker
for marker in ['fn unique_title', 'fn suffix_base', 'fn change_layer_effect', 'layer.paint_params = effect.paint_params.clone()']:
    assert marker in helper, marker
print(f"Studio layer operations: {len(evidence['receipts'])} current receipts, names/locales and mounted native bindings validated.")
