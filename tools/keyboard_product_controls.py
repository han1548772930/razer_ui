"""Derive controls from mounted current components and literal reducer defaults.

No downloaded code is evaluated. Missing receipts remain unsupported, even when
the product configuration happens to contain a field with a similar name.
"""
import hashlib
import re
from pathlib import Path


def controls_for(raw, pages, root):
    receipts = next((p['pages'] for p in pages['products'] if p['product_id'] == raw['product_id']), [])
    mounted = {page['key']: '\n'.join(c['source'] for c in page['components']) for page in receipts}
    sources = []
    seen = set()
    for evidence in raw['source_files'] + [dict(path=page['path'], sha256=page['sha256']) for page in receipts]:
        if evidence['path'] in seen:
            continue
        seen.add(evidence['path'])
        data = (root / evidence['path']).read_bytes()
        assert hashlib.sha256(data).hexdigest() == evidence['sha256'], evidence['path']
        sources.append(data.decode('utf8'))
    source = '\n'.join(sources)
    power = mounted.get('TAB_POWER', '')
    customize = mounted.get('TAB_CUSTOMIZE', '')
    defaults = {}

    def reducer(field):
        matches = set(re.findall(re.escape(field) + r':\{isEnabled:!([01]),value:(\d+)\}', source))
        existing = raw['config']['DEFAULTPROFILE'].get(field)
        if existing:
            return existing
        assert len(matches) == 1, (raw['product_id'], field, matches)
        enabled, value = next(iter(matches))
        return dict(isEnabled=enabled == '0', value=int(value))

    result = dict(power_kind=None, dim_kind=None, power_bounds=None,
                  gaming_mode='.props.setGameMode(' in customize,
                  polling='pollingRate' in customize and 'POLLING_RATE' in customize)
    if 'dimKeyboardLightingReducer.dimKeyboardLighting' in power:
        result['dim_kind'] = 'choices'
        defaults['dimKeyboardLighting'] = reducer('dimKeyboardLighting')
        assert raw['config'].get('DIM_KEYBOARD_LIGHTING_VALUES')
    elif 'dimLightingReducer.dimLighting' in power:
        assert 'min:1,max:15,step:1' in power
        result['dim_kind'] = 'slider'
        defaults['dimLighting'] = reducer('dimLighting')
    if 'KEYBOARD_WIRELESS_POWER_SAVING_VALUES' in power:
        assert raw['config'].get('KEYBOARD_WIRELESS_POWER_SAVING_VALUES')
        result['power_kind'] = 'choices'
        defaults['powerSaving'] = reducer('powerSaving')
    elif 'mousePowerSavingReducer.powerSavingValue' in power:
        assert 'min:1,max:15,step:1' in power
        values = set(re.findall(r'powerSavingValue:(\d+)', source))
        assert len(values) == 1
        defaults['powerSavingValue'] = int(next(iter(values)))
        result.update(power_kind='mouse_slider', power_bounds=[1, 15, 1])
    elif 'powerReducer.powerSaving' in power:
        # The actual page passes no override; obtain bounds from that mounted
        # class's defaultProps, rather than the similarly named audio control.
        component = next(c for page in receipts if page['key'] == 'TAB_POWER'
                         for c in page['components'] if 'this.props.setPowerSaving' in c['source'])
        match = re.search(re.escape(component['symbol']) + r'\.defaultProps=\{min:(\d+),minTag:"\d+",max:(\d+),maxTag:"\d+",step:(\d+)\}', source)
        assert match, (raw['product_id'], component['symbol'])
        result.update(power_kind='slider', power_bounds=list(map(int, match.groups())))
        defaults['powerSaving'] = reducer('powerSaving')
    result['defaults'] = defaults
    return result
