"""Validate current original interrupt-read bytes and portable transport receipts."""
import argparse
import gzip
import hashlib
import json
from pathlib import Path

import pefile

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'docs/re/hid-interrupt-current-evidence.json'


def receipt(path):
    raw = (ROOT / path).read_bytes()
    return {'path': path, 'sha256': hashlib.sha256(raw).hexdigest()}


def audit():
    original = json.loads((ROOT / 'docs/re/receiver-native-hid-current-evidence.json').read_text('utf-8'))
    raw = (ROOT / original['path']).read_bytes()
    assert hashlib.sha256(raw).hexdigest() == original['sha256']
    evidence_path = f"docs/re/evidence/ida-native/{original['sha256']}.json.gz"
    corpus = json.loads(gzip.decompress((ROOT / evidence_path).read_bytes()))
    assert corpus['input_sha256'] == original['sha256']
    pe = pefile.PE(data=raw)
    functions = []
    for rva, name in [(0x17800, 'hid_read_timeout'), (0x179f0, 'hid_read')]:
        function = next(f for f in corpus['functions'] if f['rva'] == rva)
        assert function['name'] == name and function['pseudocode']
        code = pe.get_data(rva, function['end_rva'] - rva)
        assert hashlib.sha256(code).hexdigest() == function['code_sha256']
        functions.append(function)
    source = '.ref/host-4.0.827/electron/modules/hidHardwareEvents/index.js'
    text = (ROOT / source).read_text('utf-8')
    for anchor in ['1!==c.length', 'new n.HID(c[0].path)', 'a.on("data"', 'appendReportId', 'r.unshift(n.hidConfig?.reportId??5)', 'removeAllListeners()', '.close()']:
        assert anchor in text, anchor
    dependency = '.work/hidapi-static-reference/src/lib.rs'
    lines = (ROOT / dependency).read_text('utf-8').splitlines()
    implementation = 'crates/razer-hid/src/transport/hidapi.rs'
    rust = (ROOT / implementation).read_text('utf-8')
    for anchor in ['fn read_interrupt(', 'i32::try_from(timeout_ms)', '.read_timeout(report, timeout)', 'if count == 0', 'valid_io_length(&self.lengths.input, id, api_count)', 'maximum - usize::from(unnumbered)']:
        assert anchor in rust, anchor
    return {
        'method': 'Static current host JS and byte-verified IDA/Hex-Rays; no application, DLL or device execution',
        'binary': receipt(original['path']),
        'ida_evidence': receipt(evidence_path),
        'functions': functions,
        'host_source': {**receipt(source), 'text': text},
        'dependency': {**receipt(dependency), 'first_line': 563, 'last_line': 584, 'text': '\n'.join(lines[562:584])},
        'semantics': {
            'native': 'Overlapped ReadFile; timeout returns 0; actual byte count; leading zero API ID removed, nonzero ID retained; no GET_REPORT request',
            'host': 'Unique vendor/product/ContainerId/usage pair; node data preserved; prepend only when actual producer hidConfig.appendReportId is set; unregister removes listeners and closes retained handle',
            'portable': 'Separate finite-timeout interrupt channel; raw numbered ID retained, unnumbered has no synthetic prefix; descriptor-backed ID/length validation rejects truncation',
        },
        'implementation': receipt(implementation),
        'contract': receipt('crates/razer-device/src/backend.rs'),
        'application_policy': ['Finite nonnegative timeout for cancellation polling', 'Buffer must fit descriptor maximum; mixed numbered/unnumbered input rejected', 'Malformed report rejected rather than copied/truncated'],
        'connection': 'Receiver Session requires a HardwareEvents provider before Scan/Pair; this transport alone is not a product event subscription',
        'gaps': ['164/241 mapping-engine selector/callback provenance is independently proved in receiver-pairing-transport-current-evidence.json; other product producers require their own proof and no universal record-5 prefix', 'Interrupt reader alone does not establish peer grouping or pairing completion; actual 164/241 service, Dock consumer and local cache have separate implementation evidence', 'Equivalent non-Windows physical grouping and full runtime/profile publication remain gaps; cross-platform runtime acceptance not executed'],
        'runtime_acceptance': 'not_run',
    }


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    text = json.dumps(audit(), ensure_ascii=False, indent=2) + '\n'
    if args.check:
        assert OUTPUT.read_text('utf-8') == text, 'interrupt transport evidence changed'
    else:
        OUTPUT.write_text(text, encoding='utf-8', newline='\n')
    print('HID interrupt: 2 original IDA functions byte-verified; current host and raw portable read contracts verified')
