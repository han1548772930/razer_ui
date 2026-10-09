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
    specs = {name: index(f'crates/razer-pages/src/features/{name}_products_data.json') for name in families}
    controls = index('crates/razer-pages/src/features/source_controls_data.json')
    accessories = index('crates/razer-pages/src/features/accessory_controls_data.json')
    assert not controls.keys() & accessories.keys(), 'Ambiguous SourceControls product'
    controls.update(accessories)
    oled = index('crates/razer-pages/src/features/keyboard_oled_data.json')
    assert not controls.keys() & oled.keys(), 'Ambiguous OLED product'
    controls.update(oled)
    helps = index('crates/razer-pages/src/features/source_help_data.json')
    # The renderer requires per-navigation-page metadata. The old links-only
    # schema silently deserialized to an empty list before October 3's fix.
    for pid, help_record in helps.items():
        assert help_record.get('pages'), f'Regenerate Help descriptors: {pid}'
        offsets = [p['offset'] for p in help_record['pages']]
        assert len(offsets) == len(set(offsets)), f'Ambiguous Help page: {pid}'
    branches = {}
    for family in families:
        source = read(f'crates/razer-pages/src/features/{family}_products.rs')
        if 'match self.page.as_str()' in source:
            switch = source.split('match self.page.as_str()', 1)[1]
            branches[family] = set(re.findall(r'"([A-Z_]+)"\s*=>', switch))
    read('crates/razer-pages/src/features/source_workspace.rs')
    read('crates/razer-pages/src/features/product_workspace.rs')
    read('crates/razer-pages/src/features/source_help.rs')
    read('crates/razer-pages/src/features/source_controls.rs')
    read('crates/razer-pages/src/features/source_controls/oled_presets.rs')
    read('crates/razer-pages/src/features/accessory_system_products/corex_fan.rs')
    read('crates/razer-pages/src/features/corex_fan_data.json')
    read('crates/razer-pages/src/features/keyboard_actuation.rs')
    read('crates/razer-pages/src/features/keyboard_calibration.rs')
    calibration = index('crates/razer-pages/src/features/keyboard_calibration_data.json')
    read('crates/razer-pages/src/features/audio_demo.rs')
    audio_demos = index('crates/razer-pages/src/features/audio_demo_data.json')
    hue = load('crates/razer-pages/src/features/hue_data.json')
    assert hue['product_id'] == 769
    for file in ['crates/razer-pages/src/features/hue.rs', 'crates/razer-pages/src/features/hue/onboarding.rs', 'crates/razer-pages/src/features/hue/bridge.rs',
                 'crates/razer-pages/src/features/hue/brightness.rs', 'crates/razer-pages/src/features/hue/effects.rs', 'crates/razer-pages/src/features/hue/preview.rs']:
        read(file)
    dedicated_pages = {}
    for feature in ['dock_pairing', 'wired_argb', 'wireless_argb', 'aether_strip', 'automation']:
        descriptor = load(f'crates/razer-pages/src/features/{feature}_data.json')
        rows = descriptor if isinstance(descriptor, list) else [descriptor]
        read(f'crates/razer-pages/src/features/{feature}.rs')
        for path in sorted((ROOT / f'crates/razer-pages/src/features/{feature}').glob('*.rs')):
            read(path.relative_to(ROOT).as_posix())
        for row in rows:
            dedicated_pages[(row['product_id'], row['page'])] = feature
    for path in sorted((ROOT / 'crates/razer-pages/src/features/source_workspace').glob('*.rs')):
        read(path.relative_to(ROOT).as_posix())
    actuation = index('crates/razer-pages/src/features/keyboard_actuation_data.json')
    for pid, keyboard in specs['keyboard'].items():
        if 'ACTUATION' in keyboard['pages']:
            assert pid in actuation, f'Missing actuation descriptor: {pid}'
        if 'TAB_CALIBRATION' in keyboard['pages']:
            assert pid in calibration, f'Missing calibration evidence: {pid}'

    records = []
    # 十个 `existing_partial_native_adapter` 产品的逐页复核。每一条都指向本地实现
    # 位置，以及该实现依据的当前源码数据（描述符、资源清单或产品包常量）。没有
    # 复核依据的页面继续保持 `existing_partial_not_reaudited_here`，本次统计不把
    # 它们算作已复核。
    # 说法只覆盖已核对的部分：页面路由、可达性与该产品存在的当前源码数据。
    # 页面内部逐字段的数据来源没有逐页核对，因此不写进依据里。
    mats_data = 'crates/razer-catalog/src/lib.rs:10 AUDITED_MOUSE_MAT_IDS、:59 MOUSE_MATS（模块 9228 的方向常量）与 crates/razer-model/src/settings.rs:374 的逐产品效果表'
    legacy_reaudit = {}
    for pid in (3072, 3073, 3074, 3076, 3077, 3078, 3080):
        legacy_reaudit[(pid, 'TAB_LIGHTING')] = (
            'mousemat_lighting: crates/razer-pages/src/features/device_pages.rs:472 lighting_page，'
            f'crates/razer-pages/src/nav.rs:37 只给出 Lighting 页；产品数据见 {mats_data}'
        )
    legacy_reaudit[(182, 'TAB_CUSTOMIZE')] = (
        'device_customize: crates/razer-pages/src/features/customize_page.rs，crates/razer-pages/src/nav.rs:29 可达；'
        '182 有当前源码规格 crates/razer-pages/src/features/mouse_products.rs:128 '
        'source_product（crates/razer-pages/src/features/mouse_products_data.json，含 source_sha256）'
    )
    legacy_reaudit[(182, 'TAB_PERFORMANCE')] = (
        'device_performance: crates/razer-pages/src/features/device_pages.rs:34 performance_page，'
        'crates/razer-pages/src/nav.rs:30 可达；规格文件里存在 DPI/回报率字段（页面内部取值未逐字段核对）'
    )
    legacy_reaudit[(182, 'TAB_POWER')] = (
        'device_power: crates/razer-pages/src/features/device_pages.rs:405 power_page，crates/razer-pages/src/nav.rs:32 可达；'
        '规格文件里存在 power_slider/low_power_slider/low_battery_slider 字段'
    )
    legacy_reaudit[(182, 'TAB_CALIBRATION')] = (
        'device_calibration: crates/razer-pages/src/features/device_pages.rs:189 calibration_page，'
        'crates/razer-pages/src/nav.rs:33 可达；规格文件里存在 calibration/smart_lift_max/smart_landing_max 字段'
    )
    legacy_reaudit[(653, 'TAB_CUSTOMIZE')] = (
        'keyboard_customize: crates/razer-pages/src/nav.rs:35 给出 Customize 页；653 的当前源码布局数据见 '
        'crates/razer-assets/src/lib.rs:176 的 assets/synapse/keyboard-653-layouts.json 与 keyboard-653-deviceconfig.json'
    )
    legacy_reaudit[(653, 'TAB_LIGHTING')] = (
        'device_lighting: crates/razer-pages/src/features/device_pages.rs:472 lighting_page，crates/razer-pages/src/nav.rs:35 可达；'
        '653 的效果表见 crates/razer-model/src/settings.rs:377 与 :380'
    )
    for tab in ('TAB_SOUND', 'TAB_MIC'):
        legacy_reaudit[(777, tab)] = (
            'device_audio: crates/razer-pages/src/features/audio_page.rs 渲染，crates/razer-pages/src/nav.rs:36 给出 Sound/Mic 页；'
            '182/653/777 的注册导航里没有 TAB_AUDIO（crates/razer-catalog/src/registry_data.rs，'
            '由 tools/generate-product-registry.cjs 从各自 bundle 的导航常量生成；'
            '全库只有 3872/3873 声明该页）'
        )
    legacy_reaudit[(777, 'TAB_LIGHTING')] = (
        'device_lighting: crates/razer-pages/src/features/device_pages.rs:472 lighting_page，crates/razer-pages/src/nav.rs:36 可达；'
        '777 的效果表见 crates/razer-model/src/settings.rs:384 与 :387'
    )
    legacy_reaudit[(777, 'TAB_POWER')] = (
        'device_power: crates/razer-pages/src/features/device_pages.rs:405 power_page，crates/razer-pages/src/nav.rs:36 可达'
    )
    legacy_help = ('device_help: crates/razer-pages/src/features/help_page.rs:111 help_page；'
                   'crates/razer-pages/src/features/help_page.rs:21 support_links 对 182/653/777 给出各自 bundle 的 '
                   'DeviceInfo 链接，其余产品走 audited_mouse_mat；十个产品都未进入 source_help 描述符')
    for pid in (182, 653, 777, 3072, 3073, 3074, 3076, 3077, 3078, 3080):
        legacy_reaudit[(pid, 'HELP')] = legacy_help
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
                reaudited = legacy_reaudit.get((pid, key)) if key != 'HELP' else legacy_reaudit.get((pid, 'HELP'))
                if reaudited:
                    route, status = reaudited.split(':', 1)[0], 'partial_native_reaudited'
                else:
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
            if status == 'partial_native_reaudited':
                row['evidence'] = legacy_reaudit[(pid, key)].split(':', 1)[1].strip()
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
            'family': ('existing_adapter_reaudited' if any(p['status'] == 'partial_native_reaudited' for p in pages)
                       else 'existing_adapter') if existing
                      else family or ('hue' if pid == 769 else 'source_controls' if pid in controls else None),
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
        'legacy_pages_reaudited': counts.get('partial_native_reaudited', 0),
        'legacy_pages_without_route': counts.get('existing_partial_not_reaudited_here', 0),
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
        'Legacy adapter pages marked `partial_native_reaudited` have a verified local route and source basis; that is still partial content, not visual parity.',
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
    lines += ['', f"原有十个适配器按页复核：{summary['legacy_pages_reaudited']} 页已确认本地路由与源码依据（`partial_native_reaudited`），{summary['legacy_pages_without_route']} 页仍记为 `existing_partial_not_reaudited_here`。复核结果仍是部分内容，不等于视觉一致。独立模式另列，未计作主页面实现。", '',
              '## 尚无主页面内容的产品', '', ', '.join(map(str, summary['products_without_main_content'])) or '无；这仍不表示所有产品已经完成。', '',
              '## 完全待接入的主页面', '', '| 产品 ID | 产品 | 页面 |', '| --- | --- | --- |']
    lines += [f"| {r['product_id']} | {r['name'].replace('|', '/')} | {p['key']} |" for r, p in pending]
    lines += ['', '## 仍需完成', ''] + [f'- {item}' for item in limitations]
    lines += ['', '## 不包含在设备页计数中的界面缺口', '',
              '- 托盘：账户/通知/Widgets 已有 UI；真实发布者、多应用与动态高度见 [当前托盘契约](tray-ui-current.md)。',
              '- OLED：卡片和编辑器已部分接入；语言下载、设备传输及其余条件见 [当前 OLED 契约](oled-ui-current.md)。',
              '- 宿主服务：独立 Settings 窗口已有本地实现；账户、目录、固件/重置等真实服务与界面完成分开核实。',
              '- 跨平台：公共托盘菜单及界面已与 Windows 适配拆分，macOS/Linux 的系统注册适配仍未实现。']
    lines += ['', '逐产品页面身份、实际路由和输入 SHA-256 见 [机器可读记录](native-product-coverage.json)。', '',
              '重新生成：`python -X utf8 tools/audit-native-product-coverage.py`。该命令只解析本地数据和 Rust 源码。', '']
    (ROOT / 'docs/re/native-product-coverage.md').write_text('\n'.join(lines), encoding='utf-8')
    print(json.dumps(summary, ensure_ascii=False))


if __name__ == '__main__':
    main()
