"""Verify current JS callers and IDA machine-code evidence without execution."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path

import pefile

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'docs/re/sysutils-wheel-scroll-current-evidence.json'


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def source(path, selections):
    raw = (ROOT/path).read_bytes()
    text = raw.decode('utf-8')
    locators = []
    for anchor, before, after in selections:
        index = text.index(anchor)
        start, end = max(0, index-before), min(len(text), index+after)
        locators.append({'anchor': anchor, 'start_utf16': len(text[:start].encode('utf-16-le'))//2,
                         'end_utf16': len(text[:end].encode('utf-16-le'))//2,
                         'text': text[start:end]})
    return {'path': path, 'sha256': sha(raw), 'bytes': len(raw), 'locators': locators}


def build():
    corpus = json.loads((ROOT/'docs/re/ida-native-corpus-current.json').read_text('utf-8'))
    row = next(r for r in corpus['files'] if r['path'].endswith('/SysUtilsNative.dll'))
    original = (ROOT/row['path']).read_bytes()
    assert sha(original) == row['sha256']
    evidence = ROOT/row['evidence']['path']
    assert sha(evidence.read_bytes()) == row['evidence']['sha256']
    analysis = json.loads(gzip.decompress(evidence.read_bytes()))
    pe = pefile.PE(data=original, fast_load=True)
    functions = []
    for name, rva, required in (
        ('getWheelScrollLines', 0x77f10, ['pvParam = 0;', 'SystemParametersInfoW(0x68u, 0, &pvParam, 0);', 'return pvParam;']),
        ('setWheelScrollLines', 0x77f60, ['return SystemParametersInfoW(0x69u, uiParam, 0i64, 0);']),
    ):
        fn = next(f for f in analysis['functions'] if f['name'] == name)
        assert fn['rva'] == rva
        assert sha(pe.get_data(rva, fn['end_rva']-rva)) == fn['code_sha256']
        assert all(s in fn['pseudocode'] for s in required)
        functions.append(fn)
    sources = [
        source('.ref/middleware/182/main.7d7bac778fbfcdb02c10.js', [
            ('this.getWheelScrollLines=', 0, 255), ('this.setWheelScrollLines=', 0, 302)]),
        source('.ref/middleware/182/6259.98d162c59be4fff20fa2.js', [
            ('function jo(e,t,n,o,i)', 0, 1800)]),
        source('.ref/host-4.0.827/electron/main.js', [
            ('case"getWheelScrollLines"', 0, 58)]),
        source('.ref/host-4.0.827/electron/modules/sysutil/win/index.js', [
            ('getWheelScrollLines:[', 0, 77), ('callDLL=async', 0, 1730)]),
    ]
    bridge = sources[0]['locators']
    assert 'action:"getWheelScrollLines"' in bridge[0]['text']
    assert 'action:"setWheelScrollLines",payload:{actionArgs:[e]}' in bridge[1]['text']
    task = sources[1]['locators'][0]['text']
    for fragment in ('signal.aborted', 'browsingModeApplicationList', 'isHighResolutionScrolling',
                     'getWheelScrollLines()', 'setWheelScrollLines(1)', 'setWheelScrollLines(3)',
                     'getHapticScrollBrowserModeEnable()', 'setHapticScrollBrowserModeEnable(Number(v))'):
        assert fragment in task
    bindings = sources[3]['locators'][0]['text']
    assert 'getWheelScrollLines:["int",[]],setWheelScrollLines:["bool",["int"]]' in bindings
    assert 'return this.ffi?.callDLLMain(s,t,e)' in sources[3]['locators'][1]['text']
    facade = ROOT/'crates/razer-platform/src/wheel_scroll.rs'
    facade_code = facade.read_text('utf-8')
    for fragment in ('pub fn get() -> anyhow::Result<i32>',
                     'pub fn set(lines: i32) -> anyhow::Result<bool>',
                     'crate::platform::windows::wheel_scroll::get()',
                     'crate::platform::windows::wheel_scroll::set(lines)',
                     '#[cfg(not(target_os = "windows"))]', 'unsupported on this platform'):
        assert fragment in facade_code
    assert 'unsafe extern' not in facade_code
    implementation = ROOT/'crates/razer-platform/src/platform/windows/wheel_scroll.rs'
    code = implementation.read_text('utf-8')
    for fragment in ('SystemParametersInfoW(0x68, 0,', 'let mut lines = 0u32;',
                     'Ok(lines as i32)', 'SystemParametersInfoW(0x69, lines as u32, std::ptr::null_mut(), 0)',
                     'Ok(result != 0)'):
        assert fragment in code
    runtime = (ROOT/'crates/razer-service/src/runtime/mod.rs').read_text('utf-8')
    assert 'ServiceRequest::WheelScrollLinesRead' in runtime
    assert 'razer_platform::wheel_scroll::get()' in runtime
    assert 'ServiceRequest::WheelScrollLinesWrite { lines }' in runtime
    assert 'razer_platform::wheel_scroll::set(lines,)' in ''.join(runtime.split()) or 'razer_platform::wheel_scroll::set(lines)' in ''.join(runtime.split())
    return {'schema_version': 1, 'method': 'Current JS static parsing and original-PE byte verification against IDA/Hex-Rays; no target execution',
            'binary': row, 'functions': functions, 'sources': sources,
            'semantics': {'getter': 'UINT initialized to zero; SPI_GETWHEELSCROLLLINES(0x68,0,&value,0); BOOL ignored; FFI int reinterprets UINT bit pattern',
                          'setter': 'SPI_SETWHEELSCROLLLINES(0x69,i32 argument bit pattern,NULL,0); returned BOOL; no extra persistence/broadcast flags',
                          'allocation_cleanup': 'Getter stack UINT only; setter no allocations or handles; no native service initialization or callback needed',
                          'caller': 'Product 182 jo task gates writes on active profile high-resolution flag and normalized foreground application basename; sets 1 on enter and 3 on leave; false aborts subsequent haptic operation'},
            'rust_implementation': {'path': implementation.relative_to(ROOT).as_posix(), 'sha256': sha(implementation.read_bytes()),
                                    'facade': {'path': facade.relative_to(ROOT).as_posix(), 'sha256': sha(facade.read_bytes())},
                                    'interface': ['get() -> anyhow::Result<i32>', 'set(i32) -> anyhow::Result<bool>'],
                                    'unsupported_platform': 'Explicit error; Windows settings are not mapped to another OS setting'},
            'ui_backend_connection': {'worker': 'razer-service::runtime',
                                      'read': 'ServiceRequest::WheelScrollLinesRead -> wheel_scroll::get -> {lines:i32}',
                                      'write': 'ServiceRequest::WheelScrollLinesWrite {lines:i32} -> wheel_scroll::set -> {requested_lines:i32,accepted:bool}',
                                      'gap': 'Original foreground-event/profile/haptic task and page consumer are not yet reproduced'},
            'semantic_completion': 'Two native exports recovered; product task and complete SysUtilsNative library remain incomplete',
            'runtime_acceptance': 'not_run'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    report = build()
    if args.check:
        assert json.loads(OUTPUT.read_text('utf-8')) == report
    else:
        OUTPUT.write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
    print('SysUtils wheel-scroll: 2 IDA exports, 4 current JS sources, Rust adapter verified statically; runtime not run')


if __name__ == '__main__':
    main()
