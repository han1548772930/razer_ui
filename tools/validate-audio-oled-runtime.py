"""Validate current source receipts, assets and runtime mounting statically."""
import hashlib
import json
import re
from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
def read(name): return json.loads((ROOT / name).read_text(encoding='utf8'))
def sha(data): return hashlib.sha256(data).hexdigest()
evidence = read('docs/re/audio-oled-runtime-current-evidence.json')
data = read('crates/razer-pages/src/features/audio_oled_runtime_data.json')
cache = {}
def verify(row):
    if row['path'] not in cache:
        content = (ROOT / row['path']).read_bytes()
        cache[row['path']] = sha(content), content.decode().encode('utf-16-le')
    digest, content = cache[row['path']]
    assert digest == row['sha256'], row['path']
    assert content[row['offset'] * 2:row['end'] * 2].decode('utf-16-le') == row['source']

assert sha((ROOT / evidence['manifest']['path']).read_bytes()) == evidence['manifest']['sha256']
for row in evidence['receipts']: verify(row)
for row in evidence['css']:
    assert sha((ROOT / row['path']).read_bytes()) == row['sha256']
for row in evidence['icons']:
    verify(row['receipt'])
    icon = (ROOT / 'assets' / row['output']).read_text(encoding='utf8')
    assert icon == row['svg']
    assert ET.fromstring(icon).attrib['viewBox'] == '0 0 20 20'
assert data['connection'] == {'isBle': False, 'isDongle': False}
assert data['language'] == {'value': 0, 'changed': 0} and data['error'] is None
assert data['loading'] == {'target': 'animation', 'type': 'none', 'totalItems': 0, 'currentItem': 0, 'progress': 0}
for locale in ('en', 'zh-CN'):
    translations = read(f'locales/{locale}.json')
    assert all(key in translations for key in data['labels'].values())
    assert '{{currentItem}}' in translations[data['labels']['stg']] and '{{totalItems}}' in translations[data['labels']['stg']]
receipts = {row['symbol']: row['source'] for row in evidence['receipts']}
assert 'oledLanguage:t.payload' in receipts['oledLanguageReducer']
assert 'oledLanguageChanged:e.oledLanguageChanged+1' in receipts['oledLanguageReducer']
assert 'target:"none",type:"none",totalItems:0,currentItem:0,progress:0' in receipts['oledLoadingReducer']
assert 'ON_SET_OLED_DATA_TO_UI' in receipts['GET_OLED_DATA']
assert 'height:"117px",width:"400px",showFooter:!1' in receipts['xx']
assert '255&~t' in receipts['Bv'] and '255&~i' in receipts['Bv']
assert 'wideConfirmBtn:!0' in receipts['ux'] and 'CANCEL_OLED_LANGUAGE_UPDATE' in receipts['ux']
assert 'ON_RESET_OLED' in receipts['vx'] and 'p&&!a?null' in receipts['vx']
assert 'dur:"2s"' in receipts['Qv'] and 'values:"0 50 50;180 50 50;720 50 50"' in receipts['Qv']
assert not any('ble-edit:hover' in row['selector'] for row in evidence['css'])
native = (ROOT / 'crates/razer-pages/src/features/audio_oled_runtime.rs').read_text(encoding='utf8')
home = (ROOT / 'crates/razer-pages/src/features/audio_oled_home.rs').read_text(encoding='utf8')
page = (ROOT / 'crates/razer-pages/src/features/audio_oled.rs').read_text(encoding='utf8')
for marker in ('observe_oled_runtime', 'request_oled_runtime_data', 'state.connection.is_dongle',
               'state.confirm_language_cancel', 'state.reset_error_visible', 'state.progress_motion.set(progress)',
               'changed > 1', 'old_changed', 'decode_language(staged)', 'with_priority(200)',
               'with_priority(100)', 'Duration::from_secs(2)', 'self.finish + 0.001',
               'ON_SET_OLED_LANGUAGE', 'ON_SET_CANCEL_OLED_DOWNLOAD_PROCESS', 'ON_CANCEL_OLED_LANGUAGE_UPDATE',
               'ON_SET_OLED_HOME_SCREEN_DISPLAY_ERROR_RETRY', 'ON_SET_OLED_HOME_SCREEN_DISPLAY_ERROR_REVERT',
               'ON_RESET_OLED'):
    assert marker in native, marker
for marker in ('runtime: runtime::OledRuntimeState', 'is_ble && matches!(mode, 5 | 6)', 'ble_text()', 'with_priority(107)', 'if disabled || muted'):
    assert marker in home, marker
for marker in ('self.oled_runtime_layers(window, cx)', 'staged == raw && staged == selected', 'self.oled_is_ble() || self.oled_is_loading()', 'this.apply_oled_language(window, cx)'):
    assert marker in page, marker
assert 'is_dongle: true' not in native and 'progress: 100.' not in native
assert 'if value != old_value' in native
assert 'let language = selected_language' in native
assert '&& self.page == "TAB_OLED"' in native
assert 'self.draft["device"]["oledLanguage"] = json!(self.oled_language_values().0)' in native
audio = (ROOT / 'crates/razer-pages/src/features/audio_products.rs').read_text(encoding='utf8')
assert 'let staged_oled_language = self.staged.get("/device/oledLanguage").cloned()' in audio
assert 'if self.page == "TAB_OLED" || page == "TAB_OLED"' in audio
assert 'self.request_oled_runtime_data(cx)' in audio
assert 'self.selection_value(control)' in audio
for file, owner in [('source_workspace.rs', 'SourceProductWorkspace'), ('product_workspace.rs', 'ProductWorkspace')]:
    forwarding = (ROOT / 'src/features' / file).read_text(encoding='utf8')
    assert f'impl EventEmitter<super::OledRuntimeRequested> for {owner}' in forwarding
    assert re.search(r'event:\s*&super::OledRuntimeRequested,\s*cx\|\s*\{?\s*cx\.emit\(event\.clone\(\)\)', forwarding)
    assert 'body.observe_oled_runtime(observation, window, cx)' in forwarding
resources = (ROOT / 'crates/razer-assets/src/lib.rs').read_text(encoding='utf8')
assert 'synapse/audio-oled-runtime-warning.svg' in resources
print('1383 OLED runtime validated: current defaults, 46 receipts, CSS/locales/icon, connection/loading/error/language branches and source request bindings.')
