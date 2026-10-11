"""Acquire current installed mapping engine as inert private bytes for IDA.

Only PE header/export parsing, hashing and copying; no loading or export calls.
The input is required to belong to the explicitly current app-4.0.827 directory.
"""
import hashlib
import json
from pathlib import Path
import struct

ROOT = Path(__file__).resolve().parents[1]
SOURCE = Path('C:/Program Files/Razer/RazerAppEngine/app-4.0.827/CommonDLL/mapping_engine.dll')
WORK = ROOT / '.work/ida-mapping-engine-current'
OUTPUT = ROOT / 'docs/re/evidence/mapping-engine-storage-ida-current.json'


def rva_bytes(raw, rva, size):
    pe = struct.unpack_from('<I', raw, 0x3c)[0]
    count = struct.unpack_from('<H', raw, pe+6)[0]
    optional_size = struct.unpack_from('<H', raw, pe+20)[0]
    section_table = pe+24+optional_size
    for index in range(count):
        entry = section_table+index*40
        virtual_size, start, raw_size, raw_offset = struct.unpack_from('<IIII', raw, entry+8)
        if start <= rva < start+max(raw_size, virtual_size):
            assert rva-start+size <= raw_size
            return raw[raw_offset+rva-start:raw_offset+rva-start+size]
    raise ValueError(f'Unmapped RVA {rva:x}')


def pe_exports(raw):
    def u16(offset): return struct.unpack_from('<H', raw, offset)[0]
    def u32(offset): return struct.unpack_from('<I', raw, offset)[0]
    pe = u32(0x3c)
    assert raw[:2] == b'MZ' and raw[pe:pe+4] == b'PE\0\0'
    machine, sections = u16(pe+4), u16(pe+6)
    optional_size = u16(pe+20)
    optional = pe+24
    assert machine == 0x8664 and u16(optional) == 0x20b, 'Expected current x64 PE'
    section_table = optional + optional_size
    table = []
    for index in range(sections):
        entry = section_table + index*40
        table.append((u32(entry+12), u32(entry+8), u32(entry+20), u32(entry+16)))
    def offset(rva):
        for start, virtual_size, raw_offset, raw_size in table:
            if start <= rva < start + max(virtual_size, raw_size):
                assert rva-start < raw_size
                return raw_offset + rva-start
        if rva < u32(optional+60): return rva
        raise ValueError(f'Unmapped RVA {rva:x}')
    export_rva, export_size = u32(optional+112), u32(optional+116)
    export = offset(export_rva)
    base, count, named = u32(export+16), u32(export+20), u32(export+24)
    functions, names, ordinals = offset(u32(export+28)), offset(u32(export+32)), offset(u32(export+36))
    entries = []
    for index in range(named):
        name_offset = offset(u32(names+index*4))
        name_end = raw.index(0, name_offset)
        ordinal_index = u16(ordinals+index*2)
        assert ordinal_index < count
        rva = u32(functions+ordinal_index*4)
        entries.append({'name': raw[name_offset:name_end].decode('ascii'),
                        'ordinal': base+ordinal_index, 'rva': rva,
                        'forwarded': export_rva <= rva < export_rva+export_size})
    return machine, entries


def main():
    raw = SOURCE.read_bytes()
    digest = hashlib.sha256(raw).hexdigest()
    machine, exports = pe_exports(raw)
    wanted = ['localStorageSetItem', 'localStorageGetItem', 'localStorageRemoveItem']
    selected = [entry for entry in exports if entry['name'] in wanted]
    assert any(entry['name'] == 'localStorageSetItem' for entry in selected)
    assert all(not entry['forwarded'] for entry in selected)
    WORK.mkdir(parents=True, exist_ok=True)
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    private = WORK/'mapping_engine.dll'
    private.write_bytes(raw)
    assert hashlib.sha256(private.read_bytes()).hexdigest() == digest
    receipt = {'method': 'Static PE export parsing and private byte copy; no target execution',
               'source': str(SOURCE), 'host_version': '4.0.827', 'sha256': digest,
               'bytes': len(raw), 'machine': machine,
               'private_input': private.relative_to(ROOT).as_posix(),
               'exports': exports, 'selected_exports': selected,
               'semantic_completion': False, 'runtime_acceptance': 'not_run'}
    (ROOT/'docs/re/mapping-engine-native-current-acquisition.json').write_text(
        json.dumps(receipt, indent=2)+'\n', encoding='utf8')
    config = {'expected_sha256': digest, 'output': str(OUTPUT),
              'function_rvas': [entry['rva'] for entry in selected]}
    (WORK/'config.json').write_text(json.dumps(config, indent=2)+'\n', encoding='utf8')
    print(json.dumps({'sha256': digest, 'bytes': len(raw), 'selected_exports': selected}))


if __name__ == '__main__':
    main()
