"""Validate Hue source receipts, resources and literal contracts. No UI/test execution."""
import hashlib
import json
import re
from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
digest = lambda path: hashlib.sha256((ROOT / path).read_bytes()).hexdigest()
spec = json.loads((ROOT / 'src/features/hue_data.json').read_text(encoding='utf-8'))
evidence = json.loads((ROOT / 'docs/re/hue-current-evidence.json').read_text(encoding='utf-8'))
assert evidence['generator_sha256'] == digest('tools/extract-hue.cjs')
for record in [evidence['source'], evidence['css'], evidence['manifest']]:
    assert digest(record['path']) == record['sha256'], record['path']
source = (ROOT / evidence['source']['path']).read_text(encoding='utf-8')
for receipt in [evidence['dictionaries'], evidence['archetypes'], evidence['palette'], evidence['brightness'],
                *evidence['defaults'], *evidence['constants'], *evidence['lighting'],
                *evidence['effect_components'], *evidence['common_labels']]:
    assert digest(receipt['path']) == receipt['sha256']
    assert source[receipt['offset']:receipt['end']] == receipt['source']
assert [e['id'] for e in spec['lighting']['effects']] == [11, 12, 2, 3, 1]
assert spec['initial']['brightness'] == {'isEnabled':False,'value':0}
assert spec['initial']['hue']['isPaired'] is False
assert len(spec['archetypes']) == len({a['name'] for a in spec['archetypes']}) == 48

common = json.loads((ROOT / 'locales/en.json').read_text(encoding='utf-8'))
custom = spec['translations']['en']
for path in [ROOT / 'src/features/hue.rs', *sorted((ROOT / 'src/features/hue').glob('*.rs'))]:
    if path.name == 'tests.rs':
        continue
    content = path.read_text(encoding='utf-8')
    keys = re.findall(r'(?:text|i18n::t)\("([A-Z_0-9]+)"\)',content)
    for key in keys:
        assert key in common or key in custom, (path.name,key)
for locale, labels in spec['translations'].items():
    assert all(isinstance(value,str) for value in labels.values()), locale
    assert set(labels) == set(custom), locale

manifest = json.loads((ROOT / 'assets/synapse/hue-manifest.json').read_text(encoding='utf-8'))
outputs = {entry['output'] for entry in manifest}
assert len(outputs) == len(manifest) == 63
for entry in manifest:
    assert digest(entry['source']) == entry['source_sha256']
    assert digest(entry['output']) == entry['output_sha256']
    if entry['output'].endswith('.svg'):
        svg = ET.parse(ROOT / entry['output']).getroot()
        assert svg.tag.endswith('svg')
        assert not any(node.tag.endswith('script') for node in svg.iter())
for path in [ROOT / 'src/features/hue.rs', *sorted((ROOT / 'src/features/hue').glob('*.rs'))]:
    for asset in re.findall(r'"(synapse/hue-[a-zA-Z0-9_-]+\.(?:svg|png))"',path.read_text(encoding='utf-8')):
        assert 'assets/'+asset in outputs, (path.name,asset)
assert 'assets/synapse/hue-indicator_animated.svg' in outputs
# 亮度滑条：当前 769 的两个亮度控件都是共享 `OT`，参数
# `min:0 max:100 step:1 minTag:w.KFn maxTag:w.zrT`（`KFn`/`zrT` 依导出表为
# `OFF`/`BRIGHT`），没有 `noTip`；本地绘制交给共享 `SourceSlider` 并补 `.foot` 两端标签。
css = (ROOT / evidence['css']['path']).read_text(encoding='utf-8')
for declaration in (
    '.slider-container{forced-color-adjust:none;height:64px;opacity:.3;pointer-events:none;'
    'position:relative;transition:opacity .3s;will-change:opacity}',
    '.slider-container.on{opacity:1;pointer-events:auto}',
    '.slider-container .foot{bottom:-2px;opacity:1;position:absolute;text-transform:uppercase;',
    '.foot.min{left:0}',
    '.foot.max{right:0}',
    '.slider-tip{background-color:#44d62c;border-radius:3px;bottom:42px;',
):
    assert declaration in css, declaration
assert re.search(r'\.slider\{-webkit-appearance:none;background:#0000;border-radius:3px;bottom:25px;'
                 r'height:6px;[^}]*width:100%;z-index:3\}', css)
assert re.search(r'\.slider::-webkit-slider-thumb\{[^}]*background:#44d62c;[^}]*height:16px;'
                 r'[^}]*width:16px', css)
for match in re.finditer(r'jsx\)\(OT,\{min:0,max:100,step:1', source):
    window = source[match.start():match.start() + 400]
    assert 'minTag:w.KFn' in window and 'maxTag:w.zrT' in window, window[:200]
assert source.count('minTag:w.KFn') == 2 and source.count('maxTag:w.zrT') == 2
brightness = (ROOT / 'src/features/hue/brightness.rs').read_text(encoding='utf-8')
for marker in (
    'SourceSlider::new(slider, progress)',
    '.tip(Some(format!("{value:.0}")))',
    '.enabled(enabled)',
    '.bottom(surface::css(-2.))',
    '.justify_between()',
    'i18n::t("OFF").to_uppercase()',
    'i18n::t("BRIGHT").to_uppercase()',
):
    assert marker in brightness, marker
assert 'Slider::new(slider)' not in brightness, 'the plain Kit slider must be gone'
# 高级灯效教程点：源 `PA` 把「已关闭」写进 localStorage 的 `isShowTutorialHue`
# （`k.A.set(u_, !1)`；`u_="isShowTutorialHue"`），可见条件是
# `!1===get(u_) ? 隐藏 : isLoading ? 隐藏 : 显示`。本地用同一键名做跨启动持久化。
assert 'u_="isShowTutorialHue"' in source
assert '!1===k.A.get(u_)?o(!1):e||o(!0)' in source.replace(' ', '')
assert 'k.A.set(u_,!1)' in source.replace(' ', '')
assert 'this.get=(e,E)=>{const _=window.localStorage.getItem(e);' in source
tutorial = (ROOT / 'src/features/hue/effects.rs').read_text(encoding='utf-8')
for marker in (
    'const TUTORIAL_STORAGE_KEY: &str = "isShowTutorialHue";',
    'pub(super) fn load_tutorial_visibility() -> bool',
    'fn save_tutorial_visibility(visible: bool)',
    'save_tutorial_visibility(false);',
    'pub(super) fn sync_tutorial_visibility(&mut self)',
    'self.bridge.is_paired && !self.bridge.is_loading && load_tutorial_visibility()',
):
    assert marker in tutorial, marker
for path in ('src/features/hue.rs', 'src/features/hue/bridge.rs', 'src/features/hue/preview.rs'):
    assert 'sync_tutorial_visibility()' in (ROOT / path).read_text(encoding='utf-8'), path
# 配对中（`PAIRING`）的进度动画：源 `fi()` 渲染 `.Home_progressWrapper` +
# `.Home_progress` + `.Home_child`（`animation:Home_move__oy9kP 2s linear infinite`，
# `0%{left:-80px}to{left:100%}`）与 `.Home_close`（`left:100%;margin-left:10px`）。
for declaration in (
    '.Home_wrapper__cLM2H .Home_progressWrapper__VQhXO{align-items:center;display:flex;height:20px;'
    'justify-content:center;margin:10px auto 0;position:relative;width:300px}',
    '.Home_wrapper__cLM2H .Home_progressWrapper__VQhXO .Home_progress__VIf8x{background-color:#44d62c4d;'
    'border-radius:2.5px;height:5px;overflow:hidden;position:relative;width:100%}',
    '.Home_wrapper__cLM2H .Home_progressWrapper__VQhXO .Home_progress__VIf8x .Home_child__8waW\\+'
    '{animation:Home_move__oy9kP 2s linear infinite;background-color:#44d62c;border-radius:2.5px;'
    'height:5px;position:absolute;width:80px}',
    '@keyframes Home_move__oy9kP{0%{left:-80px}to{left:100%}}',
    '.Home_wrapper__cLM2H .Home_progressWrapper__VQhXO .Home_close__cc3go{background-image:url('
    '../../static/media/icon_close_enclosed_1.d8da72c9.svg);height:20px;left:100%;margin-left:10px;'
    'position:absolute;width:20px}',
):
    assert declaration in css, declaration
compact = re.sub(r'\s+', '', source)
assert 'caseC_:return(0,q.jsx)(fi,{})' in compact
assert 'functionfi(){' in compact
pairing = source[source.index('function fi(){'):]
pairing = pairing[:pairing.index('function vi(')]
for fragment in ('className:Di', 'className:ci', 'className:ui', 'className:Li'):
    assert fragment in re.sub(r'\s+', '', pairing), fragment
assert 'Mi(e,A_)' in re.sub(r'\s+', '', pairing)
onboarding = (ROOT / 'src/features/hue/onboarding.rs').read_text(encoding='utf-8')
for marker in (
    '"hue-pairing-progress"',
    'Animation::new(Duration::from_secs(2)).repeat()',
    'phase * 1.2667 - 0.2667',
    'relative(-0.2667)',
    'cx.reduce_motion()',
    '.w(surface::css(80.))',
    'hue-icon_close_enclosed_1_hover.svg',
):
    assert marker in onboarding, marker
# 扫描的 13 秒超时：源里两个「开始扫描」按钮都 `Mi(e,I_)` 后
# `mi=setTimeout(()=>Ui(e),13e3)`，`Ui=e=>Mi(e,R_)`（SCAN_FAILED）；`hi()` 卸载时
# `clearTimeout(mi)`；`SCANNING_IP` 的手动搜索不启动它。
assert compact.count('mi=setTimeout(()=>Ui(e),13e3)') == 2
assert 'letmi=null;constUi=e=>{Mi(e,R_)}' in compact
assert 'Mi=(e,E,_)=>{e({type:h_,payload:{status:E,ip:_}})}' in compact
assert '(()=>()=>{mi&&(clearTimeout(mi),mi=null)},[])' in compact
assert 'Mi(e,S_,a)' in compact and 'Mi(e,S_),mi=setTimeout' not in compact
assert compact.count('()=>Ui(e)') == 2  # 两个「开始扫描」按钮的 setTimeout 回调
hue_rs = (ROOT / 'src/features/hue.rs').read_text(encoding='utf-8')
assert 'scan_task: Option<Task<()>>' in hue_rs
assert 'if next != Integration::Scanning {' in hue_rs
onboarding = (ROOT / 'src/features/hue/onboarding.rs').read_text(encoding='utf-8')
for marker in (
    'fn arm_scan_timeout(&mut self, cx: &mut Context<Self>)',
    'Duration::from_secs(13)',
    'this.integration == Integration::Scanning',
    '"ON_SET_INTEGRATION_STATUS: SCAN_FAILED"',
):
    assert marker in onboarding, marker
assert onboarding.count('this.arm_scan_timeout(cx);') == 2
print('Hue: current receipts, 10 locale maps, 5 quick effects, 63 resources, the shared '
      'brightness slider (`.slider-container`/`.foot`), the `isShowTutorialHue` tutorial '
      'persistence, the pairing progress animation and the 13s scan timeout validated.')
