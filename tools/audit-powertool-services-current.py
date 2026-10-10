"""Statically recover local service helper functions with IDA/Hex-Rays."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import xml.etree.ElementTree as ET

import pefile

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT/'docs/re/powertool-services-current-evidence.json'
SHA = '5d04be921ae8f9f2338fc4b69afbfdb5ac3435dd1d95511bc0c9f82c3f305c72'
RVAS = [0x356e0, 0x35810, 0x35b90, 0x35cf0, 0x35de0, 0x35eb0,
        0x36130, 0x36290, 0x36360, 0x36430, 0x6e9c0, 0x6e9f0, 0x6e960, 0x6e930]
SERVICES = ['Razer Game Manager Service 3', 'Razer Chroma SDK Server',
            'Razer Chroma SDK Service', 'Razer Chroma Stream Server',
            'Razer Synapse Service', 'VSSrv', 'Audiosrv', 'XTU3SERVICE',
            'HapticService', 'RazerExperienceService', 'Carol Routing Service',
            'razer_elevation_service']


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def source_receipt(path, anchors):
    raw = (ROOT/path).read_bytes()
    source = raw.decode('utf-8')
    locators = []
    for anchor, before, after in anchors:
        index = source.index(anchor)
        start, end = max(0, index-before), min(len(source), index+after)
        locators.append({'anchor': anchor, 'start_utf16': len(source[:start].encode('utf-16-le'))//2,
                         'end_utf16': len(source[:end].encode('utf-16-le'))//2,
                         'text': source[start:end]})
    return {'path': path, 'sha256': sha(raw), 'bytes': len(raw), 'locators': locators}


def acquire(binary, exporter, directory):
    directory.mkdir(parents=True, exist_ok=True)
    target = directory/'RzPowerTool.exe'
    if not target.exists():
        shutil.copyfile(ROOT/binary['path'], target)
    assert sha(target.read_bytes()) == SHA
    output = directory/'service-analysis.json'
    config = directory/'service-config.json'
    config.write_text(json.dumps({'expected_sha256': SHA, 'output': str(output),
                                  'function_rvas': RVAS}), encoding='utf-8')
    env = dict(os.environ)
    python_dir = str(ROOT/'.work/ida-static/python311')
    env['PATH'] = python_dir+os.pathsep+env['PATH']
    env['PYTHONHOME'] = python_dir
    database = directory/'RzPowerTool.exe.i64'
    process = subprocess.run([r'D:\ida\IDA Pro 8.3\idat64.exe', '-A',
                              '-L'+str(directory/'service-ida.log'),
                              '-S'+str(exporter)+' '+str(config),
                              str(database if database.exists() else target)],
                             cwd=directory, env=env, creationflags=subprocess.CREATE_NO_WINDOW,
                             stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=1800)
    assert process.returncode == 0
    assert sha((ROOT/binary['path']).read_bytes()) == SHA
    return json.loads(output.read_text('utf-8'))


def build(binary, analysis, exporter):
    original = (ROOT/binary['path']).read_bytes()
    assert sha(original) == SHA == analysis['input_sha256']
    pe = pefile.PE(data=original, fast_load=True)
    pe.parse_data_directories(directories=[pefile.DIRECTORY_ENTRY['IMAGE_DIRECTORY_ENTRY_RESOURCE']])
    manifests = []
    for type_entry in pe.DIRECTORY_ENTRY_RESOURCE.entries:
        if type_entry.id != 24:
            continue
        for name_entry in type_entry.directory.entries:
            for language_entry in name_entry.directory.entries:
                entry = language_entry.data.struct
                raw_manifest = pe.get_data(entry.OffsetToData, entry.Size)
                xml = ET.fromstring(raw_manifest)
                level = xml.find('.//{urn:schemas-microsoft-com:asm.v3}requestedExecutionLevel')
                assert level is not None
                manifests.append({'resource_type': 24, 'resource_id': name_entry.id,
                                  'language_id': language_entry.id, 'rva': entry.OffsetToData,
                                  'bytes': entry.Size, 'sha256': sha(raw_manifest),
                                  'text': raw_manifest.decode('utf-8'),
                                  'requested_execution_level': level.attrib})
    assert len(manifests) == 1
    assert manifests[0]['requested_execution_level'] == {'level': 'requireAdministrator', 'uiAccess': 'false'}
    assert analysis['image_base'] == pe.OPTIONAL_HEADER.ImageBase
    assert [f['rva'] for f in analysis['functions']] == RVAS
    functions = {f['rva']: f for f in analysis['functions']}
    for fn in functions.values():
        assert sha(pe.get_data(fn['rva'], fn['end_rva']-fn['rva'])) == fn['code_sha256']
        assert fn['pseudocode'] and not fn['decompiler_error']
    gate = functions[0x356e0]['pseudocode']
    assert all('L"'+s+'"' in gate for s in SERVICES)
    assert 'v3 - 65' in functions[0x6e960]['pseudocode']
    # Flag lies in the zero-filled virtual tail of .data, not four on-disk bytes.
    section = next(s for s in pe.sections if s.VirtualAddress <= 0xbe514 < s.VirtualAddress+s.Misc_VirtualSize)
    assert 0xbe514-section.VirtualAddress >= section.SizeOfRawData
    assert pe.get_memory_mapped_image()[0xbe514:0xbe518] == b'\0'*4
    main = functions[0x36430]['pseudocode']
    for required in ('OpenSCManagerW(0i64, 0i64, 0xF003Fu)', 'OpenServiceW(v15, (LPCWSTR)v4, 0xF01FFu)',
                     'QueryServiceStatusEx(v18, SC_STATUS_PROCESS_INFO, (LPBYTE)Buffer, 0x24u, &pcbBytesNeeded)',
                     '((v24 - 1) & 0xFFFFFFFD) != 0', 'LODWORD(v4) = 4;',
                     'CloseServiceHandle(v18)', 'CloseServiceHandle(v16)', 'LocalFree(v7)'):
        assert required in main
    # Match control/timing/recursion to both pseudocode and raw instructions,
    # not names or incidental imported API entries alone.
    control_requirements = {
        0x35810: ['((v18 - 1) & 0xFFFFFFFD) != 0', 'v21 / 0xA', 'v20 > v4',
                  'GetTickCount64() - v5 > v21', 'StartServiceW(hService, 0, 0i64)',
                  'v18 != 2', 'v20 > v10', 'GetTickCount64() - v11 > 30 * v13',
                  '(_DWORD)v12 == 4'],
        0x35b90: ['EnumDependentServicesW(hService, 3u', 'GetLastError() == 234',
                  'HeapAlloc(ProcessHeap, 8u', 'sub_140035CF0(hSCManager, v8[v4++].lpServiceName)',
                  'HeapFree(v9, 0, v8)', 'LOBYTE(v5) = 1'],
        0x35cf0: ['OpenServiceW(hSCManager, lpServiceName, 0xF01FFu)',
                  'v8 = sub_140035810(v4)', 'sub_140035B90(hSCManager, v5)',
                  'CloseServiceHandle(v5)', 'return v8'],
        0x35de0: ['sub_1400356E0((__int64)lpServiceName)', 'OpenSCManagerW(0i64, 0i64, 0xF003Fu)',
                  'sub_140035CF0(v3, lpServiceName)', 'CloseServiceHandle(v4)', 'return v6'],
        0x35eb0: ['TickCount64 = GetTickCount64()', 'Buffer.dwCurrentState == 1',
                  'while ( dwCurrentState == 3 )', 'Buffer.dwWaitHint / 0xA',
                  'GetTickCount64() - TickCount64 > 0x7530',
                  'ControlService(hService, 1u, &Buffer)', 'Sleep(Buffer.dwWaitHint)',
                  'return 1'],
        0x36130: ['EnumDependentServicesW(hService, 1u', 'GetLastError() == 234',
                  'HeapAlloc(ProcessHeap, 8u', 'sub_140036290(hSCManager, v8[v4++].lpServiceName)',
                  'HeapFree(v9, 0, v8)', 'LOBYTE(v5) = 1'],
        0x36290: ['OpenServiceW(hSCManager, lpServiceName, 0x2Cu)',
                  'sub_140036130(hSCManager, v4)', 'v8 = sub_140035EB0(v5)',
                  'CloseServiceHandle(v5)', 'return v8'],
        0x36360: ['sub_1400356E0((__int64)lpServiceName)', 'OpenSCManagerW(0i64, 0i64, 0xF003Fu)',
                  'sub_140036290(v3, lpServiceName)', 'CloseServiceHandle(v4)', 'return v6'],
    }
    for rva, required in control_requirements.items():
        body = functions[rva]['pseudocode']
        for fragment in required:
            assert fragment in body, (hex(rva), fragment)
    start_instructions = '\n'.join(i['text'] for i in functions[0x35810]['instructions'])
    stop_instructions = '\n'.join(i['text'] for i in functions[0x35eb0]['instructions'])
    assert 'imul    edx, ebx, 1Eh' in start_instructions
    assert 'cmp     rax, 7530h' in stop_instructions
    assert 'mov     r9d, 24h' in start_instructions and 'mov     r9d, 24h' in stop_instructions
    source_path = '.ref/host-4.0.827/electron/serviceFunction.js'
    raw = (ROOT/source_path).read_bytes()
    text = raw.decode('utf-8')
    assert 'params:`--get-service-status ${s?.payload.actionArgs.toString()}`' in text
    assert 'params:`--start-service ${s?.payload.actionArgs.toString()}`' in text
    assert 'params:`--stop-service ${s?.payload.actionArgs.toString()}`' in text
    assert 'return await r.callDLL(void 0,n)' in text
    facade = ROOT/'crates/razer-platform/src/windows_service_status.rs'
    facade_code = facade.read_text('utf-8')
    for operation in ('query', 'start', 'stop'):
        assert 'pub fn '+operation+'(service_name: &str) -> anyhow::Result<u32>' in facade_code
        assert 'crate::platform::windows::services::'+operation+'(service_name)' in facade_code
    assert '#[cfg(not(target_os = "windows"))]' in facade_code
    assert 'unsupported on this platform' in facade_code
    assert 'unsafe extern' not in facade_code
    implementation = ROOT/'crates/razer-platform/src/platform/windows/services.rs'
    code = implementation.read_text('utf-8')
    assert all('"'+s+'"' in code for s in SERVICES)
    for required in ('0xF003F', '0xF01FF', '1 | 3 => status.current_state', '_ => 4',
                     'CloseServiceHandle(self.0)', 'Ok(0)'):
        assert required in code
    compact = ''.join(code.split())
    for required in ('pub(crate)fnstart(service_name:&str)->anyhow::Result<u32>',
                     'pub(crate)fnstop(service_name:&str)->anyhow::Result<u32>',
                     'Operation::Stop=>0x2C', 'Operation::Start=>3', 'Operation::Stop=>1',
                     'let_=control_dependents(manager,&service,operation)',
                     '(wait_hint/10).clamp(1000,10000)',
                     'status.check_point>checkpoint', 'elapsed_since(started)>u64::from(status.wait_hint)',
                     'elapsed_since(started)>u64::from(30*delay)', 'elapsed_since(started)>30000',
                     'Sleep(status.wait_hint)', 'status.current_state==4',
                     'StartServiceW(service.0,0,std::ptr::null())',
                     'HeapAlloc(GetProcessHeap(),8,sizeasusize)',
                     'HeapFree(GetProcessHeap(),0,self.0)',
                     'let_=control_handle(manager,record.service_name,operation)'):
        assert required in compact, required
    runtime = (ROOT/'crates/razer-service/src/runtime/mod.rs').read_text('utf-8')
    assert 'ServiceRequest::WindowsServiceStatus { name }' in runtime
    assert 'razer_platform::windows_service_status::query(&name)' in runtime
    assert 'ServiceRequest::WindowsServiceStart { name }' in runtime
    assert 'ServiceRequest::WindowsServiceStop { name }' in runtime
    assert 'razer_platform::windows_service_status::start(&name)' in runtime
    assert 'razer_platform::windows_service_status::stop(&name)' in runtime
    callers = [
        source_receipt('.ref/middleware/1308/main.43ba39d032b2aab35ef5.js', [
            ('this.getServiceStatus=', 0, 543), ('this.startService=', 0, 322),
            ('this.stopService=', 0, 320)]),
        source_receipt('.ref/middleware/1308/3487.890232e981811de11d53.js', [
            ('let t="VSSrv"', 45, 690)]),
        source_receipt('.ref/middleware/1308/AudioMixer.4c20dfa4dc889a664eb6.js', [
            ('this.restartAudioService=', 0, 477)]),
    ]
    query_caller, start_caller, stop_caller = (l['text'] for l in callers[0]['locators'])
    assert 'action:"GetServiceStatus"' in query_caller and 'n=i.exitCode' in query_caller
    assert 'action:"StartService"' in start_caller and 'o=!!r.exitCode' in start_caller
    assert 'action:"StopService"' in stop_caller and 'o=!!r.exitCode' in stop_caller
    assert all('actionArgs:void 0!==e?\'"\'+e+\'"\':""' in s for s in (query_caller,start_caller,stop_caller))
    thx = callers[1]['locators'][0]['text']
    assert 'e.thxCarol' in thx and 'Carol Routing Service' in thx
    assert '1===e' in thx and 'startService(E())' in thx and 'e=yield f()' in thx
    restart = callers[2]['locators'][0]['text']
    assert 'stopService("Audiosrv")' in restart and 'startService("Audiosrv")' in restart
    return {'schema_version': 1, 'method': 'IDA/Hex-Rays on a hash-verified private current helper; no debugger, target execution or system commands',
            'binary': binary, 'exporter_sha256': sha(exporter.read_bytes()), 'ida': analysis,
            'process_manifest': {'receipts': manifests,
                                 'required_token': 'Original helper manifest requires Administrator; matching SCM rights in a normal worker does not recreate that elevated-token environment',
                                 'rust_gap': 'Direct adapter does not launch an elevated helper, request UAC or recreate original launcher process lifecycle; Windows access failures are honestly returned as helper0, not retried as invented success'},
            'source': {'path': source_path, 'sha256': sha(raw), 'text': text},
            'frontend_callers': {'receipts': callers,
                                 'bridge': 'Frontend wraps service name in quotes for command-line actionArgs; GetServiceStatus returns response.exitCode (default0), StartService/StopService return !!response.exitCode (defaultfalse)',
                                 'THX_service': 'Shared task chooses Carol Routing Service when thxCarol feature is set, otherwise VSSrv; state1 triggers start then a fresh status read',
                                 'AudioMixer_restart': 'Stop Audiosrv first; false returns failure immediately; only after stop true start Audiosrv and return its bool',
                                 'product_gate_limit': 'Receipts identify current emitted shared implementations; presence in product1308 files does not establish all features are selected by its actual factory/DeviceInfo'},
            'service_name_gate': {'names': SERVICES, 'comparison': 'Default-C UTF16 comparison folds ASCII A-Z, recovered at RVA 0x6e960; locale-switch flag RVA 0xbe514 lies in the zero-filled virtual .data tail',
                                  'nondefault_locale_gap': 'Alternate CRT locale branch recovered as pseudocode, but locale changes are not recreated in the Rust adapter'},
            'status': {'caller': 'GetServiceStatus -> serviceFunction.getServiceStatus -> simpleLaunchRazerApp --get-service-status argument -> WinMain',
                       'sc_manager_access': 0xf003f, 'service_access': 0xf01ff, 'query_buffer_bytes': 36,
                       'return': 'Helper exitCode: 0 on failure/unsupported; 1 stopped; 3 stop-pending; every other queried state collapsed to 4',
                       'cleanup': 'Service then manager CloseServiceHandle on query success/failure; argv LocalFree before helper exit',
                       'rust': {'path': implementation.relative_to(ROOT).as_posix(), 'sha256': sha(implementation.read_bytes()),
                                'facade': {'path': facade.relative_to(ROOT).as_posix(), 'sha256': sha(facade.read_bytes())},
                                'interface': 'query(&str) -> anyhow::Result<u32>', 'unsupported_platform': 'Explicit error'}},
            'start_stop': {
                'entry_gate': 'Only top-level DoStartService/DoStopService apply the 12-name default-C comparison gate; recursive SCM dependency names are not regated',
                'start': {'chain_rvas': [0x35de0, 0x35cf0, 0x35810, 0x35b90],
                          'service_access': 0xf01ff, 'return': 'Helper exitCode 1 on parent success, 0 on failure/unsupported',
                          'already_running': 'Initial state other than stopped(1)/stop-pending(3) returns true without StartServiceW, including paused/start-pending',
                          'wait_for_stop': 'Sleep clamp(waitHint/10,1000,10000); query failure false; checkpoint increase resets tick/checkpoint; otherwise elapsed>new waitHint fails before next state check',
                          'start_pending': 'After StartServiceW(0,NULL) and successful initial requery: Sleep clamp(waitHint/10,1000,10000); checkpoint increase resets tick/checkpoint; otherwise elapsed>30*previously chosen sleep delay breaks; polling query failure also breaks and inspects the retained buffer; final state==4 determines success',
                          'dependencies': 'Only after parent success enumerate SERVICE_STATE_ALL(3); process returned records in order recursively with same SCM handle; ignore child and enumeration return values'},
                'stop': {'chain_rvas': [0x36360, 0x36290, 0x36130, 0x35eb0],
                         'service_access': 0x2c, 'return': 'Helper exitCode 1 on parent stop success, 0 on failure/unsupported',
                         'dependencies': 'Before parent stop enumerate SERVICE_ACTIVE(1); recursively stop returned dependent names in order; ignore child and enumeration return values',
                         'pending': 'One fixed GetTickCount64 baseline before initial query; stopped true; while stop-pending Sleep clamp(waitHint/10,1000,10000), requery then stopped true, otherwise elapsed>30000 false',
                         'control': 'ControlService(SERVICE_CONTROL_STOP=1) into the same prefix buffer; then Sleep full waitHint without clamp, requery, stopped succeeds before timeout check, otherwise elapsed>30000 false; no baseline reset'},
                'enumeration': 'Probe EnumDependentServicesW; TRUE returns true; non-ERROR_MORE_DATA failure false; error234 allocates required bytes via HeapAlloc(HEAP_ZERO_MEMORY=8); allocation failure false; second query success invokes children; second query failure still frees and returns true; HeapFree/process heap and SC_HANDLE release on all paths',
                'rust': {'path': implementation.relative_to(ROOT).as_posix(), 'sha256': sha(implementation.read_bytes()),
                         'facade': {'path': facade.relative_to(ROOT).as_posix(), 'sha256': sha(facade.read_bytes())},
                         'interfaces': ['start(&str) -> anyhow::Result<u32>', 'stop(&str) -> anyhow::Result<u32>'],
                         'cancel_timeout': 'No source-defined cancellation/rollback added; progressive start checkpoints can extend indefinitely, and post-control stop sleep may exceed the nominal 30s threshold',
                         'unsupported_platform': 'Explicit error'}},
            'ui_backend_connection': {'worker': 'razer-service::runtime',
                                      'request': 'ServiceRequest::WindowsServiceStatus {name} -> windows_service_status::query -> {name,status}',
                                      'control_requests': ['ServiceRequest::WindowsServiceStart {name} -> windows_service_status::start -> {name,exit_code}',
                                                           'ServiceRequest::WindowsServiceStop {name} -> windows_service_status::stop -> {name,exit_code}'],
                                      'response_policy': 'Application IPC returns a status field containing the original helper exit-code semantics; this envelope is not the original process launcher response',
                                      'control_response_policy': 'exit_code retains helper0/1; it is not an original launcher exitCode envelope or proof all dependent services succeeded; client timeout cannot establish that no system control occurred',
                                      'gap': 'Original service/page/foreground callers are not yet connected'},
            'semantic_completion': 'Local status/start/stop machine-code branches, recursive dependent ordering, return/error/timing/cleanup recovered and implemented for default-C service names and a caller token with required rights; original required-Administrator helper launch/elevation, logging/lifecycle, whole helper and real UI consumers remain incomplete',
            'runtime_acceptance': 'not_run'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--acquire', action='store_true', help='Run installed IDA statically on private current helper input')
    args = parser.parse_args()
    assert not (args.check and args.acquire)
    helpers = json.loads((ROOT/'docs/re/host-helpers-current-evidence.json').read_text('utf-8'))
    binary = next(r for r in helpers['helpers'] if r['sha256'] == SHA)
    exporter = ROOT/'tools/ida-static-functions.py'
    directory = ROOT/'.work/ida-static/corpus'/SHA
    if args.acquire:
        analysis = acquire(binary, exporter, directory)
    elif OUTPUT.exists():
        prior = json.loads(OUTPUT.read_text('utf-8'))
        assert prior['exporter_sha256'] == sha(exporter.read_bytes())
        analysis = prior['ida']
    else:
        analysis = json.loads((directory/'service-analysis.json').read_text('utf-8'))
    report = build(binary, analysis, exporter)
    if args.check:
        assert json.loads(OUTPUT.read_text('utf-8')) == report
    else:
        OUTPUT.write_text(json.dumps(report, ensure_ascii=False, indent=2)+'\n',encoding='utf-8',newline='\n')
    print('PowerTool services: 14 IDA functions; status/start/stop, 12-name gate, recursive dependent ordering, waits and Rust adapters verified statically; runtime not run')


if __name__ == '__main__':
    main()
