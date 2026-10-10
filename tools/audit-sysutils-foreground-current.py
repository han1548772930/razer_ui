"""Verify current SysUtils foreground monitor static IDA/host/product chain.

No target exports, downloaded JavaScript, applications or OS hooks are run.
"""
import argparse
import hashlib
import json
from pathlib import Path

import pefile

ROOT = Path(__file__).resolve().parents[1]
SHA = '01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318'
BINARY = '.ref/host-4.0.827/native-evidence/CommonDLL/SysUtilsNative.dll'
RECEIPT = 'docs/re/evidence/sysutils-foreground-ida.json'
OUTPUT = 'docs/re/sysutils-foreground-current-evidence.json'


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
        followup = [json.loads((ROOT / '.work/ida-static/sysutils-foreground' / file).read_text('utf-8'))
                    for file in ['analysis.json', 'callback.json', 'paths.json', 'child.json', 'crt.json']]
        selected = {f['rva']: f for f in corpus['functions'] if f['rva'] in [0x73870, 0x73ca0, 0x73b80, 0x40ff0, 0x40920]}
        for analysis in followup:
            assert analysis['input_sha256'] == SHA and analysis['hexrays_available']
            selected.update({f['rva']: f for f in analysis['functions']})
        receipt = {key: value for key, value in followup[0].items() if key != 'functions'}
        receipt.update({'source_path': BINARY, 'functions': sorted(selected.values(), key=lambda f: f['rva']),
                        'exporter_sha256': sha((ROOT / 'tools/ida-static-functions.py').read_bytes())})
        (ROOT / RECEIPT).parent.mkdir(parents=True, exist_ok=True)
        (ROOT / RECEIPT).write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + '\n', 'utf-8')
    receipt = json.loads((ROOT / RECEIPT).read_text('utf-8'))
    assert receipt['input_sha256'] == SHA
    bodies = {f['rva']: f for f in receipt['functions']}
    for function in bodies.values():
        assert sha(pe.get_data(function['rva'], function['end_rva'] - function['rva'])) == function['code_sha256']
    def gate(rva, *tokens):
        for token in tokens:
            assert token in bodies[rva]['pseudocode'], (hex(rva), token)
    gate(0x73870, 'byte_18010574B', 'byte_180105844', 'sub_180040FF0')
    gate(0x73b80, 'byte_180105844 = 0', 'sub_180041250', 'sub_180040920')
    gate(0x40c80, 'L"RzMonitorForegroundWindow"', 'L"RzMonitorForegroundWindowClass"',
         'SetWinEventHook(3u, 0x17u, 0i64, pfnWinEventProc, 0, 0, 0)', 'GetMessageW', 'UnhookWinEvent')
    gate(0x41640, '(event == 3 || event == 9) && !idObject && !idChild', 'Sleep(0x64u)',
         r'L"c:\\windows\\explorer.exe"', 'L"applicationframehost"', 'L"wwahost"',
         'EnumChildWindows', '++v6 >= 30', 'SetTimer(v43, 1ui64, 0x12Cu, 0i64)')
    gate(0x40870, 'dwSize = 260', 'OpenProcess(0x1000u', 'QueryFullProcessImageNameW', 'CloseHandle')
    gate(0x40630, 'L"applicationframehost"', 'return 0i64', 'return 1i64')
    gate(0x409d0, 'KillTimer(a2, 1ui64)', 'PostQuitMessage', 'DestroyWindow')
    gate(0x41300, '*(_BYTE *)(a1 + 8) = 1', 'GetCurrentThreadId()', 'Thrd_detach',
         '1000000000', 'PostMessageW(v10, 0x12u', 'sub_18000B880')
    gate(0x3c5a0, '"foregroundWindow"', '"event"', '"name"', '"path"', 'WideCharToMultiByte(0xFDE9u')
    gate(0x9ce78, '(unsigned int)(a1 - 65) <= 0x19', 'a1 + 32')
    thunk = bodies[0x7e360]['instructions']
    assert thunk[0]['text'] == 'mov     rdx, [rdx]' and 'sub_180073B00' in thunk[1]['text']
    consumers = []
    required = {
        '.ref/host-4.0.827/electron/modules/sysutil/win/index.js':
            ['StartMonitorForegroundWindow:["bool",[]]', 'StopMonitorForegroundWindow:["bool",[]]',
             'this.foregroundEventList.includes(t)', 'this.foregroundEventList.length>0',
             'case"foregroundWindow":this.foregroundEventList.includes(e.getURL())'],
        '.ref/middleware/182/main.7d7bac778fbfcdb02c10.js':
            ['action:"StartMonitorForegroundWindow"', 'action:"StopMonitorForegroundWindow"',
             'this.emit("foregroundWindow",e.data)'],
        '.ref/middleware/182/6259.98d162c59be4fff20fa2.js':
            ['c.A.startMonitorForegroundWindow()', 'c.A.on("foregroundWindow"',
             'o.scrollWheelStages', 'const t=e.name.toLowerCase()', '{appName:t}'],
    }
    for path, tokens in required.items():
        data = (ROOT / path).read_bytes()
        text = data.decode('utf-8')
        for token in tokens:
            offset = text.find(token)
            assert offset >= 0, (path, token)
            consumers.append({'path': path, 'sha256': sha(data), 'offset_unit': 'Python Unicode code points',
                              'offset': offset, 'token': token,
                              'snippet': text[max(0, offset - 200):offset + len(token) + 400]})
    result = {
        'scope': 'Current Windows foreground monitor, source callback format and URL fanout; public product helper is not proof of active feature applicability',
        'binary': {'path': BINARY, 'sha256': SHA, 'bytes': len(raw)},
        'ida': {'path': RECEIPT, 'sha256': sha((ROOT / RECEIPT).read_bytes()), 'version': receipt['ida_version'],
                'functions': [{'rva': f['rva'], 'end_rva': f['end_rva'], 'code_sha256': f['code_sha256']}
                              for f in bodies.values()]},
        'current_consumers': consumers,
        'semantics': {
            'start': 'Terminating rejects; initialize if needed; repeated start succeeds without another thread',
            'thread': 'Owned thread, hidden layered window, GetMessageW loop; no initial foreground sample event',
            'hook': {'minimum': 3, 'maximum': 23, 'pid': 0, 'tid': 0, 'flags': 0},
            'filter': {'event': [3, 9], 'object_id': 0, 'child_id': 0},
            'path_query': {'access': 4096, 'utf16_capacity': 260, 'operation': 'QueryFullProcessImageNameW'},
            'explorer_stability_ms': 100, 'uwp_child_poll_ms': 100, 'uwp_child_poll_attempts': 30,
            'debounce': {'timer_id': 1, 'milliseconds': 300},
            'payload': {'event': 'foregroundWindow', 'data': {'name': 'filename, preserve case', 'path': 'parent path, preserve case'}},
            'stop': 'Clear callback before close; quit hidden window, join thread and unhook; source has monitor-thread reentrant detach',
            'host_subscribers': 'Unique sender URLs; stop native monitor only when final URL stops; deliver only to live subscribed views',
            'generic_product_consumer': 'Shared helper visible in 182 chunk: only browsingModeEnabled on active profile, lowercase name -> BIS task {appName}. 182 DeathAdder V3 Pro bootstrap does not prove haptic applicability; requires feature-specific product audit.',
        },
        'implementation': {'shared': 'crates/razer-platform/src/foreground_monitor.rs',
                           'windows': 'crates/razer-platform/src/platform/windows/foreground_monitor.rs',
                           'ipc': ['ForegroundMonitorStart', 'ForegroundMonitorStop', 'ForegroundMonitorEvents'],
                           'runtime_acceptance': 'not_run'},
        'remaining_gaps': ['Actual feature-bearing product active profile/BIS/haptic consumer not yet connected to new IPC; shared code in 182 alone is insufficient',
                           'Nondefault native CRT locale casing branch is not replicated; current default ASCII branch is preserved',
                           'Source reentrant stop is structurally avoided: callbacks only enqueue, worker IPC owns stop; no target callback execution',
                           'Original complete SysUtils initialization/termination and other monitor capabilities remain incomplete'],
    }
    output = (json.dumps(result, ensure_ascii=False, indent=2) + '\n').encode('utf-8')
    if args.check:
        assert (ROOT / OUTPUT).read_bytes() == output
    else:
        (ROOT / OUTPUT).write_bytes(output)
    print(f'SysUtils foreground: {len(bodies)} IDA bodies, {len(consumers)} current JS receipts; no runtime execution')


if __name__ == '__main__':
    main()
