"""Static current SysUtilsNative keyboardLayout semantics and typed IPC audit."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path

import pefile

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'docs/re/sysutils-keyboard-layout-current-evidence.json'
SHA = '01223bfabf0355f42705836974e343a1f16f837d692999020d218b2615b8e318'


def receipt(path):
    body = (ROOT / path).read_bytes()
    return {'path': path, 'sha256': hashlib.sha256(body).hexdigest()}


def excerpt(path, anchor, length):
    text = (ROOT / path).read_text('utf-8')
    start = text.index(anchor)
    selected = text[start:start + length]
    offset = len(text[:start].encode('utf-16-le')) // 2
    return {**receipt(path), 'start_utf16': offset,
            'end_utf16': offset + len(selected.encode('utf-16-le')) // 2, 'text': selected}


def audit():
    binary = '.ref/host-4.0.827/native-evidence/CommonDLL/SysUtilsNative.dll'
    raw = (ROOT / binary).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == SHA
    evidence = f'docs/re/evidence/ida-native/{SHA}.json.gz'
    corpus = json.loads(gzip.decompress((ROOT / evidence).read_bytes()))
    assert corpus['input_sha256'] == SHA
    pe = pefile.PE(data=raw)
    functions = []
    for rva in [0x70b10, 0x70aa0]:
        f = next(f for f in corpus['functions'] if f['rva'] == rva)
        assert f['pseudocode'] and not f['decompiler_error']
        assert hashlib.sha256(pe.get_data(rva, f['end_rva'] - rva)).hexdigest() == f['code_sha256']
        functions.append(f)
    facade = 'crates/razer-platform/src/keyboard_layout.rs'
    adapter = 'crates/razer-platform/src/platform/windows/keyboard_layout.rs'
    text = (ROOT / adapter).read_text('utf-8')
    assert 'let mut klid = [0u8; 64]' in text
    assert 'GetKeyboardLayoutNameA(klid.as_mut_ptr())' in text
    assert 'parse_klid(&klid)' in text
    text = (ROOT / 'crates/razer-service/src/runtime/mod.rs').read_text('utf-8')
    assert 'ServiceRequest::KeyboardLayoutRead => razer_platform::keyboard_layout::get()' in text
    assert 'json!({"layout":layout})' in ''.join(text.split())
    return {
        'method': 'Current native bytes checked against IDA/Hex-Rays and current JS source; no DLL, application, test or device execution',
        'binary': receipt(binary), 'ida_evidence': receipt(evidence), 'functions': functions,
        'source': [
            excerpt('.ref/host-4.0.827/electron/modules/sysutil/win/index.js', 'keyboardLayout:[', 25),
            excerpt('.ref/host-4.0.827/electron/main.js', 'case"keyboardLayout"', 23),
            excerpt('.ref/middleware/182/main.7d7bac778fbfcdb02c10.js', 'get keyboardLayout(){', 165),
        ],
        'semantics': {
            'query': 'Zeroed 64-byte ANSI buffer and UINT result; GetKeyboardLayoutNameA BOOL ignored; sscanf %x; UINT returned with original int FFI bit pattern',
            'identity': 'Calling-thread Windows KLID; no foreground-thread substitution, profile default or saved layout',
            'ownership': 'Stack-only getter; no allocated string, callback registration, initialization or FreeMalloc',
        },
        'implementation': [receipt(facade), receipt(adapter)],
        'connection': {
            'ipc': 'ServiceRequest::KeyboardLayoutRead -> keyboard_layout::get -> {layout:i32}',
            'gap': 'Page/macro consumers and Start/StopMonitorKeyboardLayout timer/event chain are separate and not yet reproduced',
        },
        'unsupported_platform': 'Explicit unsupported Windows KLID capability; no invented macOS/Linux mapping',
        'runtime_acceptance': 'not_run',
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    text = json.dumps(audit(), ensure_ascii=False, indent=2) + '\n'
    if args.check:
        assert OUTPUT.read_text('utf-8') == text, 'keyboard layout evidence changed'
    else:
        OUTPUT.write_text(text, encoding='utf-8', newline='\n')
    print('SysUtils keyboard layout: 2 IDA functions byte-verified, current wrapper and typed worker query verified')
