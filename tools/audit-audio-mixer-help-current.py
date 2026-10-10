"""Current 1342 Help reset-audio caller, mounted layout and native receipt.

Parse vendor source as text and retained IDA data. Never execute vendor code.
"""
import argparse
import gzip
import hashlib
import json
from pathlib import Path

import pefile

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = 'docs/re/audio-mixer-help-current-evidence.json'
SHA = 'af10595e3ce6cf9b394488e1929fc6e3d54a887c6be62b66260d7ee507c9e8f2'
IDA = f'docs/re/evidence/ida-native/{SHA}.json.gz'


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    inputs = {
        '.ref/devices/1342/static/js/main.2623357c.js': [
            'component:(0,Qe.jsx)(Rg,{resetObm:this.resetDevice,resetAudio:this.resetAudio})',
            'this.resetAudio=async()=>{this.setState({isRestart:!0}),setTimeout(()=>{this.props.resetAudio&&(this.props.resetAudio(),this.setState({isRestart:!1}))},1e3)}',
            'className:"btn reset-btn",onClick:this.resetAudio',
            'this.state.isRestart?(0,Qe.jsx)(FN,{isMounted:!0}):null',
            'postMessage({type:"ON_RESET_AUDIO_MIXER",timerTick:void 0,payload:{}})',
            'w=async()=>{_.A.msSettings("sound")},b=async()=>{_.A.msSettings("apps-volume")}',
            'Ve="SETTING_IT_UP"', 'Sn="AUDIO_TROUBLESHOOTING"',
            'un="AUDIO_TROUBLESHOOTING_DES"', 'Rn="AUDIO_TROUBLESHOOTING_BUTTON"',
            'this.setMicMonitorEnable=e=>this.camyDllDevice.setMicMonitorEnable(e)',
            'this.setMicMonitorLevel=e=>this.camyDllDevice.setMicMonitorLevel(e)',
        ],
        '.ref/middleware/1342/5736.fdd55044a41484df28aa.js': [
            '[l.pF.ON_RESET_AUDIO_MIXER]:D(l.n3.resetAudioMixer,!1,!1,!0)',
            'case _.n3.resetAudioMixer:', 'yield e.restartAudioDriver()',
            'p.T_.enqueueTask(p.SS.NORMAL_SKIPPABLE,Dt.resetAudioMixer',
        ],
        '.ref/middleware/1342/AudioMixer.cde922aae2f0fea23404.js': [
            'const t="RazerT2ResetStream",n=Gt(8,0,u.Number_Int);let o=null;for(let r=0;r<=8;++r)',
            'o=JSON.parse(a),o.jsonData={propterTypeName:t}}return o',
        ],
    }
    sources = []
    for path, tokens in inputs.items():
        raw = (ROOT / path).read_bytes()
        text = raw.decode('utf-8')
        receipts = []
        for token in tokens:
            assert token in text, (path, token)
            offset = text.index(token)
            receipts.append({'offset': offset, 'token': token,
                             'snippet': text[max(0, offset-100):offset+len(token)+180]})
        sources.append({'path': path, 'sha256': sha(raw), 'offset_unit': 'Python Unicode code points', 'receipts': receipts})
    analysis = json.loads(gzip.decompress((ROOT / IDA).read_bytes()))
    assert analysis['input_sha256'] == SHA and analysis['hexrays_available']
    binary = (ROOT / analysis['source']['path']).read_bytes()
    assert sha(binary) == SHA
    function = next(f for f in analysis['functions'] if f['rva'] == 0xfe70)
    pe = pefile.PE(data=binary)
    assert sha(pe.get_data(function['rva'], function['end_rva']-function['rva'])) == function['code_sha256']
    for token in ['DeviceIoControl(FileW, 0x222440u', 'CreateFileW(v7->DevicePath, 0xC0000000, 0, 0i64, 3u, 0x80u', 'v6 = 65537;', 'v6 = 65539;', 'return v6;']:
        assert token in function['pseudocode']
    implementations = []
    gates = {
        'crates/razer-pages/src/features/source_help.rs': ['fn restart_audio(', 'Duration::from_secs(1)', 'audio_streams: true', 'audio_reset_tasks.insert', 'AUDIO_TROUBLESHOOTING', 'SETTING_IT_UP', 'fn audio_setup_action('],
        'crates/razer-shell/src/shell/receiver_reset.rs': ['if request.is_audio_streams()', 'fn request_audio_stream_reset(', 'restart_audio_streams(&observation, &signal)', 'view.finish_help_reset(request, None, error'],
        'crates/razer-shell/src/shell/audio_mixer.rs': ['fn restart_audio_streams(', 'route::resolve_identity(&mut client, observation)', 'ServiceRequest::HidNodeMixerRestartStreams', 'source_sequence_completed', 'return_codes'],
        'crates/razer-shell/src/shell/audio_mixer/route.rs': ['fn resolve_identity(', 'resolve_collection(client, observation, None)', 'if let Some(target) = target'],
        'crates/razer-shell/src/shell/audio_mixer/route/windows.rs': ['target: Option<&MixerTarget>', 'let eligible = if let Some(target) = target', 'belongs(interface, observation)', 'previous_instance'],
        'crates/razer-device/src/audio_mixer.rs': ['fn restart_streams(', 'return_codes.push((index, code))', 'if code == 0', 'source_restart_continues_native_error_codes_but_stops_on_transport_exception'],
        'crates/razer-service/src/runtime/windows/mixer_driver.rs': ['Err(_) => return Ok(0x10001)', 'Ok(if succeeded { 0 } else { 0x10003 })'],
    }
    for path, tokens in gates.items():
        raw = (ROOT / path).read_bytes()
        text = raw.decode('utf8')
        for token in tokens:
            assert token in text, (path, token)
        implementations.append({'path': path, 'sha256': sha(raw)})
    result = {
        'schema_version': 1,
        'generator_sha256': sha(Path(__file__).read_bytes()),
        'source_acquisition': sources,
        'native': {'receipt': IDA, 'sha256': sha((ROOT / IDA).read_bytes()),
                   'binary': analysis['source'], 'function': function},
        'semantics': {'mount': '1342 Help ug via Rg: Setup and Audio Troubleshooting left; Support, Factory Reset, Serial, Firmware and Registration right',
                      'trigger': 'No confirmation: spinner true, independent 1000 ms timer, invoke resetAudio prop, spinner false without waiting for native result',
                      'transport': 'ON_RESET_AUDIO_MIXER -> resetAudioMixer task -> restartAudioDriver -> MixerSDKLib_PropertyControl / RazerT2ResetStream indices 0..8',
                      'native_codes': '0 / 0x10001 / 0x10003 remain response data; each later index is still submitted',
                      'exceptions': 'Thrown transport/JSON parsing/identity error stops the sequence; never converted to successful native code',
                      'publication': 'No observed audio state, profile replacement, added getter or local persistence',
                      'monitoring_namespace': 'Actual 1342 Mic Monitor uses AudioCamy SetMicMonitorEnable/Level; not CmMixerLib MicMonitorVolumeControl'},
        'rust_implementation': implementations,
        'ui_backend_connection': 'Actual Help reset action reaches direct local driver IPC; Windows owner correlation retains observed ContainerId/path/instance without DSP report gates; OS settings URI actions are Windows-only',
        'verification': {'pure_rust_mock_tests': '14 audio_mixer tests passed, including native-code continuation and exception stop',
                         'cargo_check': 'razer-shell / razer-service --locked --all-targets passed',
                         'runtime_acceptance': False},
        'remaining_gaps': ['Original NORMAL_SKIPPABLE middleware scheduling semantics are not fully equivalent to the local serialized queue',
                           '1342 factory reset / full Help overlay and final inherited CSS rendering remain incomplete',
                           'OS settings use GPUI platform URI opening rather than the original host helper chain',
                           'Original broadcasts driver interfaces; local adapter selects a unique observed container',
                           'AudioCamy Mic Monitor and virtual mixer consumers remain incomplete',
                           'No application, native DLL, helper, OS settings action or device operation was executed']
    }
    encoded = json.dumps(result, ensure_ascii=False, indent=2) + '\n'
    if args.check:
        assert (ROOT / OUTPUT).read_text('utf8') == encoded, OUTPUT
    else:
        (ROOT / OUTPUT).write_text(encoded, encoding='utf8', newline='\n')
    print('1342 Help: current mount/caller, IDA ResetStream and Rust UI/service chain verified statically')


if __name__ == '__main__':
    main()
