"""Audit native dispatch and generated descriptors without executing the app.

A reachable renderer is partial coverage, never proof of UI equivalence.
"""
import hashlib
import json
import re
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
inputs = {}


def read(path):
    data = (ROOT / path).read_bytes()
    inputs[path] = hashlib.sha256(data).hexdigest()
    return data.decode('utf-8')


def load(path):
    return json.loads(read(path))


def index(path):
    rows = load(path)
    result = {p['product_id']: p for p in rows}
    assert len(rows) == len(result), f'Duplicate product: {path}'
    return result


def page_map(spec):
    return {p['key']: p for p in spec.get('pages', []) if isinstance(p, dict)}


def has_content(page):
    return bool(page and page.get('sections'))


def main():
    read('tools/audit-native-product-coverage.py')
    registry = load('docs/re/product-registration-audit.json')['products']
    families = ['mouse', 'keyboard', 'gamepad', 'audio', 'system', 'accessory_system']
    specs = {name: index(f'src/features/{name}_products_data.json') for name in families}
    controls = index('src/features/source_controls_data.json')
    accessories = index('src/features/accessory_controls_data.json')
    assert not controls.keys() & accessories.keys(), 'Ambiguous SourceControls product'
    controls.update(accessories)
    oled = index('src/features/keyboard_oled_data.json')
    assert not controls.keys() & oled.keys(), 'Ambiguous OLED product'
    controls.update(oled)
    helps = index('src/features/source_help_data.json')
    # The renderer requires per-navigation-page metadata. The old links-only
    # schema silently deserialized to an empty list before October 3's fix.
    for pid, help_record in helps.items():
        assert help_record.get('pages'), f'Regenerate Help descriptors: {pid}'
        offsets = [p['offset'] for p in help_record['pages']]
        assert len(offsets) == len(set(offsets)), f'Ambiguous Help page: {pid}'
    branches = {}
    for family in families:
        source = read(f'src/features/{family}_products.rs')
        if 'match self.page.as_str()' in source:
            switch = source.split('match self.page.as_str()', 1)[1]
            branches[family] = set(re.findall(r'"([A-Z_]+)"\s*=>', switch))
    read('src/features/source_workspace.rs')
    read('src/features/product_workspace.rs')
    read('src/features/source_help.rs')
    read('src/features/source_controls.rs')
    read('src/features/source_controls/oled_presets.rs')
    read('src/features/accessory_system_products/corex_fan.rs')
    read('src/features/corex_fan_data.json')
    read('src/features/keyboard_actuation.rs')
    read('src/features/keyboard_calibration.rs')
    calibration = index('src/features/keyboard_calibration_data.json')
    read('src/features/audio_demo.rs')
    audio_demos = index('src/features/audio_demo_data.json')
    hue = load('src/features/hue_data.json')
    assert hue['product_id'] == 769
    for file in ['src/features/hue.rs', 'src/features/hue/onboarding.rs', 'src/features/hue/bridge.rs',
                 'src/features/hue/brightness.rs', 'src/features/hue/effects.rs', 'src/features/hue/preview.rs']:
        read(file)
    dedicated_pages = {}
    for feature in ['dock_pairing', 'wired_argb', 'wireless_argb', 'aether_strip', 'automation']:
        descriptor = load(f'src/features/{feature}_data.json')
        rows = descriptor if isinstance(descriptor, list) else [descriptor]
        read(f'src/features/{feature}.rs')
        for path in sorted((ROOT / f'src/features/{feature}').glob('*.rs')):
            read(path.relative_to(ROOT).as_posix())
        for row in rows:
            dedicated_pages[(row['product_id'], row['page'])] = feature
    for path in sorted((ROOT / 'src/features/source_workspace').glob('*.rs')):
        read(path.relative_to(ROOT).as_posix())
    actuation = index('src/features/keyboard_actuation_data.json')
    for pid, keyboard in specs['keyboard'].items():
        if 'ACTUATION' in keyboard['pages']:
            assert pid in actuation, f'Missing actuation descriptor: {pid}'
        if 'TAB_CALIBRATION' in keyboard['pages']:
            assert pid in calibration, f'Missing calibration evidence: {pid}'

    records = []
    for product in registry:
        pid = product['product_id']
        existing = product['adapter_status'] == 'existing_partial_native_adapter'
        family = next((name for name in families if pid in specs[name]), None)
        primary = [n for n in product['navigation'] if n.get('primary')]
        assert len(primary) == 1, (pid, 'primary navigation')
        pages = []
        for item in primary[0]['items']:
            key = item['name']['value']
            route, status, descriptor = None, 'pending', None
            if existing:
                route, status = 'existing_adapter', 'existing_partial_not_reaudited_here'
            elif key == 'HELP':
                help_page = next((p for p in helps.get(pid, {}).get('pages', []) if p['offset'] == item['offset']), None)
                assert help_page, f'Missing mounted Help descriptor: {pid}/{item["offset"]}'
                if help_page:
                    route, status = 'source_help', 'partial_native'
            elif pid == 769 and key == 'HOME':
                route, status = 'hue', 'partial_native'
            elif (pid, key) in dedicated_pages:
                route, status = dedicated_pages[(pid, key)], 'partial_native'
            elif family:
                spec = specs[family][pid]
                if family == 'audio':
                    descriptor = page_map(spec).get(key)
                    supported = has_content(descriptor) or (key == 'TAB_DEMO' and pid in audio_demos)
                elif family == 'accessory_system':
                    supported = key in spec['pages']
                else:
                    supported = key in spec['pages'] and key in branches.get(family, set())
                    # The current pairing branch is an explicit pending panel.
                    if family == 'mouse' and key == 'TAB_PAIRING':
                        supported = False
                if supported:
                    route, status = family, 'partial_native'
                elif family in ('audio', 'accessory_system') or (family == 'keyboard' and key == 'OLED'):
                    descriptor = page_map(controls.get(pid, {})).get(key)
                    if has_content(descriptor):
                        route, status = 'supplement', 'partial_native'
            else:
                descriptor = page_map(controls.get(pid, {})).get(key)
                if has_content(descriptor):
                    route, status = 'source_controls', 'partial_native'
            row = {'page_id': item['key'], 'key': key, 'offset': item['offset'], 'route': route, 'status': status}
            if pid == 3886 and key == 'TAB_CUSTOMIZE':
                row['limitation'] = 'Source ports branch is unreachable (!u.type===BLE_MOBIL); editor fixtures do not count as production content.'
            if route == 'automation':
                row['limitation'] = 'Main page and six action categories are partial; full macro/game/shortcut editors and hardware execution are incomplete.'
            if descriptor and has_content(descriptor):
                row['controls'] = sum(len(s.get('controls', [])) for s in descriptor['sections'])
                row['equalizers'] = sum(bool(s.get('equalizer')) for s in descriptor['sections'])
            pages.append(row)
        records.append({
            'product_id': pid, 'name': product['name'],
            'family': 'existing_adapter' if existing else family or ('hue' if pid == 769 else 'source_controls' if pid in controls else None),
            'status': 'partial_native' if any(p['status'] != 'pending' and p['key'] != 'HELP' for p in pages) else 'pending_main_pages',
            'pages': pages,
            'independent_modes': [{'mode': n.get('display_mode'), 'navigation_offset': n['offset'], 'status': 'not_exposed_by_source_workspace', 'pages': [i['key'] for i in n['items']]} for n in product['navigation'] if not n.get('primary')],
        })
    counts = Counter(p['status'] for r in records for p in r['pages'])
    pending = [(r, p) for r in records for p in r['pages'] if p['status'] == 'pending']
    summary = {
        'registered_product_ids': len(records),
        'products_with_partial_main_content': sum(r['status'] == 'partial_native' for r in records),
        'products_without_main_content': [r['product_id'] for r in records if r['status'] == 'pending_main_pages'],
        'primary_pages': sum(counts.values()), 'page_status_counts': dict(counts),
        'fully_reproduced_products_claimed': 0,
    }
    limitations = [
        'Renderer/descriptor presence is not visual equivalence, complete behavior, or hardware support.',
        'Mouse/keyboard custom mappings, alternate layouts, advanced actions and some conditional interactions remain partial.',
        'Camera preview, enumeration, framing presets, overlays and hardware commands remain incomplete.',
        'Audio demo pages and complex mappings remain incomplete; DSP and haptics are local drafts.',
        'Accessory port discovery, pairing workflows, lighting color parameters and Hue discovery remain incomplete.',
        'Accessory source bodies and preview fixtures do not prove full navigation, profile-menu, animation or modal parity.',
        'System controls do not apply hardware settings or fabricate temperature, fan RPM, SKU or display modes.',
        'Help retains per-page source conditions; unavailable firmware/reset/system services stay unavailable.',
        'Independent displayMode branches are registered as evidence but not automatically exposed by the primary workspace.',
        'No application, build, tests, installer, downloaded JavaScript or DLL was executed for this audit.',
    ]
    result = {'schema_version': 2, 'summary': summary, 'inputs_sha256': inputs,
              'limitations': limitations, 'products': records}
    target = ROOT / 'docs/re/native-product-coverage.json'
    target.write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    lines = ['# 当前原生页面覆盖审计', '',
             '本报告按实际工作区分派和生成数据统计。`partial_native` 只表示已有部分原生内容，不代表界面、交互或硬件行为已完整还原。没有产品被标记为全部完成。', '',
             f"注册 {len(records)} 个产品入口；{summary['products_with_partial_main_content']} 个有部分主页面内容；主导航共 {summary['primary_pages']} 页。", '',
             '| 页面状态 | 数量 |', '| --- | ---: |']
    lines += [f'| {key} | {value} |' for key, value in counts.items()]
    lines += ['', '原有十个适配器单独记为 `existing_partial_not_reaudited_here`，本次统计不替代它们各自的当前源码审计。独立模式另列，未计作主页面实现。', '',
              '## 尚无主页面内容的产品', '', ', '.join(map(str, summary['products_without_main_content'])) or '无；这仍不表示所有产品已经完成。', '',
              '## 完全待接入的主页面', '', '| 产品 ID | 产品 | 页面 |', '| --- | --- | --- |']
    lines += [f"| {r['product_id']} | {r['name'].replace('|', '/')} | {p['key']} |" for r, p in pending]
    lines += ['', '## 仍需完成', ''] + [f'- {item}' for item in limitations]
    lines += ['', '## 不包含在设备页计数中的界面缺口', '',
              '- 托盘：访客/登录账户内容、Widgets/Notifications、多应用入口和动态高度仍未完成；见 [当前托盘审计](tray-current-audit.md)。',
              '- OLED：主页预览卡片、内容编辑器、语言下载及设备传输仍未完成；见 [当前 OLED 审计](keyboard-oled-current-audit.md)。',
              '- 宿主服务：账户登录、独立 Settings 窗口、固件/重置等服务不能由已存在的入口视为完成。',
              '- 跨平台：公共托盘菜单及界面已与 Windows 适配拆分，macOS/Linux 的系统注册适配仍未实现。']
    lines += ['', '逐产品页面身份、实际路由和输入 SHA-256 见 [机器可读记录](native-product-coverage.json)。', '',
              '重新生成：`python -X utf8 tools/audit-native-product-coverage.py`。该命令只解析本地数据和 Rust 源码。', '']
    (ROOT / 'docs/re/native-product-coverage.md').write_text('\n'.join(lines), encoding='utf-8')
    print(json.dumps(summary, ensure_ascii=False))


if __name__ == '__main__':
    main()
