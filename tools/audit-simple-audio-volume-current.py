"""Verify current simple_service volume reads and mutations with IDA evidence.

--acquire runs installed IDA/Hex-Rays statically on a private input copy.
No target exports, debugger, system audio calls or vendor JavaScript execute.
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
OUTPUT = ROOT/'docs/re/simple-audio-volume-current-evidence.json'
FUNCTIONS = {
    'get_microphone_export': 0x179f0, 'set_microphone_export': 0x17dc0,
    'get_speaker_export': 0x19750, 'set_speaker_export': 0x19b20,
    'get_microphone_callback': 0x17700, 'set_microphone_callback': 0x17bf0,
    'get_speaker_callback': 0x19460, 'set_speaker_callback': 0x19950,
    'singleton_get_microphone': 0x5550, 'singleton_set_microphone': 0x57f0,
    'singleton_get_speaker': 0x67c0, 'singleton_set_speaker': 0x6a60,
    'get_microphone_first_task': 0x5710, 'set_microphone_first_task': 0x59e0,
    'get_speaker_first_task': 0x6980, 'set_speaker_first_task': 0x6c50,
    'get_microphone_thread': 0x32ac6, 'set_microphone_thread': 0x32dfa,
    'get_speaker_thread': 0x34066, 'set_speaker_thread': 0x3439c,
    'get_microphone_dispatch': 0x32c80, 'set_microphone_dispatch': 0x32fd0,
    'get_speaker_dispatch': 0x34220, 'set_speaker_dispatch': 0x34570,
    'win_get_thunk': 0x3a850, 'win_get': 0x3a856,
    'win_set_thunk': 0x3a8f0, 'win_set': 0x3a8f6,
    'find_cached_endpoint': 0x3b6a6, 'endpoint_id_compare': 0x3bd26,
    'endpoint_initialize': 0x37144, 'read_scalar_cache': 0x37a56,
    'write_volume_and_mute': 0x37b34, 'volume_event_cache': 0x37ad0,
    'create_endpoint_by_id': 0x3e9e4, 'activate_volume': 0x3fb86,
}


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def receipt(path, anchors):
    raw = (ROOT/path).read_bytes()
    source = raw.decode('utf-8')
    locators = []
    for anchor, before, after in anchors:
        at = source.index(anchor)
        start, end = max(0, at-before), min(len(source), at+after)
        locators.append({'anchor': anchor,
                         'start_utf16': len(source[:start].encode('utf-16-le'))//2,
                         'end_utf16': len(source[:end].encode('utf-16-le'))//2,
                         'text': source[start:end]})
    return {'path': path, 'sha256': sha(raw), 'bytes': len(raw), 'locators': locators}


def acquire():
    work = ROOT/'.work/ida-static/simple-audio-volume'
    work.mkdir(parents=True, exist_ok=True)
    source = ROOT/SOURCE
    assert sha(source.read_bytes()) == SHA
    target = work/'simple_service.dll'
    if not target.exists():
        shutil.copyfile(source, target)
    assert sha(target.read_bytes()) == SHA
    output = work/'analysis.json'
    config = work/'functions.json'
    config.write_text(json.dumps({'expected_sha256': SHA, 'output': str(output),
                                  'function_rvas': list(FUNCTIONS.values())}), encoding='utf-8')
    env = dict(os.environ)
    python = str(ROOT/'.work/ida-static/python311')
    env['PATH'] = python+os.pathsep+env['PATH']
    env['PYTHONHOME'] = python
    database = work/'simple_service.dll.i64'
    subprocess.run([r'D:\ida\IDA Pro 8.3\idat64.exe', '-A', '-L'+str(work/'analysis.log'),
                    '-S'+str(ROOT/'tools/ida-static-functions.py')+' '+str(config),
                    str(database if database.exists() else target)], cwd=work, env=env,
                   creationflags=subprocess.CREATE_NO_WINDOW, stdout=subprocess.PIPE,
                   stderr=subprocess.STDOUT, check=True, timeout=1800)
    assert sha(source.read_bytes()) == SHA
    return json.loads(output.read_text('utf-8'))


def build(analysis):
    raw = (ROOT/SOURCE).read_bytes()
    assert sha(raw) == SHA == analysis['input_sha256']
    pe = pefile.PE(data=raw)
    assert pe.OPTIONAL_HEADER.ImageBase == analysis['image_base']
    assert analysis['hexrays_available']
    functions = {f['rva']: f for f in analysis['functions']}
    assert set(functions) == set(FUNCTIONS.values())
    boundaries = {e.struct.BeginAddress: e.struct.EndAddress for e in pe.DIRECTORY_ENTRY_EXCEPTION}
    for rva, fn in functions.items():
        assert fn['pseudocode'] and not fn['decompiler_error']
        if rva in boundaries:
            assert boundaries[rva] == fn['end_rva']
        else:
            assert rva in {0x3a850, 0x3a8f0}
            assert fn['instructions'][-1]['text'].startswith('jmp ')
        assert sha(pe.get_data(rva, fn['end_rva']-rva)) == fn['code_sha256']
    table_slots = []
    for table, slot, target in [(0x1f2040, 0xc8, 0x5550), (0x1f2040, 0xd0, 0x57f0),
                                (0x1f2040, 0x100, 0x67c0), (0x1f2040, 0x108, 0x6a60),
                                (0x1f54d0, 0x48, 0x3a850), (0x1f54d0, 0x50, 0x3a8f0),
                                (0x1f54d0, 0x80, 0x3a850), (0x1f54d0, 0x88, 0x3a8f0)]:
        assert struct.unpack('<Q', pe.get_data(table+slot, 8))[0]-analysis['image_base'] == target
        table_slots.append({'vtable_rva': table, 'slot': slot, 'target_rva': target})
    assert str(uuid.UUID(bytes_le=pe.get_data(0x1f5aa0, 16))) == '5cdf2c82-841e-4546-9722-0cf74078229a'
    assert struct.unpack('<f', pe.get_data(0x1f5230, 4))[0] == 100.0
    assert struct.unpack('<d', pe.get_data(0x1f5238, 8))[0] == 0.5
    for rva, fragments in {
        0x3a856: ['sub_18003B6A6(a1 + 40', '+ 101i64)', '+ 100i64)', 'Error : Cannot find audio device.'],
        0x3a8f6: ['sub_180037B34', 'v7 < 0', 'Error: Failed to set volume.'],
        0x37b34: ['+ 56i64)', '+ 112i64)', '*((_BYTE *)a1 + 100) = a3;', '*((_BYTE *)a1 + 101) = v6;'],
        0x37ad0: ['*(_BYTE *)(a1 + 101) = a2;', '*(_BYTE *)(a1 + 100) = a3;'],
        0x3e9e4: ['+ 40i64)', '+ 48i64)', '(v30[0] & 1) == 0'],
        0x3fb86: ['&unk_1801F5AA0', '23i64', 'v3 < 0'],
    }.items():
        assert all(fragment in functions[rva]['pseudocode'] for fragment in fragments), hex(rva)
    instructions = {i['rva']: i['text'] for fn in functions.values() for i in fn['instructions']}
    for rva, fragment in [(0x37a97, 'mulss'), (0x37a9f, 'cvtss2sd'), (0x37aa3, 'addsd'),
                           (0x37aab, 'cvttsd2si'), (0x37b6d, 'movzx'), (0x37b71, 'cvtsi2ss'),
                           (0x37b75, 'divss'), (0x37b84, 'xor     r8d, r8d'),
                           (0x37bad, 'xor     r8d, r8d')]:
        assert fragment in instructions[rva], (hex(rva), fragment)
    sources = [
        receipt('.ref/host-4.0.827/electron/modules/simple_service/win/index.js', [
            ('simpleGetMicrophoneVolume:[', 0, 400),
            ('simpleGetSpeakerVolume=async', 0, 1800),
            ('simpleGetMicrophoneVolume=async', 0, 1850)]),
        receipt('.ref/middleware/1352/main.6debfb04ffc1be201fd1.js', [
            ('const n={audioProtocol:{simpleService:', 80, 380),
            ('const s=Object.assign(Object.assign({taskRunnerSetDPI:', 100, 500)]),
        receipt('.ref/middleware/1352/6259.6bff933f49e134f6804f.js', [
            ('const t="getAudioDeviceId"', 120, 1500),
            ('setHeadphoneMute:t=>', 0, 850),
            ('getSpeakerVolume:()=>', 0, 680),
            ('e.getSpeakerVolume().then', 60, 400),
            ('case"ON_CHANGE_VOLUME_VALUE"', 350, 480),
            ('yield Ne(o)', 450, 650)]),
        receipt('.ref/devices/1352/static/js/main.95a4f703.js', [
            ('class XU extends', 0, 2180),
            ('class bU extends', 0, 7450),
            ('LG=()=>', 0, 490),
            ('name:W_.s17,component:(0,Y_.jsx)(LG,{})', 40, 120),
            ('volumeReducer:function(e,a)', 0, 1450)]),
    ]
    setter_bridge = sources[0]['locators'][1]['text']
    assert '["bool","string","string","bool","uint8"]' in setter_bridge
    assert 'i?.payload.deviceId,i?.payload.mute,i?.payload.volume,t' in setter_bridge
    selector = sources[2]['locators'][0]['text']
    assert 'JSON.parse(a).filter((({type:t})=>t===e))' in selector
    assert 't===o.containerId&&n.includes(e===M?"Headphone":"Microphone")' in selector
    assert 'e.includes(o.device.productName)' in selector and 'N[e]=r.id' in selector
    assert 'this.state.volume.isEnabled})' in sources[3]['locators'][0]['text']
    assert 'volume:{isEnabled:!!e,value:e}' in sources[3]['locators'][0]['text']
    slider = sources[3]['locators'][1]['text']
    assert 'this.props.callOnChangeOnEveryStep&&this.props.changeValue(i)' in slider
    assert 'this.mouseIsDown=!0' in slider
    assert 'window.removeEventListener("mouseup",this.onMouseUp,!1)' in slider
    assert 'this.props.changeValue(this.state.value' in slider
    rust_paths = ['crates/razer-device/src/simple_audio_volume.rs',
                  'crates/razer-service/src/simple_audio_volume.rs',
                  'crates/razer-service/src/runtime/windows/audio_volume.rs']
    rust = [{'path': path, 'sha256': sha((ROOT/path).read_bytes())} for path in rust_paths]
    shared, facade, adapter = ((ROOT/path).read_text('utf-8') for path in rust_paths)
    compact = ''.join(adapter.split())
    for fragment in ('endpoint.method(9)', 'endpoint.method(15)', 'endpoint.method(7)',
                     'endpoint.method(14)', 'volume_to_scalar(volume)', 'ifhr>=0&&previous.muted!=mute',
                     'previous.volume!=volume', 'device.id_hresult()?==device_id',
                     'CoInitializeEx(null(),0)', 'CoUninitialize()', 'GetState', 'state&1!=0'):
        assert fragment in compact, fragment
    assert 'f64::from(scalar * 100.0_f32) + 0.5_f64' in shared
    assert 'f32::from(volume) / 100.0_f32' in shared
    assert 'lock.try_lock()' in adapter
    assert compact.count('let_lock=operation_lock(device_id)?;') == 2
    assert '.truncate(false)' in compact
    assert 'unsupported on this platform' in facade
    assert not any(s in adapter for s in ['libloading', 'EngineLibrary', 'LoadLibrary'])
    ipc = (ROOT/'crates/razer-ipc/src/lib.rs').read_text('utf-8')
    portable = (ROOT/'crates/razer-service/src/runtime/portable.rs').read_text('utf-8')
    runtime = (ROOT/'crates/razer-service/src/runtime/mod.rs').read_text('utf-8')
    for action in ('AudioVolumeRead', 'AudioVolumeWrite'):
        assert action in ipc and 'ServiceRequest::'+action in portable and 'ServiceRequest::'+action in runtime
    assert 'crate::simple_audio_volume::read' in portable and 'crate::simple_audio_volume::write' in portable
    return {'schema_version': 1, 'method': 'Current source text and private-input IDA/Hex-Rays; no target or system audio execution',
            'binary': {'path': SOURCE, 'sha256': SHA, 'bytes': len(raw)},
            'exporter_sha256': sha((ROOT/'tools/ida-static-functions.py').read_bytes()),
            'ida': analysis, 'function_roles': FUNCTIONS, 'vtable_slots': table_slots,
            'constants': {'volume_iid_rva': 0x1f5aa0, 'volume_iid': str(uuid.UUID(bytes_le=pe.get_data(0x1f5aa0,16))),
                          'scale_f32_rva': 0x1f5230, 'scale': 100.0,
                          'rounding_f64_rva': 0x1f5238, 'rounding': 0.5},
            'sources': sources,
            'semantics': {'getter': 'Microphone and speaker both find exact UTF8 endpoint ID in the native cache, return cached mute byte101/volume byte100; missing ID fails. Cache scalar rounding is f32*100 then f64+0.5 truncated to low byte',
                          'setter': 'Same exact ID lookup; if changed, SetMasterVolumeLevelScalar(uint8/100.0f,NULL) first; negative HRESULT returns immediately. Then if changed SetMute(bool,NULL); only successful fields update cache; mute failure leaves prior volume mutation; no clamp, cancellation or rollback',
                          'callback': 'Original exports own closures until asynchronous completion; wrapper releases callback-map entries after promises settle. Getter bool/string/string/bool/uint8 returns result/reason/deviceId/muted/volume; setter bool/string returns result/reason',
                          'product_1352': 'Current factory explicitly enables simpleService; sound page mounts connected volume switch/0..100 step1 slider. Drag updates local value and global mouseup submits once; mounted volume supplies no every-step/debounce option. Slider sets isEnabled=!!value, toggle preserves numeric value. Reducer emits ON_CHANGE_VOLUME_ENABLED/VALUE; MW task calls shared volume interface, selecting exact speaker id by container then product-name fallback',
                          'task_result_limit': 'Original scalar task marks completed after awaiting setter without testing returned bool; replacement exposes native failure and subsequent observed state instead of claiming convergence'},
            'rust': rust,
            'backend_connection': {'ipc': ['AudioVolumeRead {device_id}', 'AudioVolumeWrite {device_id,mute,volume:uint8}'],
                                   'transport': 'Windows Core Audio; no vendor DLL; unsupported on other platforms',
                                   'cleanup': 'Single-call owned COM apartment balanced for S_OK/S_FALSE; all Com outputs RAII Release, string IDs owned/free via existing Com helper; interfaces released before apartment shutdown',
                                   'coordination_policy': 'Application endpoint-ID-derived OS file try-lock holds the entire read or before/mutate/read-back operation across application workers; busy or lock failure is explicit, no blocking wait, retry or deleting lock files. This is not original service-thread cache/queue semantics and does not exclude other applications changing system audio',
                                   'response': 'Read source fields match getter shape; write source_response keeps callback result separately from previous/real optional observation, errors and partial submission flags'},
            'gaps': ['Original long-lived endpoint map, registration/event callback and cached-read timing are not recreated by per-request live COM observations',
                     'Rust endpoint acquisition rejects null/invalid ID, invalid scalar or stale inactive endpoint and reports real errors; native cache can retain older values after read failures',
                     'Source create-device recognizes default/loopback identifiers; this explicit endpoint-ID API covers only observed literal endpoint IDs',
                     'Shared middleware caches selected endpoint ID and resets virtual-channel subscriptions; this state/subscription lifecycle remains incomplete',
                     'Product page connection and its source identity resolution require a separate verified UI consumer; IPC alone is not completed product UI'],
            'runtime_acceptance': 'not_run', 'whole_library_complete': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--acquire', action='store_true')
    args = parser.parse_args()
    assert not (args.check and args.acquire)
    if args.acquire:
        analysis = acquire()
    else:
        analysis = json.loads(OUTPUT.read_text('utf-8'))['ida']
    report = build(analysis)
    if args.check:
        assert json.loads(OUTPUT.read_text('utf-8')) == report
    else:
        OUTPUT.write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n', encoding='utf-8', newline='\n')
    print(f'Current simple audio volume: {len(FUNCTIONS)} IDA bodies, real factory/page/selector/wrapper and direct Rust read/write verified statically; runtime not run')


if __name__ == '__main__':
    main()
