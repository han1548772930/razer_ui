"""Static disassembly of current host service exports and their code references.

Only dumpbin reads DLL bytes. No DLL is loaded and no export is executed.
Unwind metadata provides ranges; leaf/thunk windows are explicitly bounded.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess
import uuid
import pefile

ROOT = Path(__file__).resolve().parents[1]
DUMPBIN = Path(r'C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Tools\MSVC\14.42.34433\bin\Hostx64\x64\dumpbin.exe')
OUTPUT = ROOT / 'docs/re/host-service-machine-code-current-evidence.json'
SELECTED = {
    'mapping_engine.dll': ['mappingEngineInitialize', 'mappingEngineShutdown', 'getGlobalMode', 'getGlobalShortcuts'],
    'simple_service.dll': ['simpleServiceInitialize', 'simpleServiceShutdown', 'simpleGetVersionInfo', 'simpleEnumerateAudioDevices'],
}
# These slot identities are independently tied to the singleton constructor and
# the indirect load in each export. They are not inferred from a method name.
VTABLE = {
    'mapping_engine.dll': dict(table=0x2df010, constructor=0x1060,
        slots={'mappingEngineInitialize':8, 'mappingEngineShutdown':0x10, 'getGlobalMode':0x338, 'getGlobalShortcuts':0x348}),
    'simple_service.dll': dict(table=0x1f2040, constructor=0x1000,
        slots={'simpleServiceInitialize':8, 'simpleServiceShutdown':0x10, 'simpleGetVersionInfo':0x38, 'simpleEnumerateAudioDevices':0xb0}),
}


def inspect(resource):
    target = ROOT / resource['path']
    raw = target.read_bytes()
    assert hashlib.sha256(raw).hexdigest() == resource['sha256']
    pe = pefile.PE(data=raw)
    assert pe.FILE_HEADER.Machine == 0x8664
    base = pe.OPTIONAL_HEADER.ImageBase
    exports = {s.name.decode(): s.address for s in pe.DIRECTORY_ENTRY_EXPORT.symbols if s.name}
    ranges = [(e.struct.BeginAddress, e.struct.EndAddress) for e in pe.DIRECTORY_ENTRY_EXCEPTION]
    imports = {entry.address-base: entry.name.decode() if entry.name else f'ordinal:{entry.ordinal}'
               for library in getattr(pe, 'DIRECTORY_ENTRY_IMPORT', []) for entry in library.imports}

    def executable(rva):
        return any(s.VirtualAddress <= rva < s.VirtualAddress + s.Misc_VirtualSize and s.Characteristics & 0x20000000
                   for s in pe.sections)

    def inspect_range(rva):
        metadata = next(((a, b) for a, b in ranges if a == rva), None)
        containing = next(((a, b) for a, b in ranges if a <= rva < b), None)
        end = metadata[1] if metadata else min(containing[1], rva+96) if containing else rva+96
        # End is exclusive in our evidence; dumpbin treats its range end as inclusive.
        result = subprocess.run([str(DUMPBIN), '/disasm:nobytes', f'/range:{base+rva:#x},{base+end-1:#x}', str(target)],
                                capture_output=True, check=True, creationflags=subprocess.CREATE_NO_WINDOW)
        lines = []
        for line in result.stdout.decode(errors='replace').splitlines():
            m = re.match(r'\s*([0-9A-F]{16}):\s*(.*)', line)
            if m and base+rva <= int(m[1], 16) < base+end:
                lines.append(dict(rva=int(m[1], 16)-base, instruction=m[2].rstrip()))
        assert lines, hex(rva)
        if not metadata:
            terminator = next((i for i, line in enumerate(lines) if re.match(r'(?:ret\b|jmp\b)', line['instruction'])), None)
            if terminator is not None:
                # Last instruction length remains unproven without a decoder; retain window hash.
                lines = lines[:terminator+1]
        direct, code_refs, data_refs, indirect = [], [], [], []
        for line in lines:
            ins = line['instruction']
            m = re.match(r'(call|jmp)\s+([0-9A-F]{16})\b', ins)
            if m:
                dest = int(m[2], 16)-base
                direct.append(dict(at=line['rva'], operation=m[1], target_rva=dest))
            if not m and re.match(r'(?:call|jmp)\s+', ins):
                m = re.search(r'\[([0-9A-F]{16})h\]', ins)
                slot = int(m[1], 16)-base if m else None
                indirect.append(dict(at=line['rva'], instruction=ins, pointer_slot_rva=slot,
                                     imported_symbol=imports.get(slot)))
            m = re.match(r'lea\s+\w+,\[([0-9A-F]{16})h\]', ins)
            if not m:
                continue
            dest = int(m[1], 16)-base
            if executable(dest):
                code_refs.append(dict(at=line['rva'], target_rva=dest))
            elif dest >= 0:
                body = pe.get_data(dest, 256).split(b'\0', 1)[0]
                if len(body) >= 4 and all(32 <= c < 127 for c in body):
                    data_refs.append(dict(at=line['rva'], target_rva=dest, ascii=body.decode('ascii')))
        return dict(rva=rva,end_rva=end,boundary='AMD64 .pdata entry' if metadata else 'bounded leaf/thunk window; not a proven full function',
                    machine_code_sha256=hashlib.sha256(pe.get_data(rva,end-rva)).hexdigest(),
                    instructions=lines,direct_branches=direct,code_address_references=code_refs,
                    string_references=data_refs,indirect_branches=indirect)

    nodes = {}
    roots = []
    for name in SELECTED[resource['file']]:
        rva = exports[name]
        roots.append(dict(export=name,rva=rva))
        nodes[rva] = inspect_range(rva)
    # Inspect all explicit code-address references in export bodies (including callback closures).
    for node in list(nodes.values()):
        for ref in node['code_address_references']:
            rva = ref['target_rva']
            if rva not in nodes:
                nodes[rva] = inspect_range(rva)
    # Inspect direct helper calls/jumps one layer from exports and code references.
    # This bound is documented; recursion through arbitrary C++ internals is not claimed.
    for node in list(nodes.values()):
        for branch in node['direct_branches']:
            rva = branch['target_rva']
            if branch['operation'] == 'jmp' and not node['instructions'][0]['instruction'].startswith('jmp '):
                continue
            if executable(rva) and rva not in nodes:
                nodes[rva] = inspect_range(rva)
    # Follow jump thunks into one additional real function range.
    for node in list(nodes.values()):
        if not node['boundary'].startswith('bounded') or not node['instructions'][0]['instruction'].startswith('jmp '):
            continue
        for branch in node['direct_branches']:
            if branch['operation'] == 'jmp' and executable(branch['target_rva']) and branch['target_rva'] not in nodes:
                nodes[branch['target_rva']] = inspect_range(branch['target_rva'])
    table = VTABLE[resource['file']]
    nodes[table['constructor']] = inspect_range(table['constructor'])
    constructor_code = '\n'.join(line['instruction'] for line in nodes[table['constructor']]['instructions'])
    assert f'{base+table["table"]:016X}h' in constructor_code
    virtual_targets = []
    for name, slot in table['slots'].items():
        address = int.from_bytes(pe.get_data(table['table']+slot,8),'little')
        rva = address-base
        assert executable(rva)
        nodes[rva] = inspect_range(rva)
        virtual_targets.append(dict(export=name,table_rva=table['table'],slot_offset=slot,target_rva=rva,
                                    constructor_rva=table['constructor']))
    for virtual_entry in virtual_targets:
        node = nodes[virtual_entry['target_rva']]
        for ref in node['code_address_references']:
            if ref['target_rva'] not in nodes:
                nodes[ref['target_rva']] = inspect_range(ref['target_rva'])
    selected_service_helpers = [0xc352a,0xc0124,0xbfe62,0xbec9a,0x1bf9f0,0x1b5020] if resource['file'] == 'mapping_engine.dll' else [0x21ad2,0x32082,0x25922,0x6cb52,0x24590,0x25890,0x1d4ba,0x28200,0x39070,0x397aa,0x3e84c,0x3e864]
    for rva in selected_service_helpers:
        nodes[rva] = inspect_range(rva)
        for ref in nodes[rva]['code_address_references']:
            if ref['target_rva'] not in nodes:
                nodes[ref['target_rva']] = inspect_range(ref['target_rva'])
    result = dict(path=resource['path'],sha256=resource['sha256'],bytes=len(raw),machine='AMD64',image_base=base,
                  roots=roots,virtual_targets=virtual_targets,code_ranges=[nodes[rva] for rva in sorted(nodes)],
                  external_imports=[dict(slot_rva=rva,name=name) for rva,name in sorted(imports.items())],
                  caveat='Bounded graph: exports, explicit code references, one helper layer, one thunk hop, constructor-proven singleton vtable slots and their explicit task code references. Other virtual targets are unresolved. No full DLL semantic recovery.')
    if resource['file'] == 'mapping_engine.dll':
        record = nodes[0xbec9a]
        decimal = nodes[0x1bf9f0]
        text = '\n'.join(line['instruction'] for line in decimal['instructions'])
        assert '0CCCCCCCCCCCCCCCDh' in text and 'shr         rdx,3' in text
        assert 'or          r9b,30h' in text and 'ja          ' in text
        assert 'rdx,qword ptr [rdi+68h]' in '\n'.join(line['instruction'] for line in record['instructions'])
        assert any(b['target_rva'] == 0x1bf9f0 for b in record['direct_branches'])
        assert any(b['target_rva'] == 0x1b5020 for b in record['direct_branches'])
        result['mode_record_time_tick'] = dict(record_serializer_rva=0xbec9a,
            source_qword_offset=0x68, decimal_string_constructor_rva=0x1bf9f0,
            json_value_insertion_rva=0x1b5020,
            observed_transform='Unsigned 64-bit decimal digits by repeated division by 10, ASCII 0..9, then a null-terminated small/heap string object passed to the JSON insertion method',
            caveat='Representation transform proved; origin, clock, epoch and tick unit remain unresolved. No device value or serialization runtime executed.')
    if resource['file'] == 'simple_service.dll':
        result['audio_com_creation'] = dict(helper_rva=0x3e864,
            class_guid=dict(rva=0x1f5b50,value=str(uuid.UUID(bytes_le=pe.get_data(0x1f5b50,16)))),
            interface_guid=dict(rva=0x1f5b40,value=str(uuid.UUID(bytes_le=pe.get_data(0x1f5b40,16)))),
            imports=['CoCreateInstance','CoInitializeEx'],
            caveat='Machine code contains Core Audio creation/enumeration implementation; query-thread dynamic instance target still requires exact construction/dispatch tracing')
    pe.close()
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check',action='store_true')
    args = parser.parse_args()
    source = ROOT / 'docs/re/host-native-library-pe-current-evidence.json'
    inventory = json.loads(source.read_text('utf-8'))
    rows = [inspect(row) for row in inventory['files'] if row['file'] in SELECTED]
    asar_source = ROOT / 'docs/re/host-full-asar-current-evidence.json'
    asar = json.loads(asar_source.read_text('utf-8'))
    addons = []
    for item in asar['files']:
        if not item['entry'].endswith('.node'):
            continue
        raw = (ROOT / item['path']).read_bytes()
        assert hashlib.sha256(raw).hexdigest() == item['sha256']
        row = dict(path=item['path'],entry=item['entry'],sha256=item['sha256'],bytes=len(raw),
                   archive_entry=item['archive_entry'],unpacked=item['unpacked'],
                   asar_integrity_verified=item['asar_integrity_verified'],format_magic=raw[:4].hex(),
                   internal_implementation='not_semantically_reversed')
        if raw.startswith(b'MZ'):
            pe = pefile.PE(data=raw)
            row.update(machine=pe.FILE_HEADER.Machine,format='PE',
                       exports=[dict(name=e.name.decode() if e.name else None,rva=e.address,
                                     forwarder=e.forwarder.decode() if e.forwarder else None)
                                for e in getattr(getattr(pe,'DIRECTORY_ENTRY_EXPORT',None),'symbols',[])],
                       imports=[dict(library=library.dll.decode(),symbols=[s.name.decode() if s.name else s.ordinal for s in library.imports])
                                for library in getattr(pe,'DIRECTORY_ENTRY_IMPORT',[])])
            pe.close()
        else:
            row.update(format='ELF' if raw.startswith(b'\x7fELF') else 'non-PE native format; not decoded here',machine=None,exports=None,imports=None)
        addons.append(row)
    result = dict(schema_version=1,date='2026-10-09',host_version='4.0.827',
                  method='Byte-verified current CommonDLL AMD64 PE export/.pdata inspection and dumpbin static disassembly; no DLL load or vendor code execution',
                  source=dict(path=source.relative_to(ROOT).as_posix(),sha256=hashlib.sha256(source.read_bytes()).hexdigest()),files=rows,
                  addon_source=dict(path=asar_source.relative_to(ROOT).as_posix(),sha256=hashlib.sha256(asar_source.read_bytes()).hexdigest()),
                  packaged_native_addons=addons)
    encoded = json.dumps(result,ensure_ascii=False,indent=2)+'\n'
    if args.check:
        assert OUTPUT.read_text('utf-8') == encoded,'Stale host service machine-code evidence'
    else:
        OUTPUT.write_text(encoded,encoding='utf-8')
    print(f'Static host service code: {len(rows)} DLLs, {sum(len(r["roots"]) for r in rows)} exports, {sum(len(r["code_ranges"]) for r in rows)} code ranges; indirect targets remain explicit')


if __name__ == '__main__':
    main()
