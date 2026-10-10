"""Current SysUtils Windows utility commands and live UI caller receipts."""
import argparse
import hashlib
import json
from pathlib import Path

import pefile

ROOT = Path(__file__).resolve().parents[1]
SHA = '01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318'
BINARY = '.ref/host-4.0.827/native-evidence/CommonDLL/SysUtilsNative.dll'
RECEIPT = 'docs/re/evidence/sysutils-system-launch-ida.json'
OUTPUT = 'docs/re/sysutils-system-launch-current-evidence.json'
RVAS = [0x70a30, 0x70b40, 0x70b60, 0x70b80, 0x70ba0]


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--acquire-receipt', action='store_true')
    args = parser.parse_args()
    raw = (ROOT / BINARY).read_bytes()
    assert sha(raw) == SHA
    pe = pefile.PE(data=raw)
    if args.acquire_receipt:
        original = json.loads((ROOT / f'.work/ida-static/corpus/{SHA}/analysis.json').read_text('utf-8'))
        assert original['input_sha256'] == SHA and original['hexrays_available']
        receipt = {k: v for k, v in original.items() if k != 'functions'}
        receipt.update({'source_path': BINARY,
                        'functions': [f for f in original['functions'] if f['rva'] in RVAS],
                        'exporter_sha256': sha((ROOT / 'tools/ida-static-functions.py').read_bytes())})
        (ROOT / RECEIPT).write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n', 'utf-8')
    receipt = json.loads((ROOT / RECEIPT).read_text('utf-8'))
    functions = {f['rva']: f for f in receipt['functions']}
    assert sorted(functions) == RVAS
    native = {
        0x70a30: ['"explorer ms-settings:%s"', 'WinExec(CmdLine, 5u)'],
        0x70b40: ['WinExec("control mmsys.cpl sounds", 5u)'],
        0x70b60: ['WinExec("control keyboard", 5u)'],
        0x70b80: ['WinExec("control main.cpl", 5u)'],
        0x70ba0: ['WinExec("sndvol.exe", 5u)'],
    }
    for rva, tokens in native.items():
        f = functions[rva]
        assert sha(pe.get_data(rva, f['end_rva'] - rva)) == f['code_sha256']
        assert all(t in f['pseudocode'] for t in tokens), hex(rva)
    sources = {
        '.ref/host-4.0.827/electron/modules/sysutil/win/index.js':
            ['msSettings:["void",["string"]]', 'OpenAudioProperties:["void",[]]',
             'OpenKeyboardProperties:["void",[]]', 'OpenMouseProperties:["void",[]]',
             'OpenSoundVolume:["void",[]]'],
        '.ref/host-4.0.827/electron/main.js':
            ['case"msSettings":', 'case"OpenAudioProperties":', 'case"OpenKeyboardProperties":',
             'case"OpenMouseProperties":', 'case"OpenSoundVolume":'],
        '.ref/devices/182/static/js/main.db20a7c4.js':
            ['this.openMouseProperties=()=>{pt.A.OpenMouseProperties()}',
             'this.OpenMouseProperties=()=>{this.isElectron?this.callElectronAction({action:"OpenMouseProperties"})'],
        '.ref/devices/555/static/js/main.110aac54.js':
            ['this.openKeyboardProperties=()=>{xa.A.OpenKeyboardProperties()}'],
        '.ref/devices/1398/static/js/main.cab68a9a.js':
            ['w=()=>{"function"===typeof s.A.OpenAudioProperties&&s.A.OpenAudioProperties()}',
             'K=()=>{"function"===typeof s.A.OpenSoundVolume&&s.A.OpenSoundVolume()}'],
        '.ref/devices/3880/static/js/main.0ff487d9.js':
            ['onClick:()=>{RiA.A.msSettings("display")}'],
    }
    current = []
    for path, tokens in sources.items():
        data = (ROOT / path).read_bytes()
        text = data.decode('utf-8')
        for token in tokens:
            offset = text.find(token)
            assert offset >= 0, (path, token)
            current.append({'path': path, 'sha256': sha(data), 'offset_unit': 'Python Unicode code points',
                            'offset': offset, 'token': token,
                            'snippet': text[max(0, offset - 180):offset + len(token) + 450]})
    result = {
        'scope': 'Current SysUtils four property commands and existing display-settings launcher; not complete library',
        'binary': {'path': BINARY, 'sha256': SHA, 'bytes': len(raw)},
        'ida': {'path': RECEIPT, 'sha256': sha((ROOT / RECEIPT).read_bytes()), 'version': receipt['ida_version'],
                'functions': [{'rva': f['rva'], 'end_rva': f['end_rva'], 'code_sha256': f['code_sha256']}
                              for f in functions.values()]},
        'current_source': current,
        'semantics': {'show': 5, 'api': 'kernel32 WinExec with source ANSI command line',
                      'commands': {'mouse': 'control main.cpl', 'keyboard': 'control keyboard',
                                   'sound': 'control mmsys.cpl sounds', 'volume': 'sndvol.exe',
                                   'display': 'explorer ms-settings:display'},
                      'host_ffi': 'void; original WinExec result is discarded by the JS wrapper',
                      'rust_failure': 'Return error for WinExec result <=31; UI surfaces actual launch failure'},
        'implementation': {'shared': 'crates/razer-platform/src/system.rs',
                           'windows': 'crates/razer-platform/src/platform/windows/system.rs',
                           'ui_callers': ['crates/razer-pages/src/features/mouse_properties.rs',
                                          'crates/razer-pages/src/features/keyboard_properties.rs',
                                          'crates/razer-pages/src/features/audio_page.rs',
                                          'crates/razer-pages/src/features/accessory_system_products.rs'],
                           'runtime_acceptance': 'not_run'},
        'remaining_gaps': ['General parameterized msSettings IPC and other SysUtils launch exports remain incomplete',
                           'Complete SysUtils initialization/termination and other capabilities remain incomplete'],
    }
    output = (json.dumps(result, ensure_ascii=False, indent=2) + '\n').encode('utf-8')
    if args.check:
        assert (ROOT / OUTPUT).read_bytes() == output
    else:
        (ROOT / OUTPUT).write_bytes(output)
    print(f'System launch: {len(functions)} IDA bodies, {len(current)} current JS receipts; no commands executed')


if __name__ == '__main__':
    main()
