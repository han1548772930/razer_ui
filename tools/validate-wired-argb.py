"""Validate ARGB source receipts and resources. Does not run the application."""
import hashlib
import json
import re
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
load = lambda path: json.loads((ROOT / path).read_text(encoding='utf-8'))
digest = lambda path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest()
specs = load('crates/razer-pages/src/features/wired_argb_data.json')
evidence = load('docs/re/wired-argb-current-evidence.json')
manifest = load('assets/synapse/wired-argb-manifest.json')
assert evidence['generator_sha256'] == digest('tools/extract-wired-argb.cjs')
assert [spec['product_id'] for spec in specs] == [778, 3871]
text_cache = {}

def source_text(path):
    if path not in text_cache:
        text_cache[path] = (ROOT / path).read_text(encoding='utf-8').encode('utf-16-le')
    return text_cache[path]

def validate_receipts(value):
    if isinstance(value, dict):
        if {'path', 'sha256'} <= value.keys():
            assert value['path'].startswith('.ref/devices/'), value['path']
            assert digest(value['path']) == value['sha256'], value['path']
        if {'path', 'offset', 'end', 'source'} <= value.keys():
            original = source_text(value['path'])[2 * value['offset']:2 * value['end']].decode('utf-16-le')
            assert original == value['source'], (value['path'], value['offset'])
        for child in value.values():
            validate_receipts(child)
    elif isinstance(value, list):
        for child in value:
            validate_receipts(child)

validate_receipts(evidence)
outputs = {record['output'] for record in manifest}
assert len(outputs) == len(manifest), 'Duplicate prepared outputs'
for spec, product in zip(specs, evidence['products']):
    assert spec['minimum_leds'] == 1
    assert spec['fan_counts'] == [6, 8, 9, 10, 12, 15, 16, 18, 20, 22, 24, 25, 32, 40]
    assert spec['page'] == 'TAB_CUSTOMIZE'
    assert len(spec['translations']) == 10
    assert {'en', 'zh-CN', 'zh-TW'} <= spec['translations'].keys()
    assert all('GLITTER_NO_OF_LED' in locale and 'TEXT_LED_STRIP' in locale for locale in spec['translations'].values())
    assert product['state_scope']['owner'] == 'device/localstorage'
    assert 'ports' not in product['state_scope']['default_profile']['source']
    assert product['profile_bar']['sync_icon_enabled_page'] == 'TAB_LIGHTING'
    consumers = '\n'.join(receipt['source'] for receipt in product['profile_bar']['consumers'])
    assert 'this.props.enableSwitchProfile' in consumers
    assert 'this.enableSwitchProfile()?"":"disabled"' in consumers
    assert 'loader disable' in consumers
    for asset in spec['assets']:
        assert asset['output'] in outputs
        assert digest(asset['context']) == asset['context_sha256']
    if spec['product_id'] == 778:
        assert spec['mainboard_ports'] == [2147483651, 2147483652, 2147483656]
        assert {'product-0', 'product-128', 'product-129'} <= {asset['name'] for asset in spec['assets']}

for record in manifest:
    assert (ROOT / record['source']).resolve().is_relative_to((ROOT / '.ref/devices').resolve())
    assert (ROOT / record['output']).resolve().is_relative_to((ROOT / 'assets/synapse').resolve())
    assert digest(record['source']) == record['source_sha256'], record['source']
    assert digest(record['output']) == record['output_sha256'], record['output']
    if record['output'].endswith('.svg'):
        svg = ET.fromstring((ROOT / record['output']).read_bytes())
        assert svg.tag.endswith('svg')
        assert not any(node.tag.endswith('script') for node in svg.iter())
        assert not any(key.lower().startswith('on') for node in svg.iter() for key in node.attrib)

for frame in range(120):
    assert f'assets/synapse/wired-argb-3871-detecting-ring-{frame:03}.svg' in outputs
for side in ('left', 'right'):
    for number in range(1, 10):
        assert f'assets/synapse/wired-argb-3871-detecting-led-{side}{number}.svg' in outputs
print(f'Validated 2 current ARGB pages, 20 locale dictionaries, source receipts and {len(manifest)} resources; application not executed.')

# 图标提示：源 3871 的 `#icon-detection-wrapper` / `#icon-refreshing-wrapper` 用
# `onMouseEnter`/`onMouseLeave` 直接翻转 `isMounted`（`Gu` 组件没有展示延迟：挂载后只用一个
# 0ms 定时器加 `.show`，由 `.tooltip-razer>.main{transition:opacity .1s linear}` 完成 100ms
# 淡入），位置 `bottom-left`（贴下沿、右缘对齐）。本地改用共享的
# `crate::ui::hover_tip::source_hover_tip`，因此这里同时钉住源与本地的写法。
for record in evidence['products']:
    compact = re.sub(
        r'\s+', '', (ROOT / record['source_files'][0]['path']).read_text(encoding='utf8'))
    css_path = record['css'] if isinstance(record['css'], str) else record['css']['path']
    css_text = (ROOT / css_path).read_text(encoding='utf8')
    assert '.tooltip-razer>.main{bottom:100%' in css_text, record['product_id']
    assert 'transition:opacity .1s linear' in css_text, record['product_id']
    if record['product_id'] == 3871:
        for fragment in (
            'id:"icon-detection-wrapper",className:"icon-detection-wrapper",'
            'onMouseEnter:()=>this.toggleTooltipDetection(!0)',
            'onMouseLeave:()=>this.toggleTooltipDetection(!1)',
            'isMounted:this.state.toggleTooltipDetection,position:"bottom-left",'
            'target:"icon-detection-wrapper"',
            'id:"icon-refreshing-wrapper",className:"icon-refreshing-wrapper",'
            'onMouseEnter:()=>this.toggleTooltipRefresh(!0)',
            'isMounted:this.state.toggleTooltipRefresh,position:"bottom-left",'
            'target:"icon-refreshing-wrapper"',
            'e._timeout=setTimeout(()=>{e._isMounted&&e.setState({showClassName:"show"})})',
            'zA="bottom-right",kA="bottom-left",xA="bottom-left-edge"',
            'E===kA?{x:o.left+320-o.width,y:o.top',
        ):
            assert fragment in compact, fragment
    else:
        # 778 是主板布局，源里没有这两个图标与其提示。
        assert 'icon-detection-wrapper' not in compact, record['product_id']
shared = (ROOT / 'crates/razer-widgets/src/hover_tip.rs').read_text(encoding='utf8')
for marker in (
    'pub(crate) enum SourceTipPlacement {',
    'BottomLeft,',
    'BottomRight,',
    'Bottom,',
    'Top,',
    'pub(crate) fn source_hover_tip(',
    '.w(surface::css(300.))',
    'tip.justify_end().right_0()',
    'tip.justify_start().left_0()',
    'tip.top_full().mt(surface::css(5.))',
    'tip.bottom_full().mb(surface::css(5.))',
    'Animation::new(Duration::from_millis(100))',
    'TooltipColors::border()',
):
    assert marker in shared, marker
native = (ROOT / 'crates/razer-pages/src/features/wired_argb.rs').read_text(encoding='utf8')
for marker in (
    'enum WiredIcon {',
    'hovered_icon: Option<WiredIcon>,',
    '.id("icon-detection-wrapper")',
    '.id("icon-refreshing-wrapper")',
    'this.hovered_icon = hovered.then_some(WiredIcon::Detection);',
    'this.hovered_icon = hovered.then_some(WiredIcon::Refresh);',
    'SourceTipPlacement::BottomLeft',
    'source_hover_tip(',
    '.when(!self.spec.mainboard()',
):
    assert marker in native, marker
assert '.tooltip(self.spec.text("GLITTER_MESSAGE_AUTO_DETECTION"))' not in native
assert '.tooltip(self.spec.text("GLITTER_MESSAGE_REFRESH_ICON"))' not in native
print('Validated the shared `.tooltip-razer` hover tips and the 3871 icon wrappers '
      '(778 has no such icons in source).')

# LED 数量提示：源两个产品都是 `Gu position:"bottom-right"`，内容由
# `getTextItem(<labels>.vml, {ledCount: '<span style="color:#44d62c">N</span>'})` 生成
# （只有数字是主题绿）。本地改用共享提示并保留该分段上色。
for record in evidence['products']:
    compact = re.sub(
        r'\s+', '', (ROOT / record['source_files'][0]['path']).read_text(encoding='utf8'))
    for fragment in (
        ',{position:"bottom-right",children:',
        'dangerouslySetInnerHTML:{__html:',
        'ledCount:\'<spanstyle="color:#44d62c">\'',
    ):
        assert fragment in compact, (record['product_id'], fragment)
port = (ROOT / 'crates/razer-pages/src/features/wired_argb/port.rs').read_text(encoding='utf8')
for marker in (
    'hovered_info: bool,',
    'hovered_info: false,',
    '.id("argb-detection-info-wrapper")',
    'this.hovered_info = *hovered;',
    'source_hover_tip_element(',
    'SourceTipPlacement::BottomRight',
    'template.split_once("{{ledCount}}")',
    'div().text_color(cx.theme().primary).child(count)',
):
    assert marker in port, marker
assert '.tooltip(text)' not in port
print('Validated the `.tooltip-razer.bottom-right` LED-count tooltip (source split colouring '
      'and native hover mounting).')

# 端口帮助控件：源把 `.help`/`.tip` 放在 `.port-container.widget` 里
# （`.widget .help{…14px;position:absolute;right:10px;top:10px}`），本地改用共享的
# `surface::help_control`；产品自己的 `tooltip_questionmark` 与共享图标字节相同。
for record in evidence['products']:
    compact = re.sub(
        r'\s+', '', (ROOT / record['source_files'][0]['path']).read_text(encoding='utf8'))
    for fragment in (
        'className:"port-containerwidget",children:[',
        'jsx)("div",{className:"help"})',
        'jsx)("div",{className:"tip",children:',
    ):
        assert fragment in compact, (record['product_id'], fragment)
port = (ROOT / 'crates/razer-pages/src/features/wired_argb/port.rs').read_text(encoding='utf8')
for marker in ('surface::help_control(', '.absolute()', '.right(surface::css(10.))',
               '.top(surface::css(10.))'):
    assert marker in port, marker
assert '.tooltip(self.spec.text("GLITTER_TIP_HELP_PORT_MESSAGE"))' not in port
manifest = load('assets/synapse/wired-argb-manifest.json')
entries = manifest if isinstance(manifest, list) else manifest.get('entries', [])
icons = {entry['output']: entry['output_sha256'] for entry in entries
         if 'tooltip_questionmark' in entry.get('output', '')}
assert len(icons) == 2, icons
shared_icon = hashlib.sha256(
    (ROOT / 'assets/synapse/automation-tooltip_questionmark.svg').read_bytes()).hexdigest()
assert set(icons.values()) == {shared_icon}, icons
print('Validated the port `.widget .help` control against the source markup, the shared '
      '`surface::help_control` tip and byte-identical question-mark artwork.')
