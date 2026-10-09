"""Static validation of prepared native device data; does not launch the UI."""
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def load(path):
    return json.loads((ROOT / path).read_text(encoding='utf8'))


def check_source(path, expected):
    assert path.startswith('.ref/devices/'), path
    assert hashlib.sha256((ROOT / path).read_bytes()).hexdigest() == expected, path


def main():
    mice = load('crates/razer-pages/src/features/mouse_products_data.json')
    keyboards = load('crates/razer-pages/src/features/keyboard_products_data.json')
    for products in (mice, keyboards):
        assert len(products) == len({product['product_id'] for product in products})
        for product in products:
            if product['image']:
                assert (ROOT / 'assets' / product['image']).is_file(), product['image']
    mouse_inputs = 0
    cobra = next(p for p in mice if p['product_id'] == 162)
    assert cobra['source'].startswith('.ref/devices/162/')
    assert (cobra['info']['productId'], cobra['info']['category']) == (163, 'MOUSE')
    assert (cobra['min_dpi'], cobra['max_dpi'], cobra['dpi_step'], cobra['support_xy']) == (200, 8500, 100, False)
    assert cobra['rates']['POLLING_RATE'] == [125, 500, 1000]
    assert cobra['calibration'] == 'surface' and len(cobra['mats']) == 18
    assert [b['inputID'] for b in cobra['groups'][0]['buttonList']] == [
        'LeftClick', 'RightClick', 'ScrollButton', 'ScrollUp', 'ScrollDown', 'Button4', 'Button5', 'DKM_SB_03']
    assert cobra['groups'][0]['buttonList'][0]['isEnabled'] is False
    for product in mice:
        check_source(product['source'], product['source_sha256'])
        assert 0 < product['min_dpi'] <= product['max_dpi'] and product['dpi_step'] > 0
        assert product['groups'] or product['product_id'] == 182
        for group in product['groups']:
            assert group['groupName'] and group['buttonList']
            # Some source groups contain captions/battery decorations alongside
            # mappable inputs. They must not inflate the input coverage count.
            inputs = [button['inputID'] for button in group['buttonList'] if button.get('inputID')]
            assert len(inputs) == len(set(inputs)), (product['product_id'], group['groupName'])
            assert all(isinstance(button.get('isEnabled', True), bool) for button in group['buttonList'])
            mouse_inputs += len(inputs)
    shapes = 0
    for product in keyboards:
        for evidence in product['source_files']:
            check_source(evidence['path'], evidence['sha256'])
        assert product['shapes'], product['product_id']
        assert all(math.isfinite(value) and value > 0 for value in product['viewbox'])
        keys = {key['inputID'] for key in product['keys']}
        regions = set()
        for shape in product['shapes']:
            assert shape['id'] in keys
            assert all(math.isfinite(value) for value in shape['bounds'])
            assert shape['bounds'][2] > 0 and shape['bounds'][3] > 0
            identity = (shape['id'], *shape['bounds'][:2])
            assert identity not in regions, (product['product_id'], identity)
            regions.add(identity)
        shapes += len(product['shapes'])
        controls = product['controls']
        if 'TAB_POWER' in product['pages']:
            assert controls['power_kind'] and controls['dim_kind'], product['product_id']
        if controls['power_bounds']:
            lower, upper, step = controls['power_bounds']
            value = controls['defaults'].get('powerSavingValue', controls['defaults'].get('powerSaving', {}).get('value'))
            assert lower <= value <= upper and step > 0
    mounted = load('docs/re/keyboard-product-pages.json')
    for product in mounted['products']:
        for page in product['pages']:
            check_source(page['path'], page['sha256'])
            for component in page['components']:
                assert 'webpackChunk' not in component['source'], (product['product_id'], page['key'])
                assert not (component['source'].startswith('{') and len(component['source']) > 15000)
    result = {
        'schema_version': 1,
        'verification': 'Static JSON/source hash/resource validation; no application or tests executed.',
        'mouse': {'specifications': len(mice), 'source_workspace_products': sum(bool(p['groups']) for p in mice),
                  'source_inputs': mouse_inputs, 'prepared_default_illustrations': sum(bool(p['image']) for p in mice),
                  'asymmetric_three_preset_products': [p['product_id'] for p in mice if p['calibration'] == 'smart_preset']},
        'keyboard': {'source_workspace_products': len(keyboards), 'default_layout_shapes': shapes,
                     'prepared_default_illustrations': sum(bool(p['image']) for p in keyboards),
                     'gaming_mode_products': [p['product_id'] for p in keyboards if p['controls']['gaming_mode']],
                     'power_products': [p['product_id'] for p in keyboards if p['controls']['power_kind']]},
        'remaining_ui_work': {
            'mouse': ['Full mapping categories and source drawer layout', 'Nondefault editions, side-panel illustrations and firmware/connection branches',
                      'Calibration mat management and hardware calibration', 'Dynamic sensitivity graph editor', 'Pairing flow'],
            'keyboard': ['Nondefault keyboard layouts and editions', 'Full mapping categories and source drawer layout',
                         'Remaining Customize controls, including polling, Snap Tap and command dial',
                         'Actuation, calibration, OLED editors and pairing', 'Lighting effect parameters and advanced Chroma controls']},
        'runtime_not_verified': ['Rendered pixels, font fallback, scrolling and focus', 'Hardware writes and readback', 'Chroma service integration'],
        'artifacts': {path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest() for path in [
            'crates/razer-pages/src/features/mouse_products.rs', 'crates/razer-pages/src/features/mouse_products_data.json',
            'crates/razer-pages/src/features/keyboard_products.rs', 'crates/razer-pages/src/features/keyboard_products_data.json']},
    }
    output = ROOT / 'docs/re/mouse-keyboard-native-coverage.json'
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf8')
    print(f'Validated {len(mice)} mouse specifications / {mouse_inputs} inputs and {len(keyboards)} keyboards / {shapes} source shapes.')


if __name__ == '__main__':
    main()
