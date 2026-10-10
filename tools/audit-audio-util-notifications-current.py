"""Verify current AudioEnumerator notification IDA bodies and JS consumers.

Reads retained static analysis and original PE bytes only. Never runs IDA,
target DLLs, applications, vendor JS, or notification registration.
"""
import argparse
import hashlib
import json
from pathlib import Path
import struct

import pefile

ROOT = Path(__file__).resolve().parents[1]
SHA = '9134a79a2aac0d3ca48087fe2ad62ab29b44ad334c9d85edf36171c7ea79caaf'
BINARY = f'.ref/native-libraries/{SHA}/RzAudioUtil_v1.0.3.1.dll'
OUTPUT = 'docs/re/audio-util-notifications-current-evidence.json'
RECEIPT = 'docs/re/evidence/audio-util-notification-ida.json'


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--acquire-receipt', action='store_true',
                        help='Copy existing private IDA output into maintained evidence; no target execution')
    args = parser.parse_args()
    raw = (ROOT / BINARY).read_bytes()
    assert sha(raw) == SHA
    pe = pefile.PE(data=raw)
    if args.acquire_receipt:
        analyses = [json.loads((ROOT / '.work/ida-static/audio-util-notification' / name).read_text('utf-8'))
                    for name in ['analysis.json', 'helpers.json', 'callbacks.json', 'bridge.json']]
        result = {k: v for k, v in analyses[0].items() if k != 'functions'}
        result['functions'] = sorted([f for a in analyses for f in a['functions']], key=lambda f: f['rva'])
        result['source_path'] = BINARY
        result['exporter_sha256'] = sha((ROOT / 'tools/ida-static-functions.py').read_bytes())
        (ROOT / RECEIPT).parent.mkdir(parents=True, exist_ok=True)
        (ROOT / RECEIPT).write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', 'utf-8')
    analysis = json.loads((ROOT / RECEIPT).read_text('utf-8'))
    assert analysis['input_sha256'] == SHA and analysis['hexrays_available']
    functions = {f['rva']: f for f in analysis['functions']}
    for f in functions.values():
        assert sha(pe.get_data(f['rva'], f['end_rva'] - f['rva'])) == f['code_sha256'], hex(f['rva'])
    def gate(rva, *tokens):
        code = functions[rva]['pseudocode']
        for token in tokens:
            assert token in code, (hex(rva), token)
    gate(0x2bf0, '"cmdData"', '"enable"', 'sub_180044B00', 'sub_180003950', '"response"', '"enabled"')
    gate(0x3950, 'sub_180003AE0', 'sub_180044C00', 'a1[2] == v13')
    gate(0x44b00, 'CoCreateInstance', '0x17u', '+ 48i64', '+ 56i64', '*(_QWORD *)(v3 + 8) = a2')
    gate(0x44c00, '+ 56i64', '+ 16i64', '*(_QWORD *)(v1 + 8) = 0i64')
    gate(0x3bd0, 'if ( a2 < 2 || a2 == 3 )', 'v7 += 64i64')
    gate(0x3360, '"RzAudioUtilEvent"', '"AudioEnumerator_DeviceChange"', '"endpointId"', 'sub_18001ABB0')
    gate(0x1abb0, 'qword_1800BB430', 'sub_18001BA20', '"NotifyFFIEvent"')
    for rva in [0x459b0, 0x45a60, 0x45b10, 0x45bc0]:
        gate(rva, 'a2 && lstrlenW(a2) > 0')
    def table(address, count):
        return [value - pe.OPTIONAL_HEADER.ImageBase
                for value in struct.unpack('<' + 'Q' * count, pe.get_data(address, count * 8))]
    assert table(0x99450, 3)[2] == 0x6fc0
    assert pe.get_data(0x6fc0, 9) == bytes.fromhex('4883c108e927bcffff')
    assert table(0x99510, 2) == [0x3bd0, 0x38b0]
    assert table(0x9d070, 8) == [0x467e0, 0x3cbe0, 0x467a0, 0x46f00, 0x46ec0, 0x46ee0, 0x46ea0, 0x46f20]
    active_path = 'docs/re/audio-util-active-consumers-current.json'
    active_raw = (ROOT / active_path).read_bytes()
    active = json.loads(active_raw)
    assert active['automatic_notification_products'] == [1422, 1446]
    for product in active['products']:
        for item in product['registrations'] + product['receipts']:
            assert sha((ROOT / item['path']).read_bytes()) == item['sha256']
    shell = (ROOT / 'crates/razer-shell/src/shell/audio_notifications.rs').read_text('utf-8')
    assert 'matches!(product_id, 1422 | 1446)' in shell
    assert 'Session::retire' in shell and 'audio_notification_cleanup' in shell
    consumers = []
    for product in active['automatic_notification_products']:
        count = 0
        for path in sorted((ROOT / f'.ref/middleware/{product}').glob('*.js')):
            data = path.read_bytes()
            text = data.decode('utf-8')
            for token in ['dispatch("AudioEnumerator","EnableNotification",{enable:e})',
                          'enableAudioDeviceChangeNotification(!0)',
                          's.response("RzAudioUtilEvent"',
                          'this.emit(e.eventType,e)',
                          'this.audioUtil.on("AudioEnumerator_deviceChange"']:
                start = 0
                while (offset := text.find(token, start)) >= 0:
                    consumers.append({'product_id': product, 'path': path.relative_to(ROOT).as_posix(),
                                      'sha256': sha(data), 'offset_unit': 'Python Unicode code points',
                                      'offset': offset, 'token': token,
                                      'snippet': text[max(0, offset - 150):offset + len(token) + 260]})
                    count += 1
                    start = offset + len(token)
        assert count >= 4, product
    evidence = {
        'scope': 'RzAudioUtil 1.0.3.1 AudioEnumerator EnableNotification, event filter, formatter and local OS lifecycle',
        'binary': {'path': BINARY, 'sha256': SHA, 'bytes': len(raw)},
        'ida': {'path': RECEIPT, 'sha256': sha((ROOT / RECEIPT).read_bytes()),
                'version': analysis['ida_version'], 'hexrays_available': True,
                'functions': [{'rva': f['rva'], 'end_rva': f['end_rva'], 'code_sha256': f['code_sha256'],
                               'name': f['name']} for f in functions.values()]},
        'command_table': {'rva': 0x99450, 'invoke_rva': 0x6fc0, 'body_rva': 0x2bf0},
        'notification_vtable': {'rva': 0x9d070, 'slots': table(0x9d070, 8)},
        'current_consumers': consumers,
        'activation_receipt': {'path': active_path, 'sha256': sha(active_raw),
                               'automatic_notification_products': active['automatic_notification_products'],
                               'non_automatic_products': [p['product_id'] for p in active['products']
                                                          if not p['automatic_audio_notifications']]},
        'semantics': {
            'source_request': {'module': 'AudioEnumerator', 'command': 'EnableNotification', 'cmdData': {'enable': 'bool or numeric source value'}},
            'source_response': {'response': {'enabled': 'integer source value'}},
            'duplicate_enable': 'Every enable appends another equal callback; only first registers COM; duplicate payloads are preserved',
            'disable': 'Remove all callbacks of the registered lambda type; unregister when vector becomes empty',
            'com_register_slot': 6, 'com_unregister_slot': 7, 'class_context': 23,
            'source_notifications': {'removed': 0, 'added': 1, 'default': 2, 'state': 3, 'property': 4},
            'forwarded_notifications': [0, 1, 3],
            'endpoint_id': 'Require non-null/nonempty UTF-16 endpoint ID; state/flow/role/property and event kind are omitted from source payload',
            'event': {'event': 'RzAudioUtilEvent', 'eventType': 'AudioEnumerator_DeviceChange', 'endpointId': 'UTF-8 source conversion'},
            'current_ui_consumer': 'Only 1422/1446 boot roots activate Audio_StreamMixer and its automatic enable. Other examined products activate generic AudioUtil, whose init does not enable notifications. Wrapper emits exact eventType; active listener uses AudioEnumerator_deviceChange with lowercase d and only console.log. No source-proved cache invalidation or refresh from this event.',
            'native_registration_failures': 'Enable dispatcher ignores register result and sets response; Rust surfaces register/unregister errors instead of fabricating success',
            'cleanup': 'Explicit disable unregisters and releases; Rust retained owner additionally unregisters before apartment shutdown',
        },
        'implementation': {'shared': 'crates/razer-device/src/audio_notification.rs',
                           'service': 'crates/razer-service/src/audio_notification.rs',
                           'windows': 'crates/razer-service/src/platform/windows/audio_notification.rs',
                           'ipc': ['AudioNotificationsEnable', 'AudioNotificationsDrain'],
                           'shell': 'crates/razer-shell/src/shell/audio_notifications.rs',
                           'platforms': 'Windows Core Audio; explicit unsupported on other platforms',
                           'runtime_acceptance': 'not executed; no OS registration or vendor DLL verification'},
        'remaining_gaps': ['OS notification registration/delivery and Shell lifecycle runtime acceptance have not been executed',
                           'AudioRouter complete page consumers and MediaPlayer command/event chains remain incomplete',
                           'RzAudioUtil 1.0.1.1 has separate dispatchMediaRecorder semantics and does not export AudioEnumerator'],
    }
    output = (json.dumps(evidence, ensure_ascii=False, indent=2) + '\n').encode('utf-8')
    if args.check:
        assert (ROOT / OUTPUT).read_bytes() == output, f'Stale {OUTPUT}'
    else:
        (ROOT / OUTPUT).write_bytes(output)
    print(f'Audio notifications: {len(functions)} IDA function bodies, {len(consumers)} current JS receipts; no runtime execution')


if __name__ == '__main__':
    main()
