"""Recover the current Audio Mixer cross-DLL dispatch and report helpers statically."""
import argparse
from collections import deque
import gzip
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess
import pefile

ROOT = Path(__file__).resolve().parents[1]
DUMPBIN = Path(r'C:/Program Files/Microsoft Visual Studio/2022/Enterprise/VC/Tools/MSVC/14.42.34433/bin/Hostx64/x64/dumpbin.exe')
CHILD = '.ref/native-libraries/af10595e3ce6cf9b394488e1929fc6e3d54a887c6be62b66260d7ee507c9e8f2/CmMixerLib_v1.0.1.0.dll'
PARENT = '.ref/native-libraries/d3cf6173a2e8bb7af0a1066da3bd9d6038270884b6fcdc5afdaefd349755da5d/RzNative_053E_v1.0.16.0.dll'
SOURCE = '.ref/middleware/1342/AudioMixer.cde922aae2f0fea23404.js'
DETAIL = 'docs/re/audio-mixer-dll-protocol-current.json'
GRAPH = 'docs/re/dll-device-communication-current.json.gz'
LINE = re.compile(r'^\s*([0-9A-Fa-f]{16}):\s*(\S+)\s*(.*?)\s*$')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def disassembly(pe, path, start, end):
    base = pe.OPTIONAL_HEADER.ImageBase
    proc = subprocess.run([str(DUMPBIN), '/disasm:nobytes', f'/range:{base+start:#x},{base+end-1:#x}',
                           str(ROOT / path)], check=True, capture_output=True,
                          creationflags=subprocess.CREATE_NO_WINDOW)
    instructions = []
    for line in proc.stdout.decode(errors='replace').splitlines():
        m = LINE.match(line)
        if m and start <= int(m[1], 16)-base < end:
            instructions.append({'rva': int(m[1], 16)-base, 'instruction': m[2].lower()+' '+m[3]})
    assert instructions
    return {'start_rva': start, 'end_rva': end,
            'machine_code_sha256': sha(pe.get_data(start, end-start)), 'instructions': instructions}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    graph_bytes = (ROOT / GRAPH).read_bytes()
    graph = json.loads(gzip.decompress(graph_bytes))
    resources = json.loads((ROOT / 'docs/re/native-library-pe-current-evidence.json').read_bytes())['resources']
    identities = {r['path']: r for r in resources}
    binaries = []
    for path in [CHILD, PARENT]:
        raw = (ROOT / path).read_bytes()
        assert sha(raw) == identities[path]['sha256'] and len(raw) == identities[path]['bytes']
        assert any(b['product_id'] == 1342 for b in identities[path]['bindings'])
        binaries.append({'path': path, 'sha256': sha(raw), 'bytes': len(raw),
                         'bindings': identities[path]['bindings']})
    cp, pp = pefile.PE(str(ROOT / CHILD)), pefile.PE(str(ROOT / PARENT))
    assert cp.FILE_HEADER.Machine == pp.FILE_HEADER.Machine == 0x8664
    binary = next(f for f in graph['files'] if f['path'] == CHILD)
    assert len(binary['verified_dispatch_tables']) == 1
    table = binary['verified_dispatch_tables'][0]
    assert len(table['entries']) == 37
    nodes = {n['entry_rva']: n for n in binary['functions']}
    entries = []
    for entry in table['entries']:
        visited, pending, calls, immediates = set(), deque([entry['target_rva']]), [], []
        while pending:
            pc = pending.popleft()
            if pc in visited or pc not in nodes:
                continue
            visited.add(pc)
            n = nodes[pc]
            for i in n['instructions']:
                if re.match(r'mov edx,[0-9A-F]+h$', i['instruction']):
                    immediates.append(i)
            calls.extend(n['import_calls'])
            pending.extend(e['target_rva'] for e in n['direct_edges'])
        entries.append({**entry, 'reachable_graph_entries': sorted(visited),
                        'api_categories': sorted({c['api_category'] for c in calls if c['api_category']}),
                        'edx_immediate_candidates': sorted({i['rva']: i for i in immediates}.values(), key=lambda i: i['rva']),
                        'status': 'dispatch_proven; per-property argument/branch/scale semantics partial'})
    ranges = {'property_dispatch': (0xb8b0, 0xb961),
              'query_u32': (0xc170, 0xc2e9), 'query_u16': (0xc2f0, 0xc458),
              'write_u32': (0xc460, 0xc57c), 'write_u16': (0xc580, 0xc67c),
              'dsp_firmware_property': (0xc680, 0xc6f5),
              'hid_interface_open': (0xbdf0, 0xc080)}
    child_ranges = {label: disassembly(cp, CHILD, *interval) for label, interval in ranges.items()}
    parent_ranges = {'proc_address_store': disassembly(pp, PARENT, 0x336c3, 0x336de),
                     'property_indirect_call': disassembly(pp, PARENT, 0x347a9, 0x34825)}
    cins = {i['rva']: i['instruction'] for r in child_ranges.values() for i in r['instructions']}
    pins = {i['rva']: i['instruction'] for r in parent_ranges.values() for i in r['instructions']}
    expected_child = {0xc1e2: 'mov byte ptr [rsp+40h],4', 0xc28d: 'mov byte ptr [rsp+40h],12h',
                      0xc40d: 'mov byte ptr [rsp+40h],2', 0xc4e2: 'mov byte ptr [rsp+40h],13h',
                      0xc5ec: 'mov byte ptr [rsp+40h],3', 0xc6bb: 'mov edx,5FFC001Ch',
                      0xc214: 'call qword ptr [00000001800120A0h]',
                      0xc2b0: 'call qword ptr [0000000180012008h]',
                      0xc223: 'call qword ptr [0000000180012100h]',
                      0xc272: 'call qword ptr [0000000180012098h]',
                      0xc5e4: 'sar ax,8', 0xc5e8: 'mov byte ptr [rsp+45h],al',
                      0xc5f5: 'mov byte ptr [r11-62h],r8b'}
    assert all(cins.get(rva) == text for rva, text in expected_child.items())
    expected_parent = {0x336ca: 'lea rdx,[000000018025A500h]',
                       0x336d4: 'call qword ptr [00000001801FE898h]',
                       0x336da: 'mov qword ptr [rbx+30h],rax',
                       0x3480a: 'mov r10,qword ptr [rax+30h]', 0x3481b: 'call r10'}
    assert all(pins.get(rva) == text for rva, text in expected_parent.items())
    assert pp.get_string_at_rva(0x25a500) == b'CmMixerPropertyControl'
    imports = {}
    for path, pe in [(CHILD, cp), (PARENT, pp)]:
        imports[path] = {symbol.address-pe.OPTIONAL_HEADER.ImageBase:
                        {'library': lib.dll.decode(), 'symbol': symbol.name.decode() if symbol.name else symbol.ordinal}
                        for lib in pe.DIRECTORY_ENTRY_IMPORT for symbol in lib.imports}
    assert imports[PARENT][0x1fe898]['symbol'] == 'GetProcAddress'
    assert imports[CHILD][0x120a0]['symbol'] == 'WriteFile'
    assert imports[CHILD][0x12008]['symbol'] == 'HidD_GetInputReport'
    assert imports[CHILD][0x12100]['symbol'] == 'WaitForSingleObject'
    assert imports[CHILD][0x12098]['symbol'] == 'Sleep'
    raw = (ROOT / SOURCE).read_bytes()
    source = raw.decode('utf-8')
    receipt = json.loads((ROOT / (SOURCE+'.http.json')).read_bytes())
    assert receipt['http_status'] == 200 and receipt['sha256'] == sha(raw) and receipt['bytes'] == len(raw)
    assert receipt['source_url'] == receipt['final_url'] == 'https://apps.razer.com/synapse/products/1342/mw/'+Path(SOURCE).name
    manifest = json.loads((ROOT / '.ref/middleware/1342/webpackManifest.json').read_bytes())
    assert Path(SOURCE).name in manifest.values()
    occurrences = []
    for match in re.finditer(r'"(RazerT2\w+|MixerSDKLib_\w+)"', source):
        start = len(source[:match.start()].encode('utf-16-le'))//2
        end = start+len(match[0].encode('utf-16-le'))//2
        occurrences.append({'name': match[1], 'offset': start, 'end': end, 'source': match[0]})
    dispatch_names = {e['name'] for e in entries}
    property_names = {o['name'] for o in occurrences if o['name'].startswith('RazerT2')}
    missing = sorted(property_names-dispatch_names)
    result = {'schema_version': 1, 'generator_sha256': sha(Path(__file__).read_bytes()),
              'method': 'Current manifest bindings, direct PE pointer tables, fresh dumpbin ranges, original JS literal ranges; no DLL/device execution',
              'graph': {'path': GRAPH, 'sha256': sha(graph_bytes)}, 'product_id': 1342,
              'binaries': binaries, 'cross_dll_link': {'parent_export': 'MixerSDKLib_PropertyControl',
                  'parent_export_rva': 0x345e0, 'proc_name_rva': 0x25a500,
                  'proc_name': 'CmMixerPropertyControl', 'get_proc_address_call_rva': 0x336d4,
                  'singleton_global_rva': 0x2a6378, 'function_pointer_offset': 0x30,
                  'store_rva': 0x336da, 'pointer_load_rva': 0x3480a, 'indirect_call_rva': 0x3481b,
                  'status': 'Static stored-function-pointer chain proved; runtime loaded module/path identity not executed'},
              'dispatch_table': table, 'properties': entries,
              'parent_code_ranges': parent_ranges, 'child_code_ranges': child_ranges, 'imports': imports,
              'source': {'path': SOURCE, 'sha256': sha(raw), 'offset_unit': 'UTF-16 code units; half-open',
                         'property_literal_receipts': occurrences},
              'source_property_names_without_child_dispatch': missing,
              'report_helpers': [
                  {'entry_rva': 0xc170, 'kind': 'query_u32', 'request_report_id': 4, 'response_report_id': 18,
                   'command_encoding': 'request bytes 1..4 big-endian uint32',
                   'response_encoding': 'response bytes 1..4 little-endian uint32',
                   'send': 'WriteFile, min(66, stored OutputReportByteLength)',
                   'receive': 'HidD_GetInputReport, stored InputReportByteLength',
                   'wait_ms': 100, 'sleep_before_receive_ms': 4, 'transport_return': '0 failure; 1 after decoded output store'},
                  {'entry_rva': 0xc2f0, 'kind': 'query_u16', 'request_report_id': 4, 'response_report_id': 2,
                   'command_encoding': 'request bytes 1..4 big-endian uint32',
                   'response_encoding': 'response bytes 1..2 little-endian uint16'},
                  {'entry_rva': 0xc460, 'kind': 'write_u32', 'request_report_id': 19,
                   'command_encoding': 'bytes 1..4 big-endian uint32; bytes 5..8 big-endian value',
                   'execution': 'Implemented by Rust MixerSession for source-gated recipes; no hardware execution during development'},
                  {'entry_rva': 0xc580, 'kind': 'write_u16', 'request_report_id': 3,
                   'command_encoding': 'bytes 1..4 big-endian uint32; bytes 5..6 big-endian value',
                   'execution': 'Implemented by Rust MixerSession for source-gated hardware endpoint recipes; no hardware execution during development'}],
              'dsp_firmware_query': {'property': 'RazerT2GetDSPFWVersion', 'entry_rva': 0xc680,
                  'command': '0x5ffc001c', 'request_prefix_hex': '04 5f fc 00 1c',
                  'helper_rva': 0xc170, 'returns': {'uninitialized': 2, 'write_request_rejected': -1,
                  'query_failed': 65540, 'success': 0},
                  'decoded_value': 'uint32 formatted into a caller-provided string buffer; version component interpretation unresolved'},
              'limitations': ['Per-property selectors, conversions, gates, UI consumer and scaling remain partial.',
                  'RazerT2KeyShifterLevelEnable occurs in source but is absent from this 37-entry table; do not invent an entry.',
                  'COM operations and ResetStream driver IOCTL cannot be replaced by this HID packet format.',
                  'Query sends output reports; transport writes must be distinguished from settings mutations.',
                  'No DLL loaded, hardware query, write-back or successful device result asserted.']}
    cp.close()
    pp.close()
    encoded = json.dumps(result, ensure_ascii=False, indent=2)+'\n'
    output = ROOT / DETAIL
    if args.check:
        assert output.read_text('utf-8') == encoded, 'Stale Audio Mixer protocol evidence'
    else:
        output.write_text(encoded, encoding='utf-8')
    print(f"Static Audio Mixer: 37 dispatch targets; 4 report helpers; {len(occurrences)} source literal receipts; missing={missing}")


if __name__ == '__main__':
    main()
