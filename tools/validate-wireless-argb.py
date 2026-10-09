"""Validate source/data/artwork receipts statically; no app or vendor code runs."""
import base64
import hashlib
import json
import re
import struct
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
load = lambda p: json.loads((ROOT / p).read_text(encoding='utf8'))
digest = lambda b: hashlib.sha256(b).hexdigest()
specs = load('crates/razer-pages/src/features/wireless_argb_data.json')
audit = load('docs/re/wireless-argb-current-evidence.json')
assets = load('assets/synapse/wireless-argb-manifest.json')
assert audit['generator_sha256'] == digest((ROOT / 'tools/extract-wireless-argb.cjs').read_bytes())
assert [s['product_id'] for s in specs] == [3884, 3886]
assert len(assets) == 54
for spec, evidence in zip(specs, audit['products']):
    pid = spec['product_id']
    assert spec['page'] == 'TAB_CUSTOMIZE'
    assert len(spec['translations']) == 10
    assert spec['fan_values'] == [6, 8, 9, 10, 12, 15, 16, 18, 20, 22, 24, 25, 32, 40]
    for receipt in evidence['source_files'] + evidence['css'] + [evidence['manifest']]:
        assert digest((ROOT / receipt['path']).read_bytes()) == receipt['sha256'], receipt['path']
    bundle = (ROOT / evidence['source_files'][0]['path']).read_text(encoding='utf8')
    # Acorn offsets count UTF-16 units, so slice with the same coordinate system.
    utf16 = bundle.encode('utf-16-le')
    for component in evidence['components']:
        assert utf16[component['offset']*2:component['end']*2].decode('utf-16-le') == component['source']
    for key in spec['labels'].values():
        assert key in spec['translations']['en'], (pid, key)
    for asset in spec['assets']:
        receipt = next(r for r in assets if r['output'] == asset['output'])
        source_path = asset['bundle'] if 'data' in asset else asset['source']
        assert receipt['source'] == source_path
        assert (ROOT / source_path).resolve().is_relative_to((ROOT / f'.ref/devices/{pid}').resolve())
        assert digest((ROOT / source_path).read_bytes()) == receipt['source_sha256']
        content = base64.b64decode(asset['data'].split(',', 1)[1], validate=True) if 'data' in asset else (ROOT / asset['source']).read_bytes()
        if 'data' in asset:
            assert receipt['bundle'] == asset['bundle']
            assert digest(content) == receipt['embedded_sha256']
        else:
            assert digest(content) == receipt['source_sha256']
        output = (ROOT / asset['output']).read_bytes()
        assert digest(output) == receipt['output_sha256']
        if asset['output'].endswith('.svg'):
            assert output == content
            svg = ET.fromstring(output)
            assert svg.tag.endswith('svg') and not any(n.tag.endswith('script') for n in svg.iter())
            assert not re.search(r'(?:href|src)\s*=\s*[\"\'](?:https?:|javascript:)', output.decode())
        else:
            assert output.startswith(b'\x89PNG\r\n\x1a\n')
            width, height = struct.unpack('>II', output[16:24])
            assert width > 0 and height > 0
    if pid == 3884:
        assert spec['metadata']['minimumLedValue'] == 1
        assert 'return E>240' in bundle
    else:
        assert spec['metadata']['allowedLedsPerPort'] == 120
        assert '!r&&i&&!u.type===k_.BLE_MOBIL&&this.renderPorts(a)' in bundle
        assert 'Math.max(n,40)' in bundle
print('Validated both wireless ARGB products, 10 locales each, source offsets and 30 artwork receipts.')

# Derived interaction layers. The extracted stylesheet recolors one layer per
# icon from `:hover`/`:active` and animates the auto-detection glyph; GPUI runs
# neither, so the prepare tool splits those layers out with `currentColor` paint
# and the native page supplies the audited colors and keyframe timing. Every
# layer's geometry must still equal the audited source element.
LAYERS = {
    'auto_off-glyph': ('auto_off', 'detect-b'),
    'auto_off-green': ('auto_off', 'detect-c'),
    'auto_off-gray': ('auto_off', 'detect-f'),
    'auto_on-glyph': ('auto_on', 'detect-b'),
    'auto_on-ring': ('auto_on', 'detect-e'),
    'refresh-hover': ('refresh', 'painted'),
    'remove-hover': ('remove', 'painted'),
    'warning-hover': ('warning', 'painted'),
    'power-hover': ('power', 'rect'),
    'power_off-hover': ('power_off', 'rect'),
}
def element(svg, key):
    """`painted` is the single filled path, `rect` the power button's rect, and
    anything else is the element carrying that audited class."""
    if key == 'painted':
        return next(n for n in svg.iter() if n.tag.endswith('path') and n.get('fill') not in (None, 'none'))
    if key == 'rect':
        return next(n for n in svg.iter() if n.tag.endswith('rect'))
    return next(n for n in svg.iter() if n.get('class') == key)
for spec in specs:
    pid = spec['product_id']
    source_assets = {a['output']: a for a in spec['assets']}
    def extracted(name):
        asset = source_assets[f'assets/synapse/wireless-argb-{pid}-{name}.svg']
        return ET.fromstring(base64.b64decode(asset['data'].split(',', 1)[1], validate=True))
    for name, (key, selector) in LAYERS.items():
        receipt = next(r for r in assets if r['output'] == f'assets/synapse/wireless-argb-{pid}-{name}.svg')
        assert digest((ROOT / receipt['output']).read_bytes()) == receipt['output_sha256'], (pid, name)
        layer = ET.fromstring((ROOT / receipt['output']).read_bytes())
        assert not any(n.tag.endswith('script') for n in layer.iter())
        painted = next(n for n in layer.iter() if n.tag.endswith(('path', 'rect')))
        origin = element(extracted(key), selector)
        for attribute in ('d', 'width', 'height', 'x', 'y', 'rx'):
            assert painted.get(attribute) == origin.get(attribute), (pid, name, attribute)
        if name.endswith('-hover'):
            assert 'currentColor' in (painted.get('fill'), painted.get('stroke')), (pid, name)
    for name in ('power-base', 'power_off-base'):
        receipt = next(r for r in assets if r['output'] == f'assets/synapse/wireless-argb-{pid}-{name}.svg')
        base = ET.fromstring((ROOT / receipt['output']).read_bytes())
        assert not any(n.tag.endswith('rect') for n in base.iter()), (pid, name)
    css = next((ROOT / f'.ref/devices/{pid}/static/css').glob('main.*.css')).read_text(encoding='utf-8')
    theme = (ROOT / 'crates/razer-pages/src/features/wireless_argb/theme.rs').read_text(encoding='utf-8')
    native = (ROOT / 'crates/razer-pages/src/features/wireless_argb.rs').read_text(encoding='utf-8')
    for declaration, color in (
        ('.icon-detection:hover .detect-b{fill:#44d62c}', '0x44d62c'),
        ('.icon-detection:active .detect-b{fill:#39a029}', '0x39a029'),
        ('.icon-detection--active:hover .detect-a{stroke:#96ef89}', '0x96ef89'),
        ('.icon-detection--active:active .detect-a{stroke:#39a029}', '0x39a029'),
        ('.icon-refreshing:hover path{fill:#44d62c}', '0x44d62c'),
        ('.icon-refreshing:active path{fill:#39a029}', '0x39a029'),
        ('.icon-power:hover rect{fill:#7de36c}', '0x7de36c'),
        ('.icon-power--power-off:hover rect{fill:#d97077}', '0xd97077'),
        ('.icon-power--power-off rect{fill:#c8323c}', '0xc8323c'),
        ('.icon-warning:hover path{fill:#feab59}', '0xfeab59'),
        ('.icon-warning:active path{fill:#b15e0c}', '0xb15e0c'),
        ('.icon-close-glitter:hover path{fill:#c8323c}', '0xc8323c'),
    ):
        assert declaration in css, (pid, declaration)
        assert color in theme, (pid, color)
    # Keyframe numbers and durations the native interaction reproduces.
    for keyframe in ('scale(.8)', 'scale(.4)', 'scale(.2)', 'scale(1.8)'):
        assert keyframe in css, (pid, keyframe)
    for expression, duration in (
        ('1.8 - 0.8 * ease_out(phase)', '100'),
        ('0.4 + 1.6 * eased', '700'),
        ('0.2 + 1.2 * eased', '700'),
        ('0.8 + 0.2 * ease_out(phase)', '50'),
    ):
        assert expression in native, (pid, expression)
        assert f'Duration::from_millis({duration})' in native, (pid, duration)
    # `.port-item` reveals the close glyph only while its own row is hovered; the
    # source hides it with `display:none`, so it also occupies no space.
    for declaration in (
        '.port-item .icon-close-glitter{display:none;margin-left:-10px}',
        '.port-item:hover .icon-close-glitter{display:inline-block}',
        '.port-item:hover .icon-close-glitter:hover path{fill:#c8323c}',
    ):
        assert declaration in css, (pid, declaration)
    for marker in ('hovered_segment', '.ml(surface::css(-10.))',
                   'ix > 0 && self.hovered_segment == Some((id, sid))'):
        assert marker in native, (pid, marker)
    # `.port-add-bend`: the gray 27px inline row. 3884 draws `.underline`'s 1px
    # `:after` rule that follows the text to `#44d62c`; 3886 declares no
    # `.underline` rules and underlines the row itself.
    assert ('#multipleBrightness .port-container .port-items-led-image .port-add-bend{'
            'align-items:center;color:#707070;cursor:pointer;display:inline-flex;'
            'height:27px;margin-bottom:5px;margin-top:5px') in css, pid
    if pid == 3884:
        for declaration in (
            '.port-add-bend .underline{position:relative}',
            '.port-add-bend .underline:hover{color:#44d62c}',
            '.port-add-bend .underline:hover:after{background-color:#44d62c}',
            '.port-add-bend .underline:after{background-color:#707070;bottom:.6px;'
            'content:"";height:1px;left:0;position:absolute;width:100%}',
        ):
            assert declaration in css, (pid, declaration)
    else:
        assert 'margin-top:5px;text-decoration:underline}' in css, pid
        assert '.port-add-bend .underline' not in css, pid
    assert 'fn add_bend()' in theme and '0x707070' in theme, pid
    for marker in ('Colors::add_bend()', 'h(surface::css(27.))', '.my(surface::css(5.))',
                   '.border_b_1()', '.group_hover(', 'div().underline().child(label)',
                   'self.spec.product_id == 3884'):
        assert marker in native, (pid, marker)
print('Validated 24 derived interaction layers against the source hover, active and keyframe receipts.')
print('Validated the per-row `.icon-close-glitter` hover reveal on both products.')
print('Validated `.port-add-bend` against both products\' own underline declarations.')

# 检测/刷新图标的提示机制（源 `Gu`）。`onMouseEnter`/`onMouseLeave` 直接翻转 `isMounted`，
# 没有任何展示延迟（`showTooltip` 挂载后只用一个 0ms 定时器加 `.show`，交给
# `.tooltip-razer .main{transition:opacity .1s linear}` 淡入）；位置 `bottom-left` 由
# `Uu(target,"bottom-left")` = `target.left+320-target.width` 加内层
# `.main{right:0;top:100%;margin-top:5px}` 得到（右缘对齐目标、位于下方 5px）。
# 3884 是现代箭头函数转译，3886 是同一组件的 ES5 形式，因此分开断言。
modern = [
    'onMouseEnter:()=>this.toggleTooltipDetection(!0)',
    'onMouseLeave:()=>this.toggleTooltipDetection(!1)',
    'onMouseEnter:()=>this.toggleTooltipRefresh(!0)',
    'onMouseLeave:()=>this.toggleTooltipRefresh(!1)',
    'jsx)(Gu,{isMounted:this.state.toggleTooltipDetection,position:"bottom-left",'
    'target:"icon-detection-wrapper"',
    'jsx)(Gu,{isMounted:this.state.toggleTooltipRefresh,position:"bottom-left",'
    'target:"icon-refreshing-wrapper"',
    'this.showTooltip=()=>{conste=this;clearTimeout(e._timeout),e.setState({shouldRender:!0}',
    'e._timeout=setTimeout(()=>{e._isMounted&&e.setState({showClassName:"show"})})',
    'this.hideTooltip=()=>{conste=this;clearTimeout(e._timeout),e.setState({showClassName:""}',
    'pu="bottom-right",Mu="bottom-left",mu="bottom-left-edge"',
    'E===Mu?{x:o.left+320-o.width,y:o.top',
]
legacy = [
    'onMouseEnter:function(){returne.toggleTooltipDetection(!0)}',
    'onMouseLeave:function(){returne.toggleTooltipDetection(!1)}',
    'onMouseEnter:function(){returne.toggleTooltipRefresh(!0)}',
    'onMouseLeave:function(){returne.toggleTooltipRefresh(!1)}',
    'isMounted:this.state.toggleTooltipDetection,position:"bottom-left",'
    'target:"icon-detection-wrapper"',
    'isMounted:this.state.toggleTooltipRefresh,position:"bottom-left",'
    'target:"icon-refreshing-wrapper"',
    'showTooltip=function(){vare=(0,nf.Z)(r);clearTimeout(e._timeout),e.setState({shouldRender:!0}',
    'e._isMounted&&e.setState({showClassName:"show"})',
    'hideTooltip=function(){vare=(0,nf.Z)(r);clearTimeout(e._timeout),e.setState({showClassName:""}',
]
for evidence in audit['products']:
    source_path = ROOT / evidence['source_files'][0]['path']
    compact = re.sub(r'\s+', '', source_path.read_text(encoding='utf8'))
    expected = modern if evidence['product_id'] == 3884 else legacy
    for fragment in expected:
        assert fragment in compact, (evidence['product_id'], fragment)
print('Validated the instant `Gu` hover tooltips (no show delay, `bottom-left`, 100ms fade) '
      'for 3884 and its ES5 form for 3886.')

# 本地的即时提示实现：两个带 id 的包装元素用 `on_hover` 直接翻转状态（源 `isMounted`），
# 提示用 `.tooltip-razer` 的皮肤与 100ms 淡入，位置是 `top:100% + right:0 + 5px`。
native = (ROOT / 'crates/razer-pages/src/features/wireless_argb.rs').read_text(encoding='utf8')
for marker in (
    'enum ArgbIcon {',
    'hovered_icon: Option<ArgbIcon>,',
    '.id("icon-detection-wrapper")',
    '.id("icon-refreshing-wrapper")',
    'this.hovered_icon = hovered.then_some(ArgbIcon::Detection);',
    'this.hovered_icon = hovered.then_some(ArgbIcon::Refresh);',
    'source_hover_tip(',
    'SourceTipPlacement::BottomLeft',
    'use gpui_kit::StatefulInteractiveElement as _;',
):
    assert marker in native, marker
# 提示本体与几何集中在共享的 `crate::ui::hover_tip`（`.tooltip-razer` 的 300px 主框、
# 皮肤、100ms 淡入，以及 bottom-left 的 `top:100% + right:0 + 5px`）。
shared = (ROOT / 'crates/razer-widgets/src/hover_tip.rs').read_text(encoding='utf8')
for marker in (
    'pub(crate) fn source_hover_tip(',
    '.w(surface::css(300.))',
    'tip.justify_end().right_0()',
    'tip.top_full().mt(surface::css(5.))',
    'TooltipColors::border()',
    'TooltipColors::background()',
    'TooltipColors::foreground()',
    'Animation::new(Duration::from_millis(100))',
):
    assert marker in shared, marker
# 两个图标不再走 Kit 的 `Button::tooltip`（它只有 Top/Bottom/Left/Right，且带自己的展示延迟）。
assert '.tooltip(self.spec.text("GLITTER_MESSAGE_AUTO_DETECTION"))' not in native
assert '.tooltip(self.spec.text("GLITTER_MESSAGE_REFRESH_ICON"))' not in native
print('Validated the native instant icon tooltips (ids, hover state, `.tooltip-razer` skin, '
      '100ms fade, bottom-left placement) in `crates/razer-pages/src/features/wireless_argb.rs`.')

# LED 数量提示：源两个产品都是 `Gu position:"bottom-right"`，内容由
# `getTextItem(OT.vml, {ledCount: '<span style="color:#44d62c">N</span>'})` 生成。
for record in audit['products']:
    compact = re.sub(
        r'\s+', '', (ROOT / record['source_files'][0]['path']).read_text(encoding='utf8'))
    for fragment in (
        ',{position:"bottom-right",children:',
        'dangerouslySetInnerHTML:{__html:',
        'ledCount:\'<spanstyle="color:#44d62c">\'',
    ):
        assert fragment in compact, (record['product_id'], fragment)
for marker in (
    'hovered_detected: Option<u32>,',
    'hovered_detected: None,',
    '.id(("argb-detected-wrapper", id))',
    'this.hovered_detected = hovered.then_some(id);',
    'source_hover_tip_element(',
    'SourceTipPlacement::BottomRight',
    'template.split_once("{{ledCount}}")',
    'div().text_color(cx.theme().primary).child(count)',
):
    assert marker in native, marker
assert '.replace("{{ledCount}}", &detected.to_string()),' not in native
print('Validated the native `.tooltip-razer.bottom-right` LED-count tooltip (per-port hover '
      'mounting and the source\'s green count span).')

# 端口帮助控件：源把 `.help`/`.tip` 直接放在 `.port-container.widget` 里
# （`.widget .help{…14px;position:absolute;right:10px;top:10px}`），本地改用共享的
# `surface::help_control`（同一 `.widget .tip` 机制）。
for record in audit['products']:
    compact = re.sub(
        r'\s+', '', (ROOT / record['source_files'][0]['path']).read_text(encoding='utf8'))
    for fragment in (
        'className:"port-containerwidget",children:[',
        'jsx)("div",{className:"help"})',
        'jsx)("div",{className:"tip",children:',
    ):
        assert fragment in compact, (record['product_id'], fragment)
for marker in ('surface::help_control(', '.absolute()', '.right(surface::css(10.))',
               '.top(surface::css(10.))'):
    assert marker in native, marker
assert '.tooltip(self.spec.text("GLITTER_TIP_HELP_PORT_MESSAGE"))' not in native
assert 'synapse/help-default.svg' not in native
print('Validated the port `.widget .help` control against the source markup and the shared '
      '`surface::help_control` tip.')
