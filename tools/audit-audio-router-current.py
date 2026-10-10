"""Validate current AudioRouter IDA bodies, active JS registrations and Rust wiring.

Static reads only: never runs IDA, target exports, vendor JavaScript or audio.
"""
import argparse
import hashlib
import json
from pathlib import Path
import pefile

ROOT = Path(__file__).resolve().parents[1]
SHA = '9134a79a2aac0d3ca48087fe2ad62ab29b44ad334c9d85edf36171c7ea79caaf'
RECEIPT = 'docs/re/evidence/audio-router-ida.json'
OUTPUT = 'docs/re/audio-router-current-evidence.json'
SOURCES = 'docs/re/audio-router-current-source-receipts.json'


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    analysis = json.loads((ROOT / RECEIPT).read_text('utf-8'))
    binary = analysis['source_path']
    raw = (ROOT / binary).read_bytes()
    assert sha(raw) == SHA == analysis['input_sha256']
    assert analysis['hexrays_available'] and analysis['ida_version'] == '8.3'
    pe = pefile.PE(data=raw)
    functions = {f['rva']: f for f in analysis['functions']}
    for function in functions.values():
        assert sha(pe.get_data(function['rva'], function['end_rva'] - function['rva'])) == function['code_sha256']
    def gate(rva, *tokens):
        for token in tokens:
            assert token in functions[rva]['pseudocode'], (hex(rva), token)
    gate(0xba30, "Missing 'enable' in cmdData", 'sub_1800042E0', '3 - ((_DWORD)v24 != 0)', '"enabled"')
    gate(0xc130, '"primaryDevice"', '"routedDevice"', '"primaryDeviceId"', 'sub_18000CDE0')
    gate(0xd1b0, 'sub_180044AD0', 'sub_18000D0B0', 'sub_1800460E0', 'sub_180046420', 'sub_180046490')
    gate(0xdce0, '-30000000i64', 'sub_18000D830')
    gate(0xde60, 'else if ( !v5 )', '"AudioRouter_StatusChange"', '"deviceId"', '"deviceStatus"')
    gate(0x47000, '= 600', '= 300', '= 240', '= 360', '= 50', '= 5', '= 20000i64')
    gate(0x48300, 'CoInitializeEx(0i64, 0)', 'L"Pro Audio"', 'WaitForMultipleObjects(3u', 'sub_180048570', 'sub_180048910')
    gate(0x48570, 'v5 != 143196161 && v5 != -2004287464', 'v32 = 0', 'sub_180081B40')
    gate(0x48f00, 'v4 / 10000', '*(_DWORD *)(a1 + 536) = -(int)', '*(_DWORD *)(a1 + 536) += v7')
    initialization = functions[0x47640]
    assert pe.get_data(initialization['rva'], initialization['end_rva'] - initialization['rva']).count(bytes.fromhex('00000c88')) == 2
    sources = []
    def collect(path, token, kind, product):
        data = path.read_bytes()
        code = data.decode('utf-8')
        start = 0
        found = 0
        while (offset := code.find(token, start)) >= 0:
            sources.append({'product_id':product, 'kind':kind,
                'path':path.relative_to(ROOT).as_posix(), 'sha256':sha(data),
                'offset_unit':'Python Unicode code points', 'offset':offset,
                'token':token, 'snippet':code[max(0, offset-100):offset+len(token)+300]})
            start = offset + len(token)
            found += 1
        assert found, (path, token)
    for product in (1422, 1446):
        directory = ROOT / f'.ref/middleware/{product}'
        main = next(directory.glob('main.*.js'))
        chunk = next(directory.glob('8539.*.js'))
        collect(main, 'featureKey:"audio_streamMixer"', 'active feature registration', product)
        collect(main, 'ON_CHANGE_PLAY_BACK_MIX_DEVICE:!0', 'enabled UI event', product)
        collect(chunk, 'audio_streamMixer:()=>Promise.resolve()', 'feature loader', product)
        for token in ['Audio_StreamMixer', 'dispatch("AudioRouter","EnableRouting",{enable:e})',
                      'const i={primaryDevice:e,routedDevice:t}',
                      'dispatch("AudioRouter","RouteDevice",i)',
                      'this.audioUtil.on("AudioRouter_StatusChange"',
                      'enableAudioDeviceChangeNotification(!0)',
                      'this.routeExternalDevice(t,e.exterInputDevice)',
                      'yield this.enableRouting(!1)', 'changePlaybackMixDevice(e)',
                      'ON_CHANGE_PLAY_BACK_MIX_DEVICE']:
            collect(chunk, token, 'active stream mixer feature body', product)
    # Shared classes do not prove registration in these products.
    shared_candidates = [1398, 1427, 2638, 2641, 4124, 4126]
    semantics = {
        'routing_default_enabled':1,
        'route_key':'case-sensitive primaryDevice UTF-16 name; source map order',
        'endpoint_selection':'first active capture and render whose friendly name contains either route name',
        'empty_routed_device':'remove old route and succeed',
        'replacement_failure':'old route destroyed first; new failed node retained',
        'enable_failure':'individual route errors discarded by source overall result; Rust keeps diagnostics',
        'notification':'first matching route dirty; trailing 3000 ms callback',
        'notification_running_route':'stop to state 0; else-if prevents immediate reinitialization in same callback',
        'notification_dirty_flag':'source does not clear it in recovery callback',
        'wasapi':'shared mode 0, flags 0x880c0000, duration 200000 in 100 ns units, render mix format for both clients',
        'pcm':'byte FIFO; 600% capacity, 300% silence, 50 ms warmup, 2000 ms drift history, 240..360% thresholds, 5/1000 step',
        'resampling':'duplicate or skip final frames; no numerical sample interpolation',
        'event_device_id':'caller omits primaryDeviceId; source defaults to 0',
        'source_status_listener':'console log only; does not refresh a preset or synthesize reads',
    }
    implementations = [
        'crates/razer-device/src/audio_router.rs',
        'crates/razer-service/src/audio_router.rs',
        'crates/razer-service/src/platform/windows/audio_router.rs',
        'crates/razer-service/src/runtime/portable.rs',
        'crates/razer-service/src/runtime/mod.rs',
        'crates/razer-service/src/runtime/windows/native.rs',
        'crates/razer-ipc/src/lib.rs',
    ]
    for path in implementations:
        assert (ROOT / path).is_file()
    for path in ['crates/razer-ipc/src/lib.rs','crates/razer-service/src/runtime/mod.rs',
                 'crates/razer-service/src/runtime/windows/native.rs','crates/razer-service/src/runtime/portable.rs']:
        text = (ROOT / path).read_text('utf-8')
        for token in ['AudioRoutingEnable','AudioRouteDevice','AudioRouterEvents']:
            assert token in text, (path, token)
    report = {'schema_version':1, 'source_scope':'current middleware and retained current native input',
        'binary':{'path':binary,'sha256':SHA,'bytes':len(raw)},
        'ida':{'receipt':RECEIPT,'function_count':len(functions), 'function_rvas':sorted(functions)},
        'active_products':[1422,1446], 'shared_bundle_candidates_not_active_evidence':shared_candidates,
        'source_receipts':SOURCES,'semantics':semantics, 'rust_implementations':implementations,
        'ui_status':'active source consumers recovered; Shell action/device submission wiring audited separately',
        'runtime_acceptance':'not executed: AGENTS prohibits application, tests and target DLL execution',
        'remaining_differences':[
            'Native dereferences silent/null PCM packets; Rust releases and records an explicit error without inventing samples.',
            'Native can consume uninitialized bytes beyond silence reserve; Rust rejects that branch and releases render buffer as silence.',
            'Native blindly truncates mix format to 40 bytes; Rust explicitly rejects larger extensions before COM reads.',
            'Packet HRESULTs and release failures are recorded while the source pump/packet loops continue; Rust aggregates error strings rather than every native internal statistics field.',
            'COM interfaces use standard interthread marshalling to keep actual bound devices and thread ownership; source shares MTA interfaces directly.',
            'Source JSON accepts bool and low 32-bit numeric casts; typed IPC exposes i32/u32 parameters rather than undefined out-of-range float casts.',
        ]}
    encoded = json.dumps(report, ensure_ascii=False, indent=2)+'\n'
    source_encoded = json.dumps(sources, ensure_ascii=False, indent=2)+'\n'
    if args.check:
        assert (ROOT / OUTPUT).read_text('utf-8') == encoded, 'AudioRouter report stale'
        assert (ROOT / SOURCES).read_text('utf-8') == source_encoded, 'AudioRouter source receipts stale'
    else:
        (ROOT / OUTPUT).write_text(encoded, 'utf-8')
        (ROOT / SOURCES).write_text(source_encoded, 'utf-8')
    print(f'AudioRouter: {len(functions)} IDA bodies, {len(sources)} current-source receipts, active 1422/1446; static validation passed')


if __name__ == '__main__':
    main()
