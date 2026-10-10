"""Static IDA/Hex-Rays coverage of every currently receipted PE and host helper.

Uses private copies, no debugger, imports, target export calls or execution.
Coverage is evidence acquisition, never semantic/implementation completion.
"""
import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

import pefile

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'docs/re/ida-native-corpus-current.json'
EVIDENCE = ROOT / 'docs/re/evidence/ida-native'
WORK = ROOT / '.work/ida-static/corpus'
EXPORTER = ROOT / 'tools/ida-static-functions.py'


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def inputs():
    summary = json.loads((ROOT/'docs/re/dll-device-communication-current-summary.json').read_text('utf-8'))
    helpers = json.loads((ROOT/'docs/re/host-helpers-current-evidence.json').read_text('utf-8'))
    rows = summary['files'] + helpers['helpers']
    for row in rows:
        raw = (ROOT/row['path']).read_bytes()
        assert sha(raw) == row['sha256'], row['path']
        pe = pefile.PE(data=raw, fast_load=True)
        assert pe.FILE_HEADER.Machine == row['machine']
        yield {'path': row['path'], 'sha256': row['sha256'], 'bytes': len(raw),
               'machine': row['machine'], 'entry_rva': pe.OPTIONAL_HEADER.AddressOfEntryPoint,
               'clr_directory_present': bool(pe.OPTIONAL_HEADER.DATA_DIRECTORY[14].VirtualAddress)}


def summarize(row, analysis, destination):
    return {**row, 'status': 'static_analysis_acquired',
            'ida_version': analysis['ida_version'],
            'hexrays_available': analysis['hexrays_available'],
            'function_index_count': len(analysis['function_index']),
            'selected_function_bodies': len(analysis['functions']),
            'decompiled_bodies': sum(bool(f['pseudocode']) for f in analysis['functions']),
            'decompiler_failures': sum(bool(f['decompiler_error']) for f in analysis['functions']),
            'unresolved_entry_count': len(analysis['unresolved_entries']),
            'selection': analysis['selection'],
            'evidence': {'path': destination.relative_to(ROOT).as_posix(),
                         'sha256': sha(destination.read_bytes())},
            'semantic_completion': False, 'rust_replacement_completion': False,
            'runtime_acceptance': 'not_run'}


def analyze(row, args, exporter_sha):
    if row['clr_directory_present'] or row['machine'] not in (0x8664, 0x14c):
        return {**row, 'status': 'unsupported_static_backend',
                'reason': 'CLR IL or ARM64 requires a matching static decompiler; x64 Hex-Rays evidence is not a substitute',
                'semantic_completion': False, 'rust_replacement_completion': False,
                'runtime_acceptance': 'not_run'}
    destination = EVIDENCE / (row['sha256'] + '.json.gz')
    if destination.exists():
        analysis = json.loads(gzip.decompress(destination.read_bytes()))
        if analysis.get('exporter_sha256') == exporter_sha:
            assert analysis['input_sha256'] == row['sha256']
            return summarize(row, analysis, destination)
    directory = WORK / row['sha256']
    directory.mkdir(parents=True, exist_ok=True)
    target = directory / Path(row['path']).name
    shutil.copyfile(ROOT/row['path'], target)
    config = directory / 'config.json'
    result_path = directory / 'analysis.json'
    config.write_text(json.dumps({'expected_sha256': row['sha256'], 'output': str(result_path),
                                  'corpus_mode': True, 'entry_rvas': [row['entry_rva']]}), encoding='utf-8')
    env = dict(os.environ)
    python_dir = str(Path(args.python_dir).resolve())
    env['PATH'] = python_dir + os.pathsep + env['PATH']
    env['PYTHONHOME'] = python_dir
    started = time.monotonic()
    command = [str(Path(args.ida).resolve()), '-A', '-L'+str(directory/'ida.log'),
               '-S'+str(EXPORTER)+' '+str(config), str(target)]
    try:
        process = subprocess.run(command, cwd=directory, env=env,
                                 creationflags=subprocess.CREATE_NO_WINDOW,
                                 stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=1800)
    except subprocess.TimeoutExpired:
        return {**row, 'status': 'analysis_failed', 'reason': 'IDA exceeded 1800s analysis budget',
                'runtime_acceptance': 'not_run'}
    assert sha((ROOT/row['path']).read_bytes()) == row['sha256']
    if process.returncode != 0 or not result_path.exists():
        return {**row, 'status': 'analysis_failed', 'reason': 'IDA exporter failed',
                'exit_code': process.returncode, 'log': (directory/'ida.log').relative_to(ROOT).as_posix(),
                'runtime_acceptance': 'not_run'}
    analysis = json.loads(result_path.read_text('utf-8'))
    assert analysis['input_sha256'] == row['sha256']
    analysis['source'] = row
    analysis['exporter_sha256'] = exporter_sha
    analysis['runtime_acceptance'] = 'not_run'
    EVIDENCE.mkdir(parents=True, exist_ok=True)
    destination.write_bytes(gzip.compress((json.dumps(analysis, ensure_ascii=False, indent=2)+'\n').encode(), mtime=0))
    print(f"  {len(analysis['function_index'])} indexed / {len(analysis['functions'])} bodies, {time.monotonic()-started:.1f}s", flush=True)
    return summarize(row, analysis, destination)


def check(report, rows):
    assert report['exporter_sha256'] == sha(EXPORTER.read_bytes())
    assert [r['path'] for r in report['files']] == [r['path'] for r in rows]
    for current, row in zip(rows, report['files']):
        for key, value in current.items():
            assert row[key] == value, (current['path'], key)
        if row['status'] != 'static_analysis_acquired':
            continue
        path = ROOT/row['evidence']['path']
        assert sha(path.read_bytes()) == row['evidence']['sha256']
        analysis = json.loads(gzip.decompress(path.read_bytes()))
        assert analysis['input_sha256'] == row['sha256']
        pe = pefile.PE(data=(ROOT/row['path']).read_bytes(), fast_load=True)
        base = pe.OPTIONAL_HEADER.ImageBase
        assert analysis['image_base'] == base
        for function in analysis['functions']:
            start, end = function['rva'], function['end_rva']
            assert sha(pe.get_data(start, end-start)) == function['code_sha256'], (row['path'], start)
        assert summarize(current, analysis, path) == row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--ida', default=r'D:\ida\IDA Pro 8.3\idat64.exe')
    parser.add_argument('--python-dir', default=str(ROOT/'.work/ida-static/python311'))
    args = parser.parse_args()
    rows = list(inputs())
    if args.check:
        report = json.loads(OUTPUT.read_text('utf-8'))
        check(report, rows)
        print(json.dumps(report['summary']))
        return
    exporter_sha = sha(EXPORTER.read_bytes())
    files = []
    for index, row in enumerate(rows):
        print(f"[{index+1}/{len(rows)}] {row['path']}", flush=True)
        files.append(analyze(row, args, exporter_sha))
        report = {'schema_version': 1,
                  'method': 'IDA autoanalysis and Hex-Rays on hash-verified private copies; no target execution',
                  'exporter_sha256': exporter_sha,
                  'scope': 'All 81 current PE receipts plus 4 acquired official host helpers',
                  'selection_limit': 'Full function index; selected bodies only. Indirect calls, unselected bodies, branch semantics and consumers require further recovery.',
                  'summary': {'receipted_pe_files': len(rows), 'processed_files': len(files),
                              'ida_acquired': sum(f['status']=='static_analysis_acquired' for f in files),
                              'analysis_failures': sum(f['status']=='analysis_failed' for f in files),
                              'unsupported': sum(f['status']=='unsupported_static_backend' for f in files),
                              'indexed_functions': sum(f.get('function_index_count',0) for f in files),
                              'selected_function_bodies': sum(f.get('selected_function_bodies',0) for f in files),
                              'decompiled_bodies': sum(f.get('decompiled_bodies',0) for f in files),
                              'whole_library_replacements_completed': 0},
                  'files': files, 'runtime_acceptance': 'not_run'}
        OUTPUT.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
    check(report, rows)
    print(json.dumps(report['summary']))


if __name__ == '__main__':
    main()
