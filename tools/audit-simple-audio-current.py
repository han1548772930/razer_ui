"""Current simple_service audio-chain evidence using IDA/Hex-Rays only.

--refresh statically analyzes a hash-verified private PE copy with local IDA.
--check verifies retained code ranges against original bytes and consumer links.
Neither mode loads the target as a library or executes target instructions.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import uuid

import pefile

ROOT = Path(__file__).resolve().parents[1]
SOURCE = '.ref/host-4.0.827/native-evidence/CommonDLL/simple_service.dll'
SHA = 'f8e3886c1d83e37accebd40d4b72d5f26c1d3b5d09a6b72707a27f089c196f2b'
OUTPUT = ROOT / 'docs/re/simple-audio-current-evidence.json'
FUNCTIONS = {
    'export': 0x16d40, 'singleton_method': 0x4cb0,
    'audio_task': 0x32082, 'query_dispatch': 0x32270,
    'create_instance_task': 0x31db0, 'win_instance_factory': 0x38f60,
    'initialize': 0x39070, 'initial_endpoint_population': 0x397aa,
    'list_json': 0x3a170, 'io_callback': 0x25a00,
    'add_endpoint': 0x3b270, 'endpoint_constructor': 0x36f30,
    'endpoint_initialize': 0x37144, 'endpoint_name': 0x3ef82,
    'endpoint_id': 0x3ee43, 'endpoint_name_by_id': 0x3f128,
    'container_id': 0x3f236, 'endpoint_flow': 0x3f95a,
    'activate_volume': 0x3fb86, 'create_enumerator': 0x3e864,
    'hash': 0x3b820, 'hash_0_to_16': 0x3bae4,
    'hash_17_to_32': 0x3bbc2, 'hash_33_to_64': 0x3bc4e,
    'map_insert': 0x3be88, 'map_rehash': 0x1f746,
    'bucket_count': 0x1f5fe, 'guid_text': 0x75c10,
    'suffix_lowercase': 0x1d2d94, 'suffix_lowercase_body': 0x1d2de8,
    'utf16_to_utf8_wrapper': 0x86130, 'utf16_to_utf8': 0x85890,
}


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def refresh(ida, python_dir):
    source = ROOT / SOURCE
    raw = source.read_bytes()
    assert sha(raw) == SHA
    work = ROOT / '.work/ida-static/simple-service'
    work.mkdir(parents=True, exist_ok=True)
    target = work / 'simple_service.dll'
    shutil.copyfile(source, target)
    output = work / 'current-evidence.json'
    config = work / 'current-functions.json'
    config.write_text(json.dumps({'expected_sha256': SHA, 'output': str(output),
                                  'function_rvas': list(FUNCTIONS.values())}), encoding='utf-8')
    env = dict(os.environ)
    if python_dir:
        python_dir = str(Path(python_dir).resolve())
        env['PATH'] = python_dir + os.pathsep + env['PATH']
        env['PYTHONHOME'] = python_dir
    subprocess.run([str(Path(ida).resolve()), '-A', '-L'+str(work/'current-analysis.log'),
                    '-S'+str(ROOT/'tools/ida-static-functions.py')+' '+str(config), str(target)],
                   cwd=work, env=env, check=True, creationflags=subprocess.CREATE_NO_WINDOW,
                   stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
    assert sha(source.read_bytes()) == SHA
    analysis = json.loads(output.read_text('utf-8'))
    assert analysis['input_sha256'] == SHA and analysis['hexrays_available']
    analysis['source'] = {'path': SOURCE, 'sha256': SHA, 'bytes': len(raw)}
    analysis['function_roles'] = FUNCTIONS
    analysis['implementation'] = [
        'crates/razer-device/src/simple_audio.rs',
        'crates/razer-service/src/simple_audio.rs',
        'crates/razer-service/src/runtime/mod.rs',
        'crates/razer-service/src/runtime/portable.rs',
    ]
    analysis['consumers'] = ['crates/razer-settings/src/runtime_page.rs',
                             'crates/razer-pages/src/features/control_pod_audio.rs']
    analysis['runtime_acceptance'] = 'not_run'
    OUTPUT.write_text(json.dumps(analysis, ensure_ascii=False, indent=2)+'\n', encoding='utf-8')


def check():
    evidence = json.loads(OUTPUT.read_text('utf-8'))
    raw = (ROOT/SOURCE).read_bytes()
    assert sha(raw) == SHA == evidence['input_sha256']
    pe = pefile.PE(data=raw)
    base = pe.OPTIONAL_HEADER.ImageBase
    assert pe.FILE_HEADER.Machine == 0x8664 and evidence['image_base'] == base
    boundaries = {e.struct.BeginAddress: e.struct.EndAddress for e in pe.DIRECTORY_ENTRY_EXCEPTION}
    nodes = {f['rva']: f for f in evidence['functions']}
    assert evidence['function_roles'] == FUNCTIONS and set(nodes) == set(FUNCTIONS.values())
    for start, node in nodes.items():
        if start in boundaries:
            assert boundaries[start] == node['end_rva']
        else:
            # These are leaf hash/CRT thunks; retain IDA-inferred boundaries
            # explicitly instead of claiming nonexistent unwind metadata.
            assert start in {0x3bae4, 0x3bbc2, 0x1d2d94}
            assert node['instructions'][-1]['text'].split()[0] in {'retn', 'ret', 'jmp'}
        assert sha(pe.get_data(start, node['end_rva']-start)) == node['code_sha256']
        assert node['pseudocode'] and not node['decompiler_error']
        assert all(start <= i['rva'] < node['end_rva'] for i in node['instructions'])
    table = 0x1f54d0
    assert struct.unpack('<Q', pe.get_data(table+0x20, 8))[0]-base == 0x39070
    assert struct.unpack('<Q', pe.get_data(table+0x30, 8))[0]-base == 0x3a170
    for rva, text in [(0x1f39f8, 'type'), (0x1f39cd, 'id'),
                      (0x1f3897, 'containerId'), (0x1f39dc, 'name')]:
        assert pe.get_data(rva, len(text)+1) == text.encode()+b'\0'
    assert str(uuid.UUID(bytes_le=pe.get_data(0x1fbbf8,16))) == '8c7ed206-3f8a-4827-b3ab-ae9e1faefc6c'
    assert int.from_bytes(pe.get_data(0x1fbbf8+16,4),'little') == 2
    for path in evidence['implementation']+evidence['consumers']:
        assert (ROOT/path).is_file()
    portable = (ROOT/evidence['implementation'][3]).read_text('utf-8')
    assert 'ServiceRequest::AudioDevices' in portable and 'crate::simple_audio::enumerate()' in portable
    for path in evidence['consumers']:
        assert 'ServiceRequest::AudioDevices' in (ROOT/path).read_text('utf-8')
    adapter = (ROOT/evidence['implementation'][1]).read_text('utf-8')
    assert 'EngineLibrary' not in adapter and 'libloading' not in adapter
    print(f'Current IDA audio evidence: {len(nodes)} function bodies, source bytes/vtable/schema/consumers verified; runtime not run')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--refresh', action='store_true')
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--ida', default=r'D:\ida\IDA Pro 8.3\idat64.exe')
    parser.add_argument('--python-dir', default=str(ROOT/'.work/ida-static/python311'))
    args = parser.parse_args()
    if args.refresh:
        refresh(args.ida, args.python_dir)
    check()
