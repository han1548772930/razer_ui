"""Static IDA receipts for current calling-thread keyboard-layout monitor."""
import argparse
import hashlib
import json
from pathlib import Path

import pefile

ROOT = Path(__file__).resolve().parents[1]
SHA = '01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318'
BINARY = '.ref/host-4.0.827/native-evidence/CommonDLL/SysUtilsNative.dll'
RECEIPT = 'docs/re/evidence/sysutils-keyboard-monitor-ida.json'
OUTPUT = 'docs/re/sysutils-keyboard-monitor-current-evidence.json'


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
        corpus = json.loads((ROOT / f'.work/ida-static/corpus/{SHA}/analysis.json').read_text('utf-8'))
        functions = {f['rva']: f for f in corpus['functions'] if f['rva'] in [0x70aa0, 0x77040, 0x770b0]}
        analyses = [json.loads((ROOT / '.work/ida-static/sysutils-keyboard-monitor' / file).read_text('utf-8'))
                    for file in ['analysis.json', 'formatter.json']]
        for analysis in analyses:
            assert analysis['input_sha256'] == SHA and analysis['hexrays_available']
            functions.update({f['rva']: f for f in analysis['functions']})
        receipt = {key: value for key, value in analyses[0].items() if key != 'functions'}
        receipt.update({'source_path': BINARY, 'functions': sorted(functions.values(), key=lambda f: f['rva']),
                        'exporter_sha256': sha((ROOT / 'tools/ida-static-functions.py').read_bytes())})
        (ROOT / RECEIPT).parent.mkdir(parents=True, exist_ok=True)
        (ROOT / RECEIPT).write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n', 'utf-8')
    receipt = json.loads((ROOT / RECEIPT).read_text('utf-8'))
    bodies = {f['rva']: f for f in receipt['functions']}
    for function in bodies.values():
        assert sha(pe.get_data(function['rva'], function['end_rva'] - function['rva'])) == function['code_sha256']
    def gate(rva, *tokens):
        for token in tokens:
            assert token in bodies[rva]['pseudocode'], (hex(rva), token)
    gate(0x77040, 'if ( !uIDEvent )', 'dword_1801057A0 = sub_180070AA0()', 'SetTimer(0i64, 0i64, 0x7D0u, TimerFunc)')
    gate(0x77010, 'dword_1801057A0 != v1', 'dword_1801057A0 = v1', 'sub_18006BFF0')
    gate(0x770b0, 'KillTimer(0i64, uIDEvent)', 'uIDEvent = 0i64')
    gate(0x70aa0, 'GetKeyboardLayoutNameA', '"%x"', 'v1[0] = 0')
    gate(0x6bff0, '"keyboardlayoutchange"', '"event"', '"data"', '"langId"', 'v0 = dword_1801057A0')
    formatter = bodies[0x6bff0]['instructions']
    assert any('movsxd' in i['text'] and 'dword_1801057A0' in i['text'] for i in formatter)
    receipts = []
    required = {
        '.ref/host-4.0.827/electron/modules/sysutil/win/index.js':
            ['StartMonitorKeyboardLayout:["void",[]]', 'StopMonitorKeyboardLayout:["void",[]]',
             'this.keyboardLayoutChangeEventList=this.foregroundEventList.filter(e=>e!==t)',
             'case"keyboardlayoutchange":this.keyboardLayoutChangeEventList.includes(e.getURL())'],
        '.ref/middleware/182/main.7d7bac778fbfcdb02c10.js':
            ['action:"StartMonitorKeyboardLayout"', 'action:"StopMonitorKeyboardLayout"',
             'this.emit("keyboardlayoutchange",e.data)'],
    }
    for path, tokens in required.items():
        data = (ROOT / path).read_bytes()
        text = data.decode('utf-8')
        for token in tokens:
            offset = text.find(token)
            assert offset >= 0, (path, token)
            receipts.append({'path': path, 'sha256': sha(data), 'offset_unit': 'Python Unicode code points',
                             'offset': offset, 'token': token,
                             'snippet': text[max(0, offset - 200):offset + len(token) + 500]})
    result = {
        'scope': 'Native calling-thread keyboard layout timer, change formatter and host subscription bug; application routing remains a gap',
        'binary': {'path': BINARY, 'sha256': SHA, 'bytes': len(raw)},
        'ida': {'path': RECEIPT, 'sha256': sha((ROOT / RECEIPT).read_bytes()), 'version': receipt['ida_version'],
                'functions': [{'rva': f['rva'], 'end_rva': f['end_rva'], 'code_sha256': f['code_sha256']}
                              for f in bodies.values()]},
        'current_consumers': receipts,
        'semantics': {
            'baseline': 'Query GetKeyboardLayoutNameA on creating thread and parse zero-initialized ANSI string with %x',
            'timer': {'hwnd': None, 'requested_id': 0, 'interval_ms': 2000, 'callback_rva': 0x77010},
            'thread_contract': 'HWND-less timer callbacks require the creating thread to dispatch messages; HKL is that same thread, not foreground thread',
            'change': 'No initial event; compare full 32-bit layout, update cache before formatter, skip equal value',
            'event': {'event': 'keyboardlayoutchange', 'data': {'langId': 'signed int32 promoted to JSON signed integer'}},
            'native_stop': 'KillTimer(NULL,id) then clear id regardless of BOOL; wrapper exposes void',
            'host_stop_bug': 'Replace keyboard URL list with foreground URL list excluding sender; skip native stop whenever original foreground list is nonempty',
        },
        'implementation': {'shared': 'crates/razer-platform/src/keyboard_layout_monitor.rs',
                           'windows': 'crates/razer-platform/src/platform/windows/keyboard_layout_monitor.rs',
                           'owner': 'Thread-bound !Send UI/message-pumped owner; no background thread replacement',
                           'runtime_acceptance': 'not_run'},
        'remaining_gaps': ['Actual feature/product startup and UI message-pumped owner not yet routed to platform timer',
                           'Blocking stdin IPC worker has no message pump; no false equivalent IPC start/poll is exposed',
                           'Complete SysUtils initialization/termination and other capabilities remain incomplete'],
    }
    output = (json.dumps(result, ensure_ascii=False, indent=2) + '\n').encode('utf-8')
    if args.check:
        assert (ROOT / OUTPUT).read_bytes() == output
    else:
        (ROOT / OUTPUT).write_bytes(output)
    print(f'Keyboard monitor: {len(bodies)} IDA bodies, {len(receipts)} current source receipts; no runtime execution')


if __name__ == '__main__':
    main()
