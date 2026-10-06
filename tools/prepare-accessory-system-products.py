"""Prepare native monitor/cooling controls from statically decoded current sources.

Runs maintained local Python only; downloaded JavaScript is never executed.
"""
import copy
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
EVIDENCE = ROOT / 'docs/re/accessory-system-source.json'
source = json.loads(EVIDENCE.read_text(encoding='utf-8'))
products = []
audits = []

for product in source['products']:
    pid = product['product_id']
    for receipt in product['source_files']:
        path = ROOT / receipt['path']
        assert str(path.resolve()).startswith(str((ROOT / '.ref/devices').resolve()))
        assert hashlib.sha256(path.read_bytes()).hexdigest() == receipt['sha256'], path
    main = (ROOT / product['source']).read_text(encoding='utf-8')
    config = product['config']
    spec = {'product_id': pid, 'pages': [], 'initial': {}, 'presets': {}, 'enums': {}}
    if pid in (3858, 3880):
        state = next(s['value'] for s in product['states'] if 'inputSource' in s['value'])
        # Reducer seed is distinct from the DEFAULTPROFILE record. A preset
        # displays presetData; changing one control copies it into customData.
        spec['initial'] = {k: copy.deepcopy(state[k]) for k in (
            'gaming', 'color', 'inputSource', 'hdr', 'adaptiveSync',
            'refeshRateCounter', 'secondDisplay')}
        if pid == 3880:
            spec['initial']['thxCinema'] = state['thxCinema']
        # `uiRestraint` is part of that seed as an empty object: the source's own
        # "no restriction observed" state, which the UI keeps until the device
        # service fills in a reason.
        spec['initial']['uiRestraint'] = copy.deepcopy(state['uiRestraint'])
        assert spec['initial']['uiRestraint'] == {}
        spec['pages'] = ['TAB_GAMING', 'TAB_COLOR', 'TAB_DISPLAY']
        spec['presets'] = config['GAMING_PRESET']
        spec['enums'] = {o['name']: o['value'] for o in product['scalarObjects']
                         if o['name'] in ('Bi', 'GM', 'QB', 'Ss', 'e3', 'zH')}
        assert spec['enums']['zH']['CUSTOM'] == 5
        assert 'useExtendedGammaSlider:!%s' % (0 if pid == 3880 else 1) in main
        # `zrA` (3858) / `iTA` (3880) is the reason paragraph every monitor
        # widget mounts; its default class is `exclamationText mb20`, which the
        # gamut warning overrides to `exclamationText`.
        jsx, loc = ('t6O', 'r6O') if pid == 3858 else ('U6O', 'H6O')
        warn, enums = ('zrA', 'haA') if pid == 3858 else ('iTA', '$aA')
        assert 'e=>{let E=e.text,a=e.className,_=void 0===a?"exclamationText mb20":a;' in main
        assert main.count('jsx)(%s,{' % warn) == (6 if pid == 3858 else 9)
        for snippet in ('children:[(0,%s.jsx)(%s,{text:this.props.disabledReason}),' % (jsx, warn),
                        'children:[(0,%s.jsx)(%s,{text:e.disabledReason}),' % (jsx, warn),
                        'children:[(0,%s.jsx)(%s,{text:a}),' % (jsx, warn),
                        'jsx)(%s,{className:"exclamationText",text:' % warn):
            assert snippet in main, snippet
        # The Color tab's temperature widget: the preset group, then the
        # `.slide-off` group that only carries `.slide-on` for the CUSTOM preset.
        assert 'title:%s.Lyf,tips:%s.iQK,hasSwitch:!1' % (loc, loc) in main
        assert 'className:this.props.disabledReason?"featureDisabled":""' in main
        assert ('selectedPreset===%s.GM.CUSTOM?"".concat(this.displayClasses," slide-on"):this.displayClasses'
                % enums) in main
        assert spec['enums']['GM'] == {'NORMAL': 5, 'LOWBLUELIGHT': 12, 'WARM': 4,
                                       'COOL': 8, 'SRGB': 1, 'CUSTOM': 11}
    elif pid == 3900:
        profile = config['DEFAULTPROFILE']
        spec['initial'] = {k: copy.deepcopy(profile[k]) for k in ('ports', 'portsConfig')}
        assert [p['id'] for p in spec['initial']['ports']] == list(range(26, 34))
        spec['pages'] = ['TAB_PERFORMANCE']
        assert 'fanSpeedValue<25?25:' in main
    elif pid == 3893:
        # Empty ports are the real reducer seed. Requests must not masquerade
        # as a detected pump, detected fan, or a hardware-supplied curve.
        spec['initial'] = {'requestedFanMode': None, 'requestedPumpMode': None}
        spec['pages'] = ['TAB_PERFORMANCE']
        assert any(a['value'] == ['QUIET', 'NORMAL', 'PERFORMANCE', 'ADVANCED']
                   for a in product['arrays'])
    elif pid == 3907:
        state = next(s['value'] for s in product['states'] if 'fanControllerSetting' in s['value'])
        spec['initial'] = {'fanControllerSetting': copy.deepcopy(state['fanControllerSetting'])}
        spec['pages'] = ['TAB_PERFORMANCE']
        for snippet in ('min:500,max:2e3', 'min:1500,max:2500', 'min:1900,max:3200',
                        'dp=2,Np=20,cp=500,up=3200', 'QUIET:3,BALANCED:4,PERFORMANCE:5'):
            assert snippet in main, snippet
    elif pid == 3921:
        fan = json.loads((ROOT / 'src/features/corex_fan_data.json').read_text(encoding='utf8'))
        assert fan['curves'] == config['DEFAULT_CURVES']
        assert fan['source_files'] == product['source_files']
        spec['initial'] = {'fanCurve': fan['initial']}
        spec['presets'] = fan['curves']
        spec['pages'] = ['TAB_CUSTOMIZE']
    else:
        raise ValueError(pid)
    products.append(spec)
    audits.append({'product_id': pid, 'pages': spec['pages'],
                   'source_files': product['source_files'],
                   'hardware_values_fabricated': False,
                   'coverage': 'native local controls; hardware integration and visual parity incomplete'})

assert {p['product_id'] for p in products} == {3858, 3880, 3893, 3900, 3907, 3921}
(ROOT / 'src/features/accessory_system_products_data.json').write_text(
    json.dumps(products, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
(ROOT / 'docs/re/accessory-system-native-audit.json').write_text(json.dumps({
    'schema_version': 1, 'source_evidence_sha256': hashlib.sha256(EVIDENCE.read_bytes()).hexdigest(),
    'products': audits, 'nonlighting_pages': sum(len(p['pages']) for p in products),
    'verification': 'source hashes and static assertions; no app, build, or test execution'
}, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
print(f'Prepared {len(products)} current accessory products / {sum(len(p["pages"]) for p in products)} non-lighting pages; all source hashes verified.')
