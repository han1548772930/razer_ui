"""Acquire current host helper PE bytes from the verified official container.

7-Zip only reads the container. Helpers and vendor JavaScript never execute.
Acquisition, PE metadata and string locators are not full function semantics.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import subprocess

import pefile

ROOT = Path(__file__).resolve().parents[1]
HOST = ROOT / '.ref/host-4.0.827'
OUTPUT = ROOT / 'docs/re/host-helpers-current-evidence.json'
NAMES = ['RzPowerTool', 'RzSecurityTool', 'RzEngineMon', 'RzHandle']


def sha(body):
    return hashlib.sha256(body).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    chain = json.loads((HOST / 'ARCHIVE-CHAIN.json').read_bytes())
    # The verified package sits beside `win-unpacked`, while the ASAR is three
    # levels below that directory.  Keep the path derived from the receipt.
    package = (ROOT / chain['asar']['source_archive']).parent.parent.parent / chain['internal_exe']['entry']
    body = package.read_bytes()
    assert len(body) == chain['internal_exe']['bytes']
    assert sha(body) == chain['internal_exe']['sha256']
    listing = (HOST / 'source-evidence/inner-archive-list.txt').read_text('utf-8')
    source_files = []
    for path in sorted((HOST / 'electron').rglob('*.js')):
        raw = path.read_bytes()
        text = raw.decode('utf-8')
        if any(name in text for name in NAMES):
            source_files.append((path, raw, text))
    rows = []
    for name in NAMES:
        entry = 'win-unpacked/CommonDLL/' + name + '.exe'
        found = re.search(r'Path = ' + re.escape(entry.replace('/', '\\')) +
                          r'\r?\nFolder = -\r?\nSize = (\d+)', listing)
        assert found, entry
        extracted = subprocess.run(
            ['C:/Program Files/7-Zip/7z.exe', 'e', '-so', '-bd', str(package), entry],
            capture_output=True, check=False)
        assert extracted.returncode in (0, 1) and extracted.stdout, extracted.stderr
        raw = extracted.stdout
        assert len(raw) == int(found.group(1)) and raw.startswith(b'MZ')
        digest = sha(raw)
        target = ROOT / '.ref/host-helper-libraries' / digest / (name + '.exe')
        assert target.resolve().is_relative_to((ROOT / '.ref/host-helper-libraries').resolve())
        if target.exists():
            assert target.read_bytes() == raw, 'Preserving conflicting evidence: ' + str(target)
        else:
            assert not args.check, 'Missing current helper: ' + str(target)
            target.parent.mkdir(parents=True, exist_ok=True)
            with target.open('xb') as output:
                output.write(raw)
        pe = pefile.PE(data=raw)
        assert pe.FILE_HEADER.Machine == 0x8664
        strings = []
        for match in re.finditer(rb'(?:[\x20-\x7e]\x00){5,}', raw):
            text = match.group().decode('utf-16-le')
            if text.startswith('--') and re.fullmatch(r'--[a-z][a-z=-]*', text):
                strings.append({'text':text,'file_offset':match.start(),
                                'rva':pe.get_rva_from_offset(match.start()),'encoding':'utf-16-le'})
        callers = []
        for path, source, text in source_files:
            for match in re.finditer(re.escape(name + '.exe'), text):
                start, end = max(0, match.start()-200), min(len(text), match.end()+300)
                callers.append({'path':path.relative_to(ROOT).as_posix(), 'sha256':sha(source),
                                'offset_utf16':len(text[:start].encode('utf-16-le'))//2,
                                'end_utf16':len(text[:end].encode('utf-16-le'))//2,
                                'source_context_locator':text[start:end]})
        versions = {}
        for group in getattr(pe, 'FileInfo', []):
            for info in group:
                if info.Key == b'StringFileInfo':
                    for table in info.StringTable:
                        versions.update({k.decode('utf-8'):v.decode('utf-8') for k,v in table.entries.items()})
        rows.append({'name':name,'path':target.relative_to(ROOT).as_posix(),
                     'archive_entry':entry,'bytes':len(raw),'sha256':digest,
                     'machine':pe.FILE_HEADER.Machine,'image_base':pe.OPTIONAL_HEADER.ImageBase,
                     'entry_rva':pe.OPTIONAL_HEADER.AddressOfEntryPoint,'version_resource':versions,
                     'clr_directory_present':bool(pe.OPTIONAL_HEADER.DATA_DIRECTORY[14].VirtualAddress),
                     'exception_function_entries':len(getattr(pe,'DIRECTORY_ENTRY_EXCEPTION',[])),
                     'imports':[{'library':dll.dll.decode('ascii'),
                                 'symbols':[{'name':sym.name.decode('ascii') if sym.name else None,
                                             'ordinal':sym.ordinal,
                                             'iat_rva':sym.address-pe.OPTIONAL_HEADER.ImageBase}
                                            for sym in dll.imports]}
                                for dll in getattr(pe,'DIRECTORY_ENTRY_IMPORT',[])],
                     'command_string_locators':strings,'current_js_callers':callers,
                     'stages':{'acquisition':'verified archive bytes and PE metadata',
                               'function_semantics':'requires branch-by-branch disassembly; strings are locators only',
                               'rust_replacement':'not established by this acquisition audit',
                               'runtime_acceptance':'not executed'}})
    result = {'schema_version':1,'generator_sha256':sha(Path(__file__).read_bytes()),
              'host_version':'4.0.827','method':'static 7-Zip container read, exact official inner archive SHA-256, PE parse and current JS locators',
              'source_archive':{'path':package.relative_to(ROOT).as_posix(),
                                'sha256':chain['internal_exe']['sha256'],'bytes':len(body)},
              'helpers':rows,'full_helper_reverse_engineering_complete':False}
    encoded = json.dumps(result, ensure_ascii=False, indent=2) + '\n'
    if args.check:
        assert OUTPUT.read_text('utf-8') == encoded, OUTPUT
    else:
        OUTPUT.write_text(encoded, encoding='utf-8', newline='\n')
    print(f'Current host helpers: {len(rows)} exact-byte PE files; {sum(len(r["command_string_locators"]) for r in rows)} CLI string locators; no execution')


if __name__ == '__main__':
    main()
