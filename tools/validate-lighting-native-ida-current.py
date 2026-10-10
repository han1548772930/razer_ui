"""Validate retained IDA function bytes against the original current PE inputs."""
import hashlib
import json
from pathlib import Path
import pefile

root = Path(__file__).resolve().parent.parent
resources = json.loads((root / 'docs/re/lighting-native-current-acquisition.json').read_text(encoding='utf-8'))['resources']
total = 0
for resource in resources:
    name = 'lighting-driver-ida.json' if resource['resource_name'] == 'LightingDriverDLL' else 'lighting-engine-ida.json'
    evidence = json.loads((root / 'docs/re/evidence' / name).read_text(encoding='utf-8'))
    data = (root / resource['input']).read_bytes()
    assert hashlib.sha256(data).hexdigest() == evidence['input_sha256'] == resource['sha256']
    assert evidence['hexrays_available']
    pe = pefile.PE(data=data)
    for function in evidence['functions']:
        code = pe.get_data(function['rva'], function['end_rva'] - function['rva'])
        assert hashlib.sha256(code).hexdigest() == function['code_sha256'], hex(function['rva'])
        assert function['pseudocode'] or function['decompiler_error']
    total += len(evidence['functions'])
print(f'Verified {total} retained static IDA function bodies against current lighting PE bytes')
