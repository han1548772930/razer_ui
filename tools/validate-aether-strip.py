"""Static receipt, resource and source-boundary checks; no app or tests run."""
import hashlib
import json
import re
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
read = lambda path: (ROOT / path).read_text(encoding='utf-8')
digest = lambda data: hashlib.sha256(data).hexdigest()
spec = json.loads(read('crates/razer-pages/src/features/aether_strip_data.json'))
bundle = read('.ref/devices/784/static/js/main.3094caa4.js')
tip_module_src = read('crates/razer-pages/src/features/aether_strip/source_tip.rs')
evidence = json.loads(read('docs/re/aether-strip-current-evidence.json'))
assets = json.loads(read('assets/synapse/aether-strip-manifest.json'))
assert spec['product_id'] == 784 and spec['page'] == 'CUSTOMIZED'
assert spec['initial']['detectedLedCount'] == 0 and spec['initial']['bendData'] == []
assert spec['initial']['isRefreshing'] is False
assert len(spec['translations']) == 10
for locale, labels in spec['translations'].items():
    assert all(isinstance(labels[key], str) and labels[key] for key in spec['labels'].values()), locale
assert digest((ROOT / 'tools/extract-aether-strip.cjs').read_bytes()) == evidence['generator_sha256']
for source in evidence['source_files']:
    assert source['path'].startswith('.ref/devices/784/')
    assert digest((ROOT / source['path']).read_bytes()) == source['sha256'], source['path']
source_path = evidence['source_files'][0]['path']
source = read(source_path)
utf16 = source.encode('utf-16-le')
fragment = lambda start, end: utf16[start * 2:end * 2].decode('utf-16-le')
for contract in evidence['contracts']:
    assert contract in source, contract
for item in evidence['components'] + evidence['header']['fragments']:
    assert fragment(item['offset'], item['end']) == item['source'], item.get('symbol', item.get('method'))
for receipt in evidence['translationReceipts']:
    assert digest(fragment(receipt['offset'], receipt['end']).encode('utf-8')) == receipt['sha256']
assert evidence['header']['mounted_on_all_pages'] is True
assert evidence['header']['sync_disabled_pages'] == ['HELP', 'CUSTOMIZED']
assert evidence['header']['dropdown_disabled_pages'] == ['CUSTOMIZED']
assert len(assets) == 23
for asset in assets:
    source_file, output = ROOT / asset['source'], ROOT / asset['output']
    assert source_file.resolve().is_relative_to((ROOT / '.ref/devices/784').resolve())
    assert output.resolve().is_relative_to((ROOT / 'assets/synapse').resolve())
    assert digest(source_file.read_bytes()) == asset['source_sha256'], asset['source']
    assert digest(output.read_bytes()) == asset['output_sha256'], asset['output']
    if output.suffix == '.svg':
        svg = ET.fromstring(output.read_bytes())
        assert svg.tag.endswith('svg')
        assert not any(el.tag.endswith('script') or any(k.lower().startswith('on') for k in el.attrib) for el in svg.iter())
    if 'fragment_sha256' in asset:
        assert digest(fragment(asset['offset'], asset['end']).encode('utf-8')) == asset['fragment_sha256']
for svg in evidence['assets']:
    parsed = ET.fromstring(svg['svg'])
    assert parsed.get('viewBox'), svg['name']
native = read('crates/razer-pages/src/features/aether_strip.rs')
state = read('crates/razer-pages/src/features/aether_strip/state.rs')
preview = read('crates/razer-pages/src/features/aether_strip/preview.rs')
snapshot = native[native.index('pub(crate) fn snapshot'):native.index('pub(crate) fn restore')]
assert 'bendData' in snapshot
assert not any(key in snapshot for key in ['detected', 'online', 'locked', 'power_on', 'synapse_override', 'last_request'])
assert 'Observation::default()' in native
assert 'SourceProductWorkspace::new' in preview and 'aether_preview_page' in preview
assert 'self.workspace.clone()' in preview
assert 'checked_add' in state and 'array.len() > 4' in state
assert 'stripeLedNumber' in state and 'pollTime' in state and 'sleepInterval' in state
assert 'SourceSpinner' in read('crates/razer-pages/src/features/aether_strip/presentation.rs')
for path in [ROOT / 'crates/razer-pages/src/features/aether_strip.rs', *sorted((ROOT / 'crates/razer-pages/src/features/aether_strip').glob('*.rs'))]:
    assert not re.search(r'\b(?:rgb|rgba|hsla)\(', path.read_text(encoding='utf-8')), path
# 轮播：`.carousel--item` 的盒模型、`.device` 的选中/未选中位移，以及选中项
# 靠 `scrollLeft = carousel.scrollWidth / 5 * index` 的平滑居中（CSS
# `scroll-behavior:smooth`）。表达式在 7 个当前 IOT 包（780-784、790、791）里出现。
css = next((ROOT / '.ref/devices/784/static/css').glob('main.*.css')).read_text(
    encoding='utf-8', errors='replace')
for declaration in (
    '.carousel--inner{align-items:flex-end;display:flex;margin:0 auto;overflow:hidden;'
    'padding:0 496px;scroll-behavior:smooth}',
    '.carousel--inner.center{overflow:visible;padding:initial;width:-webkit-fit-content;'
    'width:fit-content}',
    '.carousel--item .device{opacity:.5}',
    '.carousel--item .device:not(.active){margin-top:75px}',
    '.carousel--item .device.active{opacity:1;padding-top:28px}',
    '.carousel--item .device.active .device--img{display:block;height:auto;'
    'transform:translateY(3px);width:237px}',
):
    assert declaration in css, declaration
item = re.search(r'\.carousel--item\{[^}]*\}', css).group(0)
for field in ('height:212px', 'padding:0 30px', 'justify-content:center', 'width:248px',
              'flex:none', 'position:relative'):
    assert field in item, (field, item)
bundle = read('.ref/devices/784/static/js/main.3094caa4.js')
assert re.search(r'scrollLeft=\w+\.current\.scrollWidth/5\*', bundle)
centering = sum(
    1 for directory in sorted((ROOT / '.ref/devices').iterdir()) if directory.is_dir()
    for path in sorted((directory / 'static/js').glob('*.js'))
    if re.search(r'scrollLeft=\w+\.current\.scrollWidth/5\*', path.read_text(encoding='utf-8', errors='replace'))
)
assert centering == 7, centering
device = read('crates/razer-pages/src/features/aether_strip/device.rs')
for marker in ('justify_center()', '.px(surface::css(30.))', 'let mut device = v_flex()',
               '.opacity(0.5).mt(surface::css(75.))', '.pt(surface::css(28.))',
               'card.child(device)', 'if selected { 237. } else { 154.8 }',
               # `.carousel--inner` 的容器与 `H()` 居中：多设备时 496px 内边距 +
               # 无滚动条的可滚动行 + 偏移量 `scrollWidth / 5 * index`；单设备走
               # `.carousel--inner.center`（居中、无内边距、不滚动）。
               '.px(surface::css(496.))', '.scrollbar_width(px(0.))',
               '.track_scroll(&self.carousel_scroll)',
               'self.carousel_scroll.bounds().size.width / 5. * index as f32',
               'self.carousel_scroll.set_offset(point(target, px(0.)))'):
    assert marker in device, marker
assert 'scrollable_both()' not in device, 'the carousel must not draw a scrollbar'
aether_view = read('crates/razer-pages/src/features/aether_strip.rs')
assert 'carousel_scroll: ScrollHandle,' in aether_view
assert 'carousel_scroll: ScrollHandle::default()' in aether_view
# 编号指示器：`.indicator--list` 药丸容器 + `.indicator--item` 的配色，以及源码给
# `[tooltip]` 的定位覆盖（`right:auto;top:calc(100% + 10px);width:fit-content`）。
for declaration in (
    '.indicator--container{align-items:center;display:flex;justify-content:center;margin-top:10px;'
    'position:relative;z-index:1}',
    '.indicator--list{background:#111;border:1px solid #ccc;border-radius:40px;display:flex;'
    'margin-right:10px;padding:5px}',
    '.indicator--item.active,.indicator--item:hover{background-color:#44d62c;color:#111}',
    '.indicator--item.busy,.indicator--item.offline{color:#fd8611}',
    '.indicator--item.active.busy,.indicator--item.active.busy:hover,.indicator--item.active.offline,'
    '.indicator--item.active.offline:hover,.indicator--item.busy:hover,.indicator--item.offline:hover'
    '{background-color:#fd8611;color:#111}',
    '.indicator--item[tooltip]:before{right:auto;top:calc(100% + 10px);width:-webkit-fit-content;'
    'width:fit-content}',
):
    assert declaration in css, declaration
indicator = re.search(r'\.indicator--item\{[^}]*\}', css).group(0)
for field in ('border-radius:30px', 'color:#ccc', 'font-size:14px', 'height:26px', 'margin-right:10px',
              'position:relative', 'width:26px'):
    assert field in indicator, (field, indicator)
for marker in ('.rounded(surface::css(40.))', '.mr(surface::css(10.))', '.p(surface::css(5.))',
               '.border_color(cx.theme().foreground)', '.size(surface::css(26.))',
               '.rounded(surface::css(30.))', "ElementId::from(\"aether-device-indicator\")"):
    assert marker in device, marker
# 徽标可见性矩阵：`.device--badge{visibility:hidden}` 之上按选中/离线/忙碌/悬停分别放开，
# 忙碌徽标还有 42×27 居中与 `busy-btn`/`busy-btn-white` 两种皮肤。
for declaration in (
    '.carousel--item .device--badge{align-items:center;box-sizing:initial;display:flex;'
    'justify-content:center;position:absolute;visibility:hidden}',
    '.carousel--item .device.active .device--cta-power{bottom:54%;left:6%;visibility:visible}',
    '.carousel--item .device.active .device--cta-find{bottom:37%;left:6%;visibility:visible}',
    '.carousel--item .device.active:hover .device--cta-del{visibility:visible}',
    '.carousel--item .device.active.offline .device--cta-del{visibility:visible}',
    '.carousel--item .device.active.offline .device--cta-find,'
    '.carousel--item .device.active.offline .device--cta-power{visibility:hidden}',
    '.carousel--item .device.active.linked .device--cta-find,'
    '.carousel--item .device.active.linked .device--cta-power{visibility:hidden}',
    '.carousel--item .device.active.busy .device--cta-del,'
    '.carousel--item .device.active.busy .device--cta-find,'
    '.carousel--item .device.active.busy .device--cta-power{visibility:visible}',
    '.carousel--item .device.active.busy .device--cta-enable:hover{background-color:#707070}',
    '.carousel--item .device.offline .device--badge-offline{visibility:visible}',
    '.carousel--item .device.busy .device--badge-offline{visibility:hidden}',
    '.carousel--item .device.active.offline .device--badge-offline{height:24px;'
    'visibility:visible;width:24px}',
):
    assert declaration in css, declaration
for declaration in (
    '.carousel--item .device.busy .device--cta-enable{',
    'background-size:16px 14px;border:1px solid hsla(0,0%,44%,.302);border-radius:3px;height:27px;',
    'visibility:visible;width:42px;z-index:2}',
    '.device.active.busy .device--cta-enable{background-image:url(../../static/media/'
    'busy-btn-white.453a134e.svg);border-color:#fff;left:50%;top:calc(50% - 8px)',
):
    assert declaration in css, declaration
for marker in (
    'let busy = locked;',
    'let power_find = selected && (busy || !offline);',
    'let remove = selected && (offline || busy);',
    '.group(card_group.clone())',
    'group_hover(card_group.clone(), |v| v.visible())',
    'if offline && !busy {',
    '"busy-btn"',
    'with_hover_bg(Colors::control_border())',
    'if active { 84.5 } else { 82.5 }',
    'if selected { 25. } else { 62. }',
    'if selected { 72. } else { 84.8 }',
):
    assert marker in device, marker
# 仍未实现：`.device.linked` 的可见性没有本地数据（Observation 无 linked），
# `hasPowerButton` 同理。
assert '.device.active.linked' not in device, 'linked visibility is not modelled locally yet'
# 卡片徽标的 `[tooltip]`：轮播里默认 `display:none`，只有选中卡片上的三种锚点在
# `:hover` 时显示；本地按同一锚点实现（`SourceTipItem` + `TipAnchor`）。
for declaration in (
    '.carousel--item .device--badge[tooltip]:before{content:attr(tooltip);display:none;height:auto;'
    'left:50%;width:-webkit-fit-content;width:fit-content;z-index:3}',
    '.carousel--item .device.active .device--badge.anchor--left[tooltip]:before{content:attr(tooltip);'
    'left:50%;width:-webkit-fit-content;width:fit-content}',
    '.carousel--item .device.active .device--badge.anchor--middle[tooltip]:hover:before{'
    'content:attr(tooltip);left:50%;transform:translateX(-50%);white-space:break-spaces;'
    'width:-webkit-fit-content;width:fit-content;width:200px}',
):
    assert declaration in css, declaration
assert '[tooltip]:before{background-color:#000' in css
# 当前 784 页面**不会**进入「链接」形态：轮播组件 `Gl` 从调用点只拿到
# `devicesData`/`changeLinked`/`selectedDeviceIndex`，`supportsLinked` 始终 undefined，
# `hasPowerButton` 取默认 true；卡片 `<ol>` 也没有 `isLinked` 属性，所以
# `.button--cta-link`、`.indicator.linked` 与 `.device.linked` 的规则在本产品里不可达。
carousel_calls = re.findall(r'\(Gl,\{.{0,400}?\}\)', bundle, re.S)
assert len(carousel_calls) == 2, len(carousel_calls)
for call in carousel_calls:
    assert 'supportsLinked' not in call and 'hasPowerButton' not in call, call[:120]
assert 'supportsLinked:i' in bundle or 'supportsLinked:_' in bundle or 'supportsLinked:' in bundle
assert ('const Gl=e=>{let E=e.devicesData,a=e.changeLinked,t=e.selectedDeviceIndex,_=e.supportsLinked,'
        'i=e.hasPowerButton,o=void 0===i||i,n=e.iotProps;') in bundle
assert re.search(r'\(ol,\{id:\w+\.id,containerId:\w+\.deviceContainerId,title:\w+\.title,'
                 r'isOnline:\w+\.isOnline,isLocked:\w+\.isLocked,isSingle:1===\w+\.length,'
                 r'isActive:\w+===\w+,', bundle)
card_call = re.search(r'\(ol,\{id:\w+\.id,containerId:.*?hasPowerButton:\w+\}', bundle, re.S).group(0)
assert 'isLinked' not in card_call, card_call[:160]
# 图标按钮的提示框：源码里没有 identify/refresh 的专用覆盖规则，走全局 `[tooltip]:before`
# （`right:0;top:calc(100% + 5px);width:auto;white-space:nowrap`）——本地原先的
# `trigger.origin + (2, 34)` 没有源码依据，已删除。
assert 'right:0;text-align:left;top:calc(100% + 5px)' in css
assert 'trigger.origin + point(scale * 2., scale * 34.)' not in tip_module_src
assert 'Kind::Icon' not in tip_module_src
assert 'anchor: TipAnchor::Right,' in tip_module_src
assert 'gap: 5.,' in tip_module_src
tip_module = read('crates/razer-pages/src/features/aether_strip/source_tip.rs')
for marker in (
    'pub(super) enum TipAnchor {',
    'Self::Start => trigger.left(),',
    'Self::Half => trigger.left() + trigger.size.width / 2.',
    'Self::Right => trigger.right() - width',
    'Self::Center => trigger.left() + (trigger.size.width - width) / 2.',
    'Badge { anchor, gap } => point(',
    'trigger.bottom() + scale * gap',
    'gap: 5.',
    '.whitespace_nowrap()',
    'tooltip-opacity',
):
    assert marker in tip_module, marker
# `.anchor--middle` 的 200px 与 `.device--badge[tooltip]` 的 5px 间距。
assert '200. / 300.' in tip_module
for marker in ('TipAnchor::Half,', 'TipAnchor::Right,', 'TipAnchor::Center,'):
    assert marker in device, marker
assert device.count('SourceTipItem::new(') == 4, device.count('SourceTipItem::new(')
# 编号项 `.indicator--item[tooltip]:before{right:auto;top:calc(100% + 10px);width:fit-content}`
# ——本地用同一提示层、`TipAnchor::Start`、间距 10px，Kit tooltip 已移除。
assert '.indicator--item[tooltip]:before{right:auto;top:calc(100% + 10px)' in css
for marker in (
    'Self::Start => trigger.left(),',
    'pub(super) struct SourceTipWrap {',
    'TipAnchor::Start,',
):
    assert (marker in tip_module) or (marker in device), marker
assert '10.,' in device
assert 'tooltip::Tooltip::new(name.clone())' not in device
# `.button--cta-link`、`.indicator.linked` 与 `.device.linked` 的规则在当前 784 页面不可达
# （`supportsLinked` 恒为 undefined、`hasPowerButton` 恒为默认 true、卡片没有 `isLinked`），
# 上面已用源事实断言；因此本地不实现它们是忠实于当前源，而不是缺 UI。
assert '.indicator--container .button--cta{background-position:50%' in css
# 源码的 `scroll-behavior:smooth` 没有声明时长（UA 决定），本地因此按同一偏移量直接定位，
# 不自造缓动或时长；这一点写在审计文档里。
print('Aether Strip: current source receipts, 10 locales, 23 resources, mounted header policy, '
      'profile/observation separation, the carousel item/device box model, the source scroll '
      'centering offset, the indicator list/配色, both `[tooltip]` placements (badges with the '
      'three anchors, indicator items with `right:auto` + 10px) and the badge visibility matrix '
      'validated statically; the link/unlink button, `.indicator.linked` and `.device.linked` are '
      'proven unreachable in the current 784 page and the icon tooltips no longer use an invented '
      'offset.')
