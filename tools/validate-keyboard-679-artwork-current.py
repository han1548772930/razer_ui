"""Static receipt, image-byte and geometry validation; no vendor execution."""
import hashlib
import json
from pathlib import Path
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
sha = lambda b: hashlib.sha256(b).hexdigest()


def main():
    receipt = json.loads((ROOT/'docs/re/keyboard-679-source-artwork-current.json').read_text(encoding='utf8'))
    count = 0
    for row in receipt['receipts']+receipt['css']+receipt['images']:
        raw = (ROOT/row['source_file']).read_bytes()
        assert sha(raw) == row['file_sha256'], row['source_file']
        text = raw.decode('utf8')
        start, end = row['utf16_range']
        source = text.encode('utf-16-le')[start*2:end*2].decode('utf-16-le')
        assert source == row['source']
        assert sha(source.encode('utf8')) == row['slice_sha256']
        if 'byte_offset' in row:
            assert len(text.encode('utf-16-le')[:start*2].decode('utf-16-le').encode('utf8')) == row['byte_offset']
        count += 1
    assets = json.loads((ROOT/'assets/synapse/keyboard679-current-manifest.json').read_text(encoding='utf8'))
    for asset in assets:
        original, output = ROOT/asset['source'], ROOT/asset['output']
        assert sha(original.read_bytes()) == asset['source_sha256']
        assert sha(output.read_bytes()) == asset['output_sha256']
        if output.suffix == '.svg':
            assert original.read_bytes() == output.read_bytes()
        else:
            with Image.open(original) as src, Image.open(output) as png:
                assert src.size == png.size == tuple(asset['pixel_size'])
                assert src.convert('RGBA').tobytes() == png.convert('RGBA').tobytes()
    raw = json.loads((ROOT/'.work/keyboard-679-artwork/raw.json').read_text(encoding='utf8'))
    native = json.loads((ROOT/'crates/razer-pages/src/features/keyboard_source_artwork_data.json').read_text(encoding='utf8'))
    # The trusted converter deterministically decodes original SVG literal paths.
    import importlib.util
    spec = importlib.util.spec_from_file_location('prepare', ROOT/'tools/prepare-keyboard-679-artwork-current.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    assert native['product_id'] == raw['product_id']
    assert native['default_keys'] == module.shapes(raw['default_groups'])
    assert native['layouts'] == [{'layout_id':layout['layout_id'], 'keys':module.shapes(layout['groups'])} for layout in raw['layouts']]
    assert native['viewbox'] == [730, 387] and native['image_css_size'] == [730, 340]
    print(json.dumps({'validated_receipts':count, 'validated_assets':len(assets), 'validated_layouts':len(native['layouts'])}))


if __name__ == '__main__':
    main()
