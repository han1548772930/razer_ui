"""Statically recover current RzAudioUtil AudioEnumerator implementation."""
import argparse
import hashlib
import importlib
import json
from pathlib import Path
import struct
import uuid

import pefile

ROOT = Path(__file__).resolve().parents[1]
PATH = '.ref/native-libraries/9134a79a2aac0d3ca48087fe2ad62ab29b44ad334c9d85edf36171c7ea79caaf/RzAudioUtil_v1.0.3.1.dll'
OUTPUT = 'docs/re/audio-util-enumerator-current-evidence.json'
SHA = '9134a79a2aac0d3ca48087fe2ad62ab29b44ad334c9d85edf36171c7ea79caaf'
helper = importlib.import_module('audit-cmmixer-protocol-current')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    raw = (ROOT / PATH).read_bytes()
    assert sha(raw) == SHA
    pe = pefile.PE(data=raw)
    assert pe.FILE_HEADER.Machine == 0x8664
    source = json.loads((ROOT / 'docs/re/native-library-pe-current-evidence.json').read_bytes())
    identity = next(r for r in source['resources'] if r['path'] == PATH)
    assert identity['sha256'] == SHA and identity['bytes'] == len(raw)
    exports = {e.name.decode(): e.address for e in pe.DIRECTORY_ENTRY_EXPORT.symbols if e.name}
    assert exports['Dispatch'] == 0x1af70 and exports['dispatchAudioEnumerator'] == 0x1660
    functions = {e.struct.BeginAddress: e.struct.EndAddress for e in pe.DIRECTORY_ENTRY_EXCEPTION}
    ranges = {}
    for name, start in [('dispatch', 0x1af70), ('audio_enumerator_dispatch', 0x1660),
                        ('playback_names', 0x2110), ('record_names', 0x2680),
                        ('enumerate_vector', 0x44ad0), ('enumerate_endpoints', 0x44dc0)]:
        ranges[name] = helper.disassembly(pe, PATH, start, functions[start])
    # Invoke thunks are leaf functions and absent from .pdata; exact 9-byte
    # add-this-and-jump instruction sequence supplies their proven boundary.
    for name, start in [('playback_invoke_thunk', 0x7040), ('record_invoke_thunk', 0x7000)]:
        ranges[name] = helper.disassembly(pe, PATH, start, start + 9)
    ins = {i['rva']: i['instruction'] for r in ranges.values() for i in r['instructions']}
    gates = {
        0x1942: 'lea rdx,[00000001800987E0h]',
        0x1956: 'lea rax,[00000001800994A0h]',
        0x19a6: 'lea rdx,[0000000180098800h]',
        0x19ba: 'lea rax,[0000000180098A38h]',
        0x7044: 'jmp 0000000180002110',
        0x7004: 'jmp 0000000180002680',
        0x2173: 'xor r8d,r8d',
        0x26e3: 'mov r8d,1',
        0x44ae5: 'call 0000000180044DC0',
        0x44e50: 'mov r8d,17h',
        0x44e5d: 'call qword ptr [000000018008A4C0h]',
        0x44ec7: 'mov r8d,ebx',
        0x44eca: 'mov edx,ebx',
        0x44f29: 'mov r8d,ebx',
        0x44f2c: 'xor edx,edx',
        0x44fca: 'mov edx,ebx',
        0x45104: 'mov r8d,2',
        0x45238: 'call qword ptr [rax+18h]',
        0x4528c: 'call qword ptr [rax+20h]',
        0x45324: 'call qword ptr [rax+28h]',
        0x454bf: 'call qword ptr [rax+20h]',
        0x45507: 'lea rdx,[000000018008A658h]',
        0x4550e: 'call qword ptr [rax+28h]',
        0x45515: 'cmp eax,401A0h',
        0x455b7: 'cmp eax,1Fh',
        0x45685: 'lea rdx,[000000018008A640h]',
        0x45693: 'cmp eax,401A0h',
        0x4572f: 'cmp eax,1Fh',
        0x21e0: 'lea rdx,[r8+20h]',
        0x2750: 'lea rdx,[r8+20h]',
    }
    for address, text in gates.items():
        assert ins.get(address) == text, (hex(address), ins.get(address), text)
    tables = []
    for name, address, thunk in [('GetWindowsPlaybackDevices', 0x994a0, 0x7040),
                               ('GetWindowsRecordDevices', 0x98a38, 0x7000)]:
        pointer = struct.unpack('<Q', pe.get_data(address + 16, 8))[0]
        assert pointer - pe.OPTIONAL_HEADER.ImageBase == thunk
        tables.append({'command': name, 'vtable_rva': address, 'invoke_slot': 16,
                       'invoke_thunk_rva': thunk})
    constants = []
    for kind, address, guid, pid in [
        ('MMDeviceEnumerator', 0x99998, 'bcde0395-e52f-467c-8e3d-c4579291692e', None),
        ('IMMDeviceEnumerator', 0x99e00, 'a95664d2-9614-4f35-a746-de8db63617e6', None),
        ('PKEY_Device_FriendlyName', 0x8a658, 'a45c254e-df1c-4efd-8020-67d146a850e0', 14),
        ('PKEY_Device_DeviceDesc', 0x8a640, 'a45c254e-df1c-4efd-8020-67d146a850e0', 2),
    ]:
        data = pe.get_data(address, 20 if pid else 16)
        assert str(uuid.UUID(bytes_le=data[:16])) == guid
        if pid is not None:
            assert struct.unpack('<I', data[16:])[0] == pid
        constants.append({'kind': kind, 'rva': address, 'guid': guid, 'pid': pid,
                          'bytes_hex': data.hex()})
    imports = {s.address - pe.OPTIONAL_HEADER.ImageBase: s.name.decode()
               for lib in pe.DIRECTORY_ENTRY_IMPORT for s in lib.imports if s.name}
    for address, name in [(0x8a4c0, 'CoCreateInstance'), (0x8a4d0, 'CoTaskMemFree'),
                          (0x8a4d8, 'PropVariantClear')]:
        assert imports[address] == name
    consumers = []
    for binding in identity['bindings']:
        manifest = (ROOT / binding['source']).read_bytes()
        assert sha(manifest) == binding['source_sha256']
        for path in sorted((ROOT / f".ref/middleware/{binding['product_id']}").glob('*.js')):
            data = path.read_bytes()
            text = data.decode('utf-8')
            for token in ['dispatch("AudioEnumerator","GetWindowsPlaybackDevices"',
                          'dispatch("AudioEnumerator","GetWindowsRecordDevices"']:
                start = 0
                while (offset := text.find(token, start)) >= 0:
                    a, z = max(0, offset-180), min(len(text), offset+220)
                    consumers.append({'product_id': binding['product_id'],
                                      'path': path.relative_to(ROOT).as_posix(),
                                      'sha256': sha(data), 'offset': offset,
                                      'token': token, 'snippet': text[a:z]})
                    start = offset + len(token)
    result = {
        'scope': 'Two current AudioEnumerator operations; not the whole RzAudioUtil DLL',
        'verification': 'Static PE decoding and current middleware text; no DLL/JS execution',
        'binary': {'path': PATH, 'sha256': SHA, 'bytes': len(raw), 'bindings': identity['bindings']},
        'export_entries': {'Dispatch': 0x1af70, 'dispatchAudioEnumerator': 0x1660},
        'command_tables': tables, 'constants': constants,
        'instruction_gates': [{'rva': a, 'instruction': s} for a, s in gates.items()],
        'code_ranges': ranges, 'current_consumers': consumers,
        'semantics': {
            'flow': {'playback': 0, 'record': 1}, 'state_mask': 1,
            'class_context': 0x17, 'default_roles': [0, 2],
            'properties': {'friendly_name': 14, 'description': 2},
            'property_accepted_results': [0, 0x401a0], 'property_variant_tag': 31,
            'record_stride': 0x68, 'friendly_name_offset': 0x20,
            'response': {'devices': 'Ordered friendly-name string array; preserve empty/duplicate names'},
            'fatal_failure': 'Native helper swallows several failures into an empty vector; Rust returns explicit errors',
            'item_failure': 'Skip item when Item/GetId/OpenPropertyStore/GetValue/type checks fail',
            'default_failure': 'GetDefaultAudioEndpoint failure leaves empty ID; successful endpoint with failed GetId aborts',
            'memory': 'Release every COM interface, CoTaskMemFree every GetId string, PropVariantClear both property values',
        },
        'implementation': {'shared': 'crates/razer-device/src/audio_util.rs',
                           'windows': 'crates/razer-service/src/audio_util.rs',
                           'runtime_acceptance': 'not executed; current development execution prohibition'},
        'remaining_library_gaps': ['RzAudioUtil EnableNotification runtime acceptance (shared/Windows/IPC/Shell implementation separately audited)',
                                   'RzAudioUtil AudioRouter EnableRouting/RouteDevice write-back/lifecycle',
                                   'RzAudioUtil MediaPlayer commands/output/state/events'],
    }
    output = (json.dumps(result, ensure_ascii=False, indent=2)+'\n').encode('utf-8')
    target = ROOT / OUTPUT
    if args.check:
        assert target.read_bytes() == output, f'Stale {OUTPUT}'
    else:
        target.write_bytes(output)
    print(f'AudioUtil: {len(ranges)} static ranges, {len(consumers)} current call sites, 2 direct COM operations')


if __name__ == '__main__':
    main()
