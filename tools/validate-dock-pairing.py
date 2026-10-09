"""Validate current dock source receipts and resources without running the UI."""
import hashlib
import json
import re
from pathlib import Path
import xml.etree.ElementTree as ET

ROOT = Path(__file__).resolve().parents[1]
def read(path):
    return (ROOT / path).read_text(encoding='utf-8')
def digest(path):
    return hashlib.sha256((ROOT / path).read_bytes()).hexdigest()

specs = json.loads(read('crates/razer-pages/src/features/dock_pairing_data.json'))
evidence = json.loads(read('docs/re/dock-pairing-current-evidence.json'))
assert evidence['generator_sha256'] == digest('tools/extract-dock-pairing.cjs')
assert [s['product_id'] for s in specs] == [164, 241]
for spec, product in zip(specs, evidence['products']):
    assert spec['product_id'] == product['product_id']
    assert spec['page'] == ('TAB_CUSTOMIZE' if spec['product_id'] == 164 else 'TAB_PAIRING')
    for receipt in [*product['source_files'], product['label_source'], product['manifest'],
                    *product['locale_receipts'], *product['css']]:
        assert receipt['path'].startswith(f'.ref/devices/{spec["product_id"]}/')
        assert digest(receipt['path']) == receipt['sha256'], receipt['path']
    for component in product['components']:
        # Acorn offsets count UTF-16 units, including astral characters in logs.
        source = read(component['path']).encode('utf-16-le')
        excerpt = source[component['offset'] * 2:component['end'] * 2].decode('utf-16-le')
        excerpt = re.sub(r'data:image/[^"\s]+', '[embedded bitmap omitted; retained in hashed source]', excerpt)
        assert excerpt == component['source'], (spec['product_id'], component['offset'])
    locales = spec['translations']
    assert len(locales) == 10
    for locale, labels in locales.items():
        assert set(labels) == set(locales['en']), (spec['product_id'], locale)
        assert all(isinstance(label, str) for label in labels.values())
    declared = {v.removeprefix('./') for v in json.loads(read(product['manifest']['path']))['files'].values()}
    for asset in spec['assets']:
        assert asset['source'].removeprefix(f'.ref/devices/{spec["product_id"]}/') in declared

assets = json.loads(read('assets/synapse/dock-pairing-manifest.json'))
assert {a['output'] for s in specs for a in s['assets']} == {a['output'] for a in assets}
assert len(assets) == len({a['output'] for a in assets}) == 27
for asset in assets:
    assert digest(asset['source']) == asset['source_sha256']
    assert digest(asset['output']) == asset['output_sha256']
    if asset['output'].endswith('.svg'):
        svg = ET.parse(ROOT / asset['output']).getroot()
        assert svg.tag.endswith('svg') and not any(n.tag.endswith('script') for n in svg.iter())
common = json.loads(read('locales/en.json'))
files = [ROOT / 'crates/razer-pages/src/features/dock_pairing.rs', *sorted((ROOT / 'crates/razer-pages/src/features/dock_pairing').glob('*.rs'))]
for file in files:
    for key in re.findall(r'\.text\("([A-Z_0-9]+)"\)', file.read_text(encoding='utf-8')):
        assert key in common or any(key in s['translations']['en'] for s in specs), (file.name, key)
# The pairing-utility note ships inside each device bundle under that bundle's own
# key (241's current source spells it `ENABLE_LAUNCH_PARING_UTILITY_INFO`), so the
# common locale must not stand in for a missing device string.
for spec in specs:
    key = spec['config'].get('launchUtilityInfoKey')
    assert key, ('launchUtilityInfoKey missing', spec['product_id'])
    for locale, labels in spec['translations'].items():
        assert key in labels, (spec['product_id'], locale, key)
# 设备名链接（源 241 chunk `914.4c13bdac.chunk.js`）：
#   const t = Es(e, o); … t ? <span className={`hyperpolling-span-hover${U}`}
#   onClick={() => z(e)}><u>{name}</u></span> : <span>{name}</span>
# CSS：`.HyperPollingWirelessMouseDock_deviceNameLink__jCiyT{cursor:pointer;
# text-decoration:underline}`、`.body-widgets .widget .hyperpolling-span-hover:hover{color:#44d62c}`。
dock_js = re.sub(r'\s+', '', read('.ref/devices/241/static/js/914.4c13bdac.chunk.js'))
dock_chunk_css = read('.ref/devices/241/static/css/914.530f0373.chunk.css')
dock_main_css = read('.ref/devices/241/static/css/main.1525b0e6.css')
for token in ['Es(e,o)', 'hyperpolling-span-hover${U}', 'onClick:()=>z(e)', '"u"']:
    assert token in dock_js, ('dock link markup drift', token)
assert ('HyperPollingWirelessMouseDock_deviceNameLink__jCiyT{'
        'cursor:pointer;text-decoration:underline}') in dock_chunk_css
assert ('.body-widgets .widget .hyperpolling-span-hover:hover{color:#44d62c}') in dock_main_css
dock_native = read('crates/razer-pages/src/features/dock_pairing.rs')
for token in ['pub(crate) struct DeviceLinkRequested', 'known_devices: Vec<(u32, u32)>',
              'pub(crate) fn set_known_devices', 'dock-device-link-', '.cursor_pointer()',
              '.underline()', 'rgb(0x44d62c)', '.split_once("{{deviceName}}")',
              'WorkspaceEvent::OpenDevice' if 'WorkspaceEvent::OpenDevice' in dock_native else '.emit(DeviceLinkRequested']:
    assert token in dock_native, ('dock link not implemented locally', token)
workspace = read('crates/razer-pages/src/features/workspace.rs')
assert 'OpenDevice{product_id:u32,edition_id:u32,}' in re.sub(r'\s+', '', workspace)
source_workspace = read('crates/razer-pages/src/features/source_workspace.rs')
for token in ['pub(crate) fn set_known_devices', 'DeviceLinkRequested', 'WorkspaceEvent::OpenDevice']:
    assert token in source_workspace, ('dock link not plumbed', token)
product_workspace = read('crates/razer-pages/src/features/product_workspace.rs')
assert 'pub(crate) fn set_known_devices' in product_workspace
shell = read('crates/razer-shell/src/shell.rs')
for token in ['fn sync_known_devices', 'WorkspaceEvent::OpenDevice', 'device.edition_id == *edition_id']:
    assert token in shell, ('shell dock link handling drift', token)
print('Dock pairing: two current sources, 20 locale maps and 27 resources validated.')
