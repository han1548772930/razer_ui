"""Validate embedded descriptors and mounted page identity without running Rust."""
import hashlib
import json
import math
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
MISSING = object()
inputs = {}


def read(path):
    data = (ROOT / path).read_bytes()
    inputs[path] = hashlib.sha256(data).hexdigest()
    return json.loads(data)


def pointer(root, path):
    assert path.startswith('/'), path
    for part in path[1:].split('/'):
        part = part.replace('~1', '/').replace('~0', '~')
        try:
            root = root[int(part)] if isinstance(root, list) else root[part]
        except (KeyError, IndexError, TypeError, ValueError):
            return MISSING
    return root


def paths(root, path):
    if path.startswith('@image/'):
        return [f'/image/{i}/dataSet/{path[7:]}' for i in range(len(root['image']))]
    if path.startswith('@view/'):
        return [f'/camera/viewPresets/data/{i}/{path[6:]}'
                for i in range(len(root['camera']['viewPresets']['data']))]
    return [path]


def validate_controls(filename):
    products = read(filename)
    assert len({p['product_id'] for p in products}) == len(products), filename
    counts = Counter()
    sentinels = []
    for product in products:
        root, pid = product['profile'], product['product_id']
        assert isinstance(root, dict), pid
        fields = product.get('device_fields', [])
        assert len(fields) == len(set(fields)) and all(field in root for field in fields), pid
        pages, keys = set(), set()
        for page in product['pages']:
            assert page['key'] not in pages, (pid, page['key'])
            pages.add(page['key'])
            for section in page['sections']:
                assert isinstance(section['title'], str)
                assert section.get('column') in (None, 'left', 'right')
                for control in section['controls']:
                    key, kind = control['key'], control['kind']
                    # Descriptors cannot borrow another product's control id
                    # or implementation evidence. This caught a 179 pairing
                    # action accidentally inserted into 126's lighting page.
                    assert key.startswith(f'{pid}:'), (pid, key)
                    if source := control.get('source'):
                        assert source['path'].startswith(f'.ref/devices/{pid}/'), (key, source)
                    assert key not in keys, (pid, key)
                    keys.add(key)
                    counts[kind] += 1
                    assert kind in ('slider', 'switch', 'toggle', 'select', 'options', 'image_options', 'reset', 'oled_presets', 'preset', 'pan_tilt', 'keys', 'direction'), key
                    if kind == 'oled_presets':
                        assert pid == 691 and control['path'] == '/oled/homeScreenDisplay/selected', key
                    assert isinstance(control['label'], str) and control['label'], key
                    if kind == 'image_options':
                        embedded = (ROOT / 'assets/synapse/embedded.rs').read_text(encoding='utf-8')
                        for option in control['options']:
                            if 'image' in option:
                                assert '"' + option['image'] + '"' in embedded, (key, option['image'])
                    targets = paths(root, control['path'])
                    assert targets and all(pointer(root, p) is not MISSING for p in targets), key
                    options = [o['value'] for o in control.get('options', [])]
                    for option in control.get('options', []):
                        assert type(option.get('disabled_on_ble', False)) is bool, key
                        if option.get('disabled_on_ble'):
                            # Option-level gating is rendered by the native button group.
                            assert kind == 'options', key
                    assert len({json.dumps(v, sort_keys=True) for v in options}) == len(options), key
                    if kind in ('select', 'options', 'image_options', 'preset', 'direction'):
                        assert options, key
                        for path in targets:
                            value = pointer(root, path)
                            if value not in options:
                                # 3940's mounted reducer starts at 15, while its
                                # product choices start at 20. The source leaves
                                # every option unselected until hydration/edit.
                                assert pid in (3940, 3942, 3949) and (path, value, options) == (
                                    '/runtime/powerSaving/value', 15, [20, 30, 40, 50, 60]), key
                    if kind in ('toggle', 'switch'):
                        assert not options or len(options) == 2, key
                        assert all(pointer(root, p) in options if options
                                   else isinstance(pointer(root, p), bool) for p in targets), key
                    if kind == 'reset':
                        assert 'reset_value' in control, key
                    if kind == 'pan_tilt':
                        assert control['tilt_path'].startswith('@view/'), key
                        assert control['max_pan_tilt'] > 0 and control['box_width'] > 0, key
                        for path in paths(root, control['tilt_path']):
                            assert pointer(root, path) is not MISSING, key
                    if kind == 'direction':
                        assert control['description'], key
                        assert control['disabled_unless'] == '/camera/watermark/isEnabled', key
                        for path in targets:
                            value = pointer(root, path)
                            assert value in ('left-bottom', 'right-bottom', 'left-top',
                                             'right-top', 'center-bottom', 'center-top'), (key, value)
                    if kind == 'keys':
                        assert control['placeholder'], key
                        for path in targets:
                            value = pointer(root, path)
                            assert isinstance(value, list), key
                            assert all(isinstance(item, str) for item in value), key
                    # 取景块的复合禁用条件：`ldc && (4K 30FPS | 1440p 30FPS)`。
                    # 每组必须是 ldc 开关加一个真实存在的分辨率记录。
                    if 'disabled_when_any' in control:
                        groups = control['disabled_when_any']
                        assert groups, key
                        resolution = pointer(root, '/camera/resolution')
                        for group in groups:
                            assert len(group) == 2, key
                            fans = {condition['path']: condition['value'] for condition in group}
                            assert fans.get('/camera/ldc') is True, key
                            gated = fans.get('/camera/resolution')
                            assert gated is not None, key
                            if resolution is not None:
                                assert gated in (
                                    {'width': 3840, 'height': 2160, 'fps': 30},
                                    {'width': 2560, 'height': 1440, 'fps': 30},
                                ), (key, gated)
                    if kind == 'slider':
                        low, high, step = (control[k] for k in ('min', 'max', 'step'))
                        assert all(math.isfinite(v) for v in (low, high, step)), key
                        assert low < high and step > 0, key
                        for path in targets:
                            value = pointer(root, path)
                            assert type(value) in (int, float) and math.isfinite(value), key
                            if not low <= value <= high:
                                # Current reducer seed is an inactive sentinel, not
                                # a user-selectable value. No broad range exemption.
                                assert (pid, path, value, low, high) == (
                                    3907, '/switchOffLighting/idleMinutes', 0, 1, 15), (key, value)
                                assert pointer(root, '/switchOffLighting/isIdleEnabled') is False
                                sentinels.append({'product_id': pid, 'path': path, 'value': value})
                    bindings = [control[k] for k in ('disabled_when', 'disabled_unless', 'enabled_from_value') if k in control]
                    bindings += control.get('disabled_unless_all', [])
                    bindings += [control[k]['path'] for k in ('visible_when', 'minimum_when') if k in control]
                    for binding in bindings:
                        if (pid, binding) == (207, '/hardware/pairedMousePowered'):
                            # Live paired-mouse state is deliberately unavailable
                            # in a local draft; both optimizer controls stay disabled.
                            assert (binding == control.get('disabled_unless')
                                    or binding in control.get('disabled_unless_all', []))
                            continue
                        assert all(pointer(root, p) is not MISSING for p in paths(root, binding)), (key, binding)
    return {'products': len(products), 'controls': dict(counts), 'inactive_sentinels': sentinels}


def validate_oled_home():
    product, = read('src/features/keyboard_oled_data.json')
    section = product['pages'][0]['sections'][0]
    assert section['title'] == 'OLED_HOME_SCREEN_DISPLAY_TITLE'
    assert section.get('column') is None
    enabled, modes, presets = section['controls']
    assert presets['kind'] == 'oled_presets' and presets['disabled_unless'] == '/oled/homeScreenDisplay/enabled'
    assert enabled['kind'] == 'switch' and enabled['path'] == '/oled/homeScreenDisplay/enabled'
    assert modes['disabled_unless'] == enabled['path']
    assert [o['value'] for o in modes['options']] == [0, 1, 4, 2, 5, 6, 3]
    assert [o['value'] for o in modes['options'] if o.get('disabled_on_ble')] == [5, 6]
    editor = read('src/features/keyboard_oled_editor_data.json')
    assert product['profile']['oled']['homeScreenDisplay'] == editor['home_default']
    assert 'oled' in product['device_fields']
    embedded = (ROOT / 'assets/synapse/embedded.rs').read_text(encoding='utf8')
    for kind, count, ext in [('animation', 6, 'webp'), ('image', 10, 'png')]:
        defaults = editor['preset_defaults'][kind]
        assert product['profile']['oled'][kind] == defaults
        assert defaults['selectedIdx'] == 0 and len(defaults['list']) == count
        for ix, item in enumerate(defaults['list'], 1):
            assert item == {'id': f'{kind}-{ix}', 'custom': False, 'enabled': True}
            assert f'"synapse/oled-home-{kind}-{ix}.{ext}"' in embedded
    for receipt in editor['editor_evidence']:
        source = (ROOT / receipt['path']).read_text(encoding='utf8')
        # Acorn offsets use UTF-16 code units, not Python Unicode code points.
        fragment = source.encode('utf-16-le')[receipt['offset'] * 2:receipt['end'] * 2].decode('utf-16-le')
        assert hashlib.sha256(fragment.encode()).hexdigest() == receipt['sha256']
    evidence = read('docs/re/keyboard-oled-current-evidence.json')
    for source in evidence['source_files']:
        assert hashlib.sha256((ROOT / source['path']).read_bytes()).hexdigest() == source['sha256']
    for control in (enabled, modes):
        assert control['source'] == evidence['home_display']
    return {'modes': 7, 'ble_disabled_modes': [5, 6], 'preset_editors': {'animation': 6, 'image': 10}, 'scope': 'device'}


def validate_help():
    records = read('src/features/source_help_data.json')
    registry = read('docs/re/product-registration-audit.json')['products']
    generated = {p['product_id']: p for p in records}
    assert len(generated) == len(records)
    expected = {p['product_id']: p for p in registry
                if p['adapter_status'] != 'existing_partial_native_adapter'}
    assert generated.keys() == expected.keys()
    boolean_fields = ('serial', 'registration', 'firmware', 'view_more', 'reset', 'oled_reset', 'obm',
                      'system_info', 'tutorial', 'thx_instructions', 'camo')
    total = 0
    for pid, record in generated.items():
        source = record['source']
        assert source.startswith('.ref/devices/'), source
        assert hashlib.sha256((ROOT / source).read_bytes()).hexdigest() == record['evidence'], source
        pages = record['pages']
        identities = {(p['source'], p['offset']) for p in pages}
        mounted = {(nav['source'], item['offset']) for nav in expected[pid]['navigation']
                   for item in nav['items'] if item['name']['value'] == 'HELP'}
        assert len(identities) == len(pages) and identities == mounted, pid
        for page in pages:
            assert all(type(page[k]) is bool for k in boolean_fields), pid
            assert isinstance(page['reset_title'], str), pid
            assert page['firmware_reset'] is None or isinstance(page['firmware_reset'], dict), pid
        total += len(pages)
    return {'products': len(records), 'mounted_pages': total}


def validate_corex():
    fan = read('src/features/corex_fan_data.json')
    product = next(p for p in read('src/features/accessory_system_products_data.json') if p['product_id'] == 3921)
    assert product['pages'] == ['TAB_CUSTOMIZE']
    assert product['initial']['fanCurve'] == fan['initial']
    assert product['presets'] == fan['curves']
    assert fan['modes'] == {'Auto': 'Auto', 'Smart': 'Manual'}
    assert (fan['min_rpm'], fan['max_rpm']) == (1000, 2800)
    for file in fan['source_files']:
        assert file['path'].startswith('.ref/devices/3921/')
        assert hashlib.sha256((ROOT / file['path']).read_bytes()).hexdigest() == file['sha256']
    receipt = fan['state_source']
    text = (ROOT / receipt['path']).read_text(encoding='utf8')
    fragment = text.encode('utf-16-le')[receipt['offset'] * 2:receipt['end'] * 2].decode('utf-16-le')
    assert hashlib.sha256(fragment.encode()).hexdigest() == receipt['sha256']
    counts = []
    for mode, curves in fan['curves'].items():
        for target, points in curves.items():
            assert 2 <= len(points) <= 20
            xs = [p['temperature'] for p in points]
            ys = [p['fanSpeedValue'] for p in points]
            assert xs == sorted(set(xs)) and ys == sorted(ys)
            assert fan['axes'][target][0] <= xs[0] <= xs[-1] <= fan['axes'][target][-1]
            assert 1000 <= ys[0] <= ys[-1] <= 2800
            counts.append(len(points))
    assert counts == [7, 7, 8, 8, 8, 9]
    return {'presets': 3, 'curves': 6, 'points': sum(counts), 'live_telemetry': False}


def main():
    result = {'schema_version': 1, 'verification': 'Static data validation; no application or tests executed.',
              'camera': validate_controls('src/features/source_controls_data.json'),
              'accessory': validate_controls('src/features/accessory_controls_data.json'),
              'oled': validate_controls('src/features/keyboard_oled_data.json'),
              'oled_home': validate_oled_home(),
              'corex_fan': validate_corex(),
              'help': validate_help(), 'inputs_sha256': inputs}
    (ROOT / 'docs/re/native-product-data-validation.json').write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({k: v for k, v in result.items() if k != 'inputs_sha256'}, ensure_ascii=False))


if __name__ == '__main__':
    main()
