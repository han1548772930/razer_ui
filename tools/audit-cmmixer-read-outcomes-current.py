"""Validate CmMixerLib read outcomes from retained IDA/Hex-Rays and PE bytes.

Only static data is read. This never executes vendor code or device operations.
"""
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re

import pefile

ROOT = Path(__file__).resolve().parents[1]
SHA = 'af10595e3ce6cf9b394488e1929fc6e3d54a887c6be62b66260d7ee507c9e8f2'
RECEIPT = f'docs/re/evidence/ida-native/{SHA}.json.gz'
OUTPUT = 'docs/re/audio-mixer-read-outcomes-current-evidence.json'
RUST = 'crates/razer-device/src/audio_mixer.rs'


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    receipt_raw = (ROOT / RECEIPT).read_bytes()
    analysis = json.loads(gzip.decompress(receipt_raw))
    binary = analysis['source']['path']
    raw = (ROOT / binary).read_bytes()
    assert sha(raw) == SHA == analysis['input_sha256']
    assert analysis['ida_version'] == '8.3' and analysis['hexrays_available']
    pe = pefile.PE(data=raw)
    functions = {f['rva']: f for f in analysis['functions']}
    mailbox_rvas = [0xe1e0, 0xe4c0, 0xe610, 0xe780, 0xe8f0, 0xeaf0,
                    0xeca0, 0xee50, 0xef90, 0xf190, 0xf2d0, 0xf410,
                    0xf580, 0xf6c0, 0xf800, 0xf940, 0xfb40]
    selected = [functions[rva] for rva in [*mailbox_rvas, 0xde60]]
    for f in selected:
        assert sha(pe.get_data(f['rva'], f['end_rva'] - f['rva'])) == f['code_sha256']
        assert f['pseudocode'] and not f['decompiler_error']
    for rva in mailbox_rvas:
        body = functions[rva]['pseudocode']
        assert 'Sleep(8u);' in body and re.search(r'v\d+ = 5;', body)
        assert 'sub_18000C170' in body and 'sub_18000C460' in body
        assert 'swprintf' in body and 'return 0i64;' in body and 'return 65540i64;' in body
        # Each body exits the loop after its fifth successful query and then
        # formats the raw reply. No busy-exhaustion error is introduced.
        assert re.search(r'v\d+ <= 0|v\d+ > 0', body)
    for token in ['v14[0] >= 0 || v12 <= 0', '(unsigned int)v14[0] >> 8', 'return 0i64;']:
        assert token in functions[0xe4c0]['pseudocode']
    for token in ['(v13[0] & 0x10) == 0 || v11 <= 0', 'swprintf', 'return 0i64;']:
        assert token in functions[0xf190]['pseudocode']
    peak = functions[0xde60]['pseudocode']
    assert '*v10 = v15;\n      sub_18000C460(v10, v8, 0i64);\n      return 0i64;' in peak
    rust = (ROOT / RUST).read_text('utf-8')
    poll = rust[rust.index('    fn poll('):rust.index('    pub fn read(')]
    assert 'Ok(result)' in poll and 'bail!' not in poll
    assert 'let _ = self.write(command, 0);' in rust
    for test in ['source_last_busy_reply_is_formatted_for_both_mailboxes_and_magic_voice',
                 'source_mailbox_failed_query_returns_error_without_further_polling',
                 'source_peak_read_sends_clear_and_keeps_samples_on_clear_failure']:
        assert f'fn {test}(' in rust
    result = {
        'schema_version': 1,
        'method': 'Retained IDA 8.3 / Hex-Rays pseudocode, function byte hashes and incoming references; static validation only',
        'generator_sha256': sha(Path(__file__).read_bytes()),
        'binary': {'path': binary, 'sha256': SHA, 'bytes': len(raw)},
        'ida_receipt': {'path': RECEIPT, 'sha256': sha(receipt_raw)},
        'functions': [{'rva': f['rva'], 'end_rva': f['end_rva'],
                       'code_sha256': f['code_sha256'],
                       'incoming_references': f['incoming_references']} for f in selected],
        'mailbox': {'function_count': len(mailbox_rvas), 'poll_delay_ms': 8,
                    'max_successful_queries': 5,
                    'busy_exhaustion': 'Decode and format final raw reply even if busy remains set',
                    'query_failure': 'Return original query error immediately; no further polling'},
        'peak': {'rva': 0xde60, 'query_failure': 'Return original query error',
                 'clear': 'After publishing both samples, issue zero write and ignore helper return; samples do not confirm successful clear'},
        'rust': {'path': RUST, 'sha256': sha((ROOT / RUST).read_bytes())},
        'runtime_acceptance': 'Not executed: no application, vendor DLL or hardware operation; Rust mock tests are separate verification',
        'remaining_gaps': ['COM endpoint branch and lifetime remain incomplete',
                           'No full native library equivalence or device acceptance is claimed']
    }
    encoded = json.dumps(result, ensure_ascii=False, indent=2) + '\n'
    if args.check:
        assert (ROOT / OUTPUT).read_text('utf-8') == encoded, OUTPUT
    else:
        (ROOT / OUTPUT).write_text(encoded, encoding='utf-8', newline='\n')
    print(f'CmMixerLib read outcomes: {len(selected)} IDA bodies and original PE hashes verified; static validation passed')


if __name__ == '__main__':
    main()
