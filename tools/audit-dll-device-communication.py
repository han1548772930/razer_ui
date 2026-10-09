"""Static export-to-import machine-code graphs of acquired current native files.

dumpbin reads bytes only. No LoadLibrary, device command, vendor JS or DLL call.
Graph paths mean syntactic potential reachability, never runtime behavior or ABI.
"""
import argparse
from bisect import bisect_right
from collections import Counter, deque
from concurrent.futures import ThreadPoolExecutor
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
OUT = ROOT / 'docs/re/dll-device-communication-current.json.gz'
SUMMARY = ROOT / 'docs/re/dll-device-communication-current-summary.json'
CACHE = ROOT / '.work/dll-device-communication-static'
INPUTS = ['docs/re/native-library-pe-current-evidence.json',
          'docs/re/host-native-library-pe-current-evidence.json',
          'docs/re/host-full-asar-current-evidence.json']
LINE = re.compile(r'^\s*([0-9A-Fa-f]{8,16}):\s*(\S+)\s*(.*?)\s*$')
DIRECT = re.compile(r'^([0-9A-Fa-f]{8,16})(?:h)?$')
ABSOLUTE = re.compile(r'\[([0-9A-Fa-f]{8,16})h?\]')
ASCII = re.compile(rb'[\x20-\x7e]{5,}\x00')
WIDE = re.compile(rb'(?:[\x20-\x7e]\x00){5,}\x00\x00')
# Categories describe imported API roles only, not a per-export semantic claim.
SYMBOLS = {
    'hid_feature': {'HidD_SetFeature', 'HidD_GetFeature', 'HidD_SetOutputReport', 'HidD_GetInputReport'},
    'hid_metadata': {'HidD_GetHidGuid', 'HidD_GetAttributes', 'HidD_GetPreparsedData',
                     'HidD_FreePreparsedData', 'HidD_GetSerialNumberString', 'HidP_GetCaps'},
    'device_ioctl': {'DeviceIoControl'},
    'file_or_pipe': {'CreateFileA', 'CreateFileW', 'ReadFile', 'WriteFile', 'CreateNamedPipeA',
                     'CreateNamedPipeW', 'CallNamedPipeA', 'CallNamedPipeW', 'WaitNamedPipeA',
                     'WaitNamedPipeW', 'ConnectNamedPipe', 'GetOverlappedResult'},
    'dynamic_loading': {'LoadLibraryA', 'LoadLibraryW', 'LoadLibraryExA', 'LoadLibraryExW',
                        'GetProcAddress', 'GetModuleHandleA', 'GetModuleHandleW'},
    'com_activation': {'CoCreateInstance', 'CoInitializeEx', 'CoGetClassObject', 'CLSIDFromString'},
    'service_management': {'OpenSCManagerA', 'OpenSCManagerW', 'OpenServiceA', 'OpenServiceW',
                           'StartServiceA', 'StartServiceW', 'QueryServiceStatus', 'ControlService'},
}
LIBRARY_CLASSES = {'winusb.dll': 'winusb', 'ws2_32.dll': 'network_socket',
                   'winhttp.dll': 'http', 'wininet.dll': 'http', 'rpcrt4.dll': 'rpc',
                   'mf.dll': 'media_foundation', 'mfplat.dll': 'media_foundation',
                   'mfplay.dll': 'media_foundation', 'setupapi.dll': 'device_enumeration',
                   'cfgmgr32.dll': 'device_enumeration'}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def category(library, name):
    for label, names in SYMBOLS.items():
        if name in names:
            return label
    return LIBRARY_CLASSES.get(library.lower())


def resources():
    receipts, rows = [], []
    for rel in INPUTS:
        body = (ROOT / rel).read_bytes()
        receipts.append({'path': rel, 'sha256': digest(body), 'bytes': len(body)})
        obj = json.loads(body)
        if 'resources' in obj:
            assert not obj['failures']
            for row in obj['resources']:
                rows.append({**row, 'origin': 'current_product_manifest'})
        elif rel.endswith('host-native-library-pe-current-evidence.json'):
            rows.extend({**r, 'origin': 'current_host_common_dll'} for r in obj['files'])
        else:
            for row in obj['files']:
                if not row['path'].lower().endswith(('.dll', '.node')):
                    continue
                # ELF/Mach-O are recorded separately, not fed to a PE disassembler.
                raw = (ROOT / row['path']).read_bytes()
                assert digest(raw) == row['sha256']
                if raw[:2] == b'MZ':
                    rows.append({**row, 'file': Path(row['path']).name, 'origin': 'current_host_asar'})
    merged = {}
    for row in rows:
        identity = row['sha256']
        alias = {'path': row['path'], 'origin': row['origin'], 'bindings': row.get('bindings', [])}
        if identity not in merged:
            merged[identity] = {**row, 'aliases': [alias]}
        else:
            merged[identity]['aliases'].append(alias)
    return receipts, sorted(merged.values(), key=lambda r: r['path'])


def disassemble(row, pe, tool_sha, refresh):
    executable = [s for s in pe.sections if s.Characteristics & 0x20000000 and s.SizeOfRawData]
    cache = CACHE / (row['sha256'] + '.json.gz')
    expected = {'binary_sha256': row['sha256'], 'dumpbin_sha256': tool_sha,
                'method': 'dumpbin /disasm:nobytes executable raw-backed sections v1'}
    if cache.exists() and not refresh:
        recorded = json.loads(gzip.decompress(cache.read_bytes()))
        assert recorded['identity'] == expected, 'Disassembler cache identity changed; use --refresh-disassembly'
        assert digest(recorded['text'].encode()) == recorded['text_sha256']
        return recorded['text'], recorded['text_sha256']
    texts = []
    base = pe.OPTIONAL_HEADER.ImageBase
    for section in executable:
        start = base + section.VirtualAddress
        end = start + min(section.SizeOfRawData, section.Misc_VirtualSize or section.SizeOfRawData)
        proc = subprocess.run([str(DUMPBIN), '/disasm:nobytes', f'/range:{start:#x},{end-1:#x}',
                               str(ROOT / row['path'])], capture_output=True, check=True,
                              creationflags=subprocess.CREATE_NO_WINDOW)
        # Remove host banners and use one normalized, independently hashed stream.
        texts.extend(line.strip() for line in proc.stdout.decode(errors='replace').splitlines()
                     if LINE.match(line) and start <= int(LINE.match(line)[1], 16) < end)
    text = '\n'.join(texts)
    assert text, 'No executable instructions decoded: ' + row['path']
    CACHE.mkdir(parents=True, exist_ok=True)
    cache.write_bytes(gzip.compress(json.dumps({'identity': expected, 'text': text,
                                               'text_sha256': digest(text.encode())}).encode(), mtime=0))
    return text, digest(text.encode())


def interesting_strings(raw, pe):
    result = {}
    for pattern, encoding in [(ASCII, 'ascii'), (WIDE, 'utf-16-le')]:
        for match in pattern.finditer(raw):
            text = match[0].decode(encoding).rstrip('\0')
            if not (re.search(r'(?i)(?:hid[dp]?_|\.dll\b|\.sys\b|\\pipe\\|\\device\\|https?://|\\\\\.\\|deviceiocontrol|winusb|razerservice)', text)):
                continue
            try:
                rva = pe.get_rva_from_offset(match.start())
            except pefile.PEFormatError:
                continue
            # Security certificate / overlay offsets are file evidence, not RVA data.
            if not any(s.PointerToRawData <= match.start() < s.PointerToRawData+s.SizeOfRawData
                       for s in pe.sections):
                continue
            result[rva] = {'rva': rva, 'file_offset': match.start(), 'encoding': encoding,
                           'bytes': len(match[0]), 'value': text}
    return result


def inspect(row, tool_sha, refresh):
    raw = (ROOT / row['path']).read_bytes()
    assert digest(raw) == row['sha256'] and len(raw) == row['bytes']
    pe = pefile.PE(data=raw)
    machine = pe.FILE_HEADER.Machine
    header = {'path': row['path'], 'file': row['file'], 'sha256': row['sha256'], 'bytes': len(raw),
              'aliases': row['aliases'], 'machine': machine, 'semantic_status': 'partial'}
    if machine not in (0x8664, 0x14c):
        unexamined_exports = [{'name': s.name.decode('ascii', errors='replace') if s.name else None,
                               'ordinal': s.ordinal, 'rva': s.address, 'status': 'architecture_not_analyzed',
                               'forwarder': s.forwarder.decode() if s.forwarder else None}
                              for s in getattr(getattr(pe, 'DIRECTORY_ENTRY_EXPORT', None), 'symbols', [])]
        pe.close()
        return {**header, 'status': 'architecture_not_analyzed', 'reason': 'Only x86/x64 disassembly parser audited',
                'exports': unexamined_exports, 'functions': [], 'imports': [], 'candidate_strings': []}
    base = pe.OPTIONAL_HEADER.ImageBase
    text, text_sha = disassemble(row, pe, tool_sha, refresh)
    code = {}
    for line in text.splitlines():
        m = LINE.match(line)
        code[int(m[1], 16)-base] = (m[2].lower(), m[3])
    addresses = sorted(code)
    following = dict(zip(addresses, addresses[1:]))
    unwind = sorted({(r.struct.BeginAddress, r.struct.EndAddress)
                     for r in getattr(pe, 'DIRECTORY_ENTRY_EXCEPTION', [])})
    unwind_starts = [r[0] for r in unwind]
    imports = {}
    for mode, entries in [('import', getattr(pe, 'DIRECTORY_ENTRY_IMPORT', [])),
                          ('delay_import', getattr(pe, 'DIRECTORY_ENTRY_DELAY_IMPORT', []))]:
        for lib in entries:
            library = lib.dll.decode('ascii', errors='replace')
            for symbol in lib.imports:
                name = symbol.name.decode('ascii', errors='replace') if symbol.name else f'ordinal:{symbol.ordinal}'
                slot = symbol.address-base
                imports[slot] = {'slot_rva': slot, 'library': library, 'symbol': name,
                                 'mode': mode, 'api_category': category(library, name)}
    strings = interesting_strings(raw, pe)
    nodes = {}
    dispatch_tables = []
    table_edges = {}
    if row['sha256'] == 'af10595e3ce6cf9b394488e1929fc6e3d54a887c6be62b66260d7ee507c9e8f2':
        # This is a binary-specific recovered dispatch, not a general vtable guess.
        expected = {0xb8cd: ('lea', 'rdi,[00000001800134B0h]'),
                    0xb90f: ('cmp', 'ebx,25h'),
                    0xb943: ('lea', 'r10,[0000000180013380h]'),
                    0xb95d: ('call', 'qword ptr [r10+rax*8]')}
        assert all(code[rva] == value for rva, value in expected.items())
        entries = []
        for index in range(37):
            name_rva = struct.unpack('<Q', pe.get_data(0x134b0+index*8, 8))[0]-base
            target = struct.unpack('<Q', pe.get_data(0x13380+index*8, 8))[0]-base
            assert target in code
            data = pe.get_data(name_rva, 256)
            name = data.decode('utf-16-le', errors='strict').split('\0')[0]
            assert re.fullmatch(r'RazerT2\w+', name)
            entries.append({'index': index, 'name': name, 'name_rva': name_rva,
                            'target_rva': target, 'name_pointer_rva': 0x134b0+index*8,
                            'function_pointer_rva': 0x13380+index*8})
        dispatch_tables.append({'export': 'CmMixerPropertyControl', 'call_site_rva': 0xb95d,
                                'name_table_rva': 0x134b0, 'target_table_rva': 0x13380,
                                'entries': entries,
                                'selection': 'UTF-16 exact string comparison; index < 37; same index loads function pointer',
                                'semantics': 'Candidate target selected by property name; arguments and branch gates need per-target analysis'})
        table_edges[0xb95d] = [{'instruction_rva': 0xb95d, 'kind': 'verified_dispatch_table_candidate',
                               'target_rva': e['target_rva'], 'selector_index': e['index'],
                               'selector_name': e['name']} for e in entries]

    def node(start):
        if start in nodes:
            return nodes[start]
        i = bisect_right(unwind_starts, start)-1
        boundary = unwind[i] if i >= 0 and unwind[i][0] <= start < unwind[i][1] else None
        work, seen, branches, external, indirect, string_refs, stops, resolved_tables = [start], set(), [], [], [], [], [], []
        while work:
            pc = work.pop()
            if pc in seen:
                continue
            if pc not in code:
                stops.append({'rva': pc, 'reason': 'instruction_not_decoded'})
                continue
            if len(seen) >= 8192:
                stops.append({'rva': pc, 'reason': 'instruction_budget'})
                break
            seen.add(pc)
            mnemonic, operand = code[pc]
            for ref in ABSOLUTE.finditer(operand):
                target = int(ref[1], 16)-base
                if target in strings:
                    string_refs.append({'instruction_rva': pc, 'string_rva': target})
            if mnemonic in ('ret', 'retf', 'iret', 'iretq', 'ud2') or (mnemonic == 'int' and operand == '3'):
                continue
            direct = DIRECT.fullmatch(operand)
            target = int(direct[1], 16)-base if direct else None
            if mnemonic in ('call', 'jmp'):
                if target is not None:
                    # Local jumps stay in a known unwind interval. Unknown leaf
                    # intervals keep jumps as graph edges; do not invent bounds.
                    if mnemonic == 'jmp' and boundary and boundary[0] <= target < boundary[1]:
                        work.append(target)
                    else:
                        branches.append({'instruction_rva': pc, 'kind': mnemonic, 'target_rva': target})
                else:
                    absolute = ABSOLUTE.fullmatch(operand.replace('qword ptr ', '').replace('dword ptr ', ''))
                    slot = int(absolute[1], 16)-base if absolute else None
                    if pc in table_edges:
                        resolved_tables.extend(table_edges[pc])
                    elif slot in imports:
                        external.append({'instruction_rva': pc, **imports[slot]})
                    else:
                        indirect.append({'instruction_rva': pc, 'instruction': mnemonic+' '+operand,
                                         'reason': 'register/vtable/function pointer target unresolved'})
                if mnemonic == 'jmp':
                    continue
            elif mnemonic.startswith('j') or mnemonic.startswith('loop'):
                if target is not None:
                    if boundary and not boundary[0] <= target < boundary[1]:
                        branches.append({'instruction_rva': pc, 'kind': mnemonic, 'target_rva': target})
                    else:
                        work.append(target)
                else:
                    stops.append({'rva': pc, 'reason': 'conditional_target_not_decoded'})
            nxt = following.get(pc)
            if nxt is None:
                stops.append({'rva': pc, 'reason': 'fallthrough_outside_decoded_range'})
            elif nxt-pc > 15:
                stops.append({'rva': pc, 'reason': 'decoded_instruction_gap'})
            elif boundary and not boundary[0] <= nxt < boundary[1]:
                # MSVC splits one logical routine across multiple adjacent
                # RUNTIME_FUNCTION records. An ordinary instruction falls into
                # the next record; unwind boundaries are not return statements.
                branches.append({'instruction_rva': pc, 'kind': 'unwind_range_fallthrough', 'target_rva': nxt})
            else:
                work.append(nxt)
        instruction_rows = [{'rva': pc, 'instruction': code[pc][0]+' '+code[pc][1]} for pc in sorted(seen)]
        spans = []
        for pc in sorted(seen):
            # Last instruction spans up to the next decoded address (max x86 length
            # 15). A missing successor remains an explicit unbounded byte end.
            end = following.get(pc)
            if end is None or not 0 < end-pc <= 15:
                continue
            if spans and spans[-1]['end_rva'] == pc:
                spans[-1]['end_rva'] = end
            else:
                spans.append({'start_rva': pc, 'end_rva': end})
        for span in spans:
            span['machine_code_sha256'] = digest(pe.get_data(span['start_rva'], span['end_rva']-span['start_rva']))
        result = {'entry_rva': start, 'unwind_range': list(boundary) if boundary else None,
                  'instructions': instruction_rows, 'machine_code_spans': spans,
                  'direct_edges': sorted(branches, key=lambda r: r['instruction_rva']),
                  'resolved_table_edges': resolved_tables,
                  'import_calls': sorted(external, key=lambda r: r['instruction_rva']),
                  'unresolved_indirect_calls': sorted(indirect, key=lambda r: r['instruction_rva']),
                  'candidate_string_references': sorted(string_refs, key=lambda r: r['instruction_rva']),
                  'analysis_stops': sorted(stops, key=lambda r: (r['rva'], r['reason']))}
        nodes[start] = result
        return result

    exports = []
    for symbol in getattr(getattr(pe, 'DIRECTORY_ENTRY_EXPORT', None), 'symbols', []):
        name = symbol.name.decode('ascii', errors='replace') if symbol.name else None
        exports.append({'name': name, 'ordinal': symbol.ordinal, 'rva': symbol.address,
                        'forwarder': symbol.forwarder.decode() if symbol.forwarder else None})
    pending = deque(e['rva'] for e in exports if not e['forwarder'] and e['rva'] in code)
    while pending:
        pc = pending.popleft()
        if pc in nodes:
            continue
        n = node(pc)
        pending.extend(e['target_rva'] for e in n['direct_edges']+n['resolved_table_edges'] if e['target_rva'] in code)
    for export in exports:
        start = export['rva']
        if export['forwarder']:
            export['status'] = 'forwarder'
            continue
        if start not in nodes:
            export['status'] = 'not_decoded_code_export'
            continue
        export['status'] = 'syntactic_graph_partial'
        paths, visited, queue = {}, {start}, deque([(start, [])])
        unresolved, stops = set(), set()
        while queue:
            pc, chain = queue.popleft()
            n = nodes[pc]
            for call in n['import_calls']:
                if call['api_category'] and call['slot_rva'] not in paths:
                    paths[call['slot_rva']] = {'import': imports[call['slot_rva']],
                        'path': chain + [{'function_entry_rva': pc, 'instruction_rva': call['instruction_rva']}],
                        'claim': 'syntactic_path_only; branch feasibility and arguments not established'}
            unresolved.update(c['instruction_rva'] for c in n['unresolved_indirect_calls'])
            stops.update((c['rva'], c['reason']) for c in n['analysis_stops'])
            for edge in n['direct_edges']+n['resolved_table_edges']:
                dst = edge['target_rva']
                if dst in nodes and dst not in visited:
                    visited.add(dst)
                    queue.append((dst, chain + [{'function_entry_rva': pc, 'kind': edge['kind'],
                                                 'instruction_rva': edge['instruction_rva'], 'target_rva': dst,
                                                 **({'selector_name': edge['selector_name']} if 'selector_name' in edge else {})}]))
                elif dst not in nodes:
                    stops.add((edge['instruction_rva'], 'direct_target_not_analyzed'))
        export['reachable_function_count'] = len(visited)
        export['api_paths'] = [paths[k] for k in sorted(paths)]
        export['unresolved_indirect_call_count'] = len(unresolved)
        export['analysis_stop_count'] = len(stops)
    result = {**header, 'status': 'export_graph_partial', 'image_base': base,
              'disassembly_sha256': text_sha, 'decoded_instruction_count': len(code),
              'unwind_range_count': len(unwind), 'exports': exports,
              'imports': [imports[k] for k in sorted(imports)],
              'candidate_strings': [strings[k] for k in sorted(strings)],
              'functions': [nodes[k] for k in sorted(nodes)],
              'verified_dispatch_tables': dispatch_tables,
              'clr_header_present': bool(pe.OPTIONAL_HEADER.DATA_DIRECTORY[14].VirtualAddress)}
    pe.close()
    print(f"Static {row['file']}: {len(exports)} exports; {len(nodes)} graph entries", flush=True)
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--refresh-disassembly', action='store_true')
    parser.add_argument('--only', help='Filename substring for inspection; no committed output')
    args = parser.parse_args()
    inputs, rows = resources()
    if args.only:
        rows = [r for r in rows if args.only.lower() in r['file'].lower()]
        assert rows, 'No binary selected'
    tool_sha = digest(DUMPBIN.read_bytes())
    with ThreadPoolExecutor(max_workers=3) as pool:
        files = list(pool.map(lambda r: inspect(r, tool_sha, args.refresh_disassembly), rows))
    counts = Counter()
    summaries = []
    for file in files:
        categories = Counter(p['import']['api_category'] for e in file['exports'] for p in e.get('api_paths', []))
        counts.update(categories)
        summaries.append({'path': file['path'], 'sha256': file['sha256'], 'machine': file['machine'],
                          'status': file['status'], 'semantic_status': file['semantic_status'],
                          'exports': len(file['exports']), 'analyzed_graph_entries': len(file['functions']),
                          'exports_with_api_paths': sum(bool(e.get('api_paths')) for e in file['exports']),
                          'exports_with_unresolved_indirect_calls': sum(bool(e.get('unresolved_indirect_call_count'))
                                                                       for e in file['exports']),
                          'api_path_categories': dict(sorted(categories.items()))})
    summary = {'unique_current_pe_files': len(files), 'aliases': sum(len(f['aliases']) for f in files),
               'product_dll_files': sum(any(a['origin'] == 'current_product_manifest' for a in f['aliases']) for f in files),
               'host_common_dll_files': sum(any(a['origin'] == 'current_host_common_dll' for a in f['aliases']) for f in files),
               'exports': sum(len(f['exports']) for f in files),
               'analyzed_graph_entries': sum(len(f['functions']) for f in files),
               'syntactic_api_paths': sum(counts.values()), 'api_path_categories': dict(sorted(counts.items())),
               'entire_dll_semantics_completed': 0, 'device_commands_sent': 0}
    result = {'schema_version': 1, 'generator_sha256': digest(Path(__file__).read_bytes()),
              'method': 'Byte-verified current PE + dumpbin static instructions + bounded intraprocedural CFG and direct-call traversal',
              'disassembler_sha256': tool_sha, 'inputs': inputs, 'summary': summary, 'files': files,
              'limitations': ['Direct paths are syntactic only; argument types, path feasibility and protocol bytes remain unresolved.',
                              'Dynamic calls, virtual dispatch, callback entrypoints and initialization reachability are not closed.',
                              'Graphs start at exports, not every internal routine or node registration callback.',
                              'CLR IL is not decoded; native exports do not establish managed internal semantics.',
                              'ARM64 bytes are verified but architecture is not analyzed here.',
                              'No application, DLL, vendor JavaScript or device command was executed.']}
    if args.only:
        output = CACHE / 'selected-inspection.json.gz'
        output.write_bytes(gzip.compress(json.dumps(result, ensure_ascii=False).encode(), mtime=0))
    else:
        encoded = (json.dumps(result, ensure_ascii=False, separators=(',', ':'))+'\n').encode()
        packed = gzip.compress(encoded, mtime=0)
        brief = {'schema_version': 1, 'method': result['method'], 'inputs': inputs,
                 'generator_sha256': result['generator_sha256'], 'disassembler_sha256': tool_sha,
                 'evidence': {'path': OUT.relative_to(ROOT).as_posix(), 'sha256': digest(packed),
                              'bytes': len(packed), 'decompressed_sha256': digest(encoded)},
                 'summary': summary, 'files': summaries, 'limitations': result['limitations']}
        brief_text = json.dumps(brief, ensure_ascii=False, indent=2)+'\n'
        if args.check:
            assert OUT.read_bytes() == packed, 'Stale machine-code graph evidence'
            assert SUMMARY.read_text('utf-8') == brief_text, 'Stale machine-code summary'
        else:
            OUT.write_bytes(packed)
            SUMMARY.write_text(brief_text, encoding='utf-8')
    print(json.dumps(summary), flush=True)


if __name__ == '__main__':
    main()
