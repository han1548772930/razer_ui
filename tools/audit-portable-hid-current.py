"""Recheck original PE spans and pinned cross-platform transport source, statically."""
from pathlib import Path
import argparse
import hashlib
import json
import tomllib
import pefile

ROOT=Path(__file__).resolve().parents[1]
OUTPUT=ROOT/'docs/re/cross-platform-hid-current-evidence.json'
CACHE=ROOT/'.work/hidapi-static-reference'
SELECTED={
    'src/lib.rs':[(162,185),(580,613)],
    'src/hidapi.rs':[(223,250)],
    'src/linux_native.rs':[(179,199),(448,480),(540,584)],
    'etc/hidapi/windows/hid.c':[(337,366),(975,1047),(1244,1343)],
    'etc/hidapi/mac/hid.c':[(510,536),(1019,1061),(1101,1180),(1334,1343),(1501,1517)],
    'build.rs':[(202,236)],
}
def audit():
    original=json.loads((ROOT/'docs/re/receiver-native-hid-current-evidence.json').read_text('utf-8'))
    raw=(ROOT/original['path']).read_bytes()
    assert hashlib.sha256(raw).hexdigest()==original['sha256']
    pe=pefile.PE(data=raw)
    names={'hid_open_path','hid_send_feature_report','hid_get_feature_report','hid_close'}
    functions=[]
    for item in original['functions']:
        if item['name'] not in names: continue
        data=pe.get_data(item['rva'],item['end_rva']-item['rva'])
        assert hashlib.sha256(data).hexdigest()==item['machine_code_sha256']
        functions.append(item)
    assert len(functions)==4
    crate=json.loads((CACHE/'crate-receipt.json').read_text('utf-8'))
    assert crate['version']=='2.6.7' and crate['checksum']==crate['crate_sha256']=='818c0e1d27887aaf76fe737042e27a66b796a7b099e6d2e1a72d106c2dff3fa6'
    lock=tomllib.loads((ROOT/'Cargo.lock').read_text('utf-8'))
    assert any(p['name']=='hidapi' and p['version']==crate['version'] and p['checksum']==crate['checksum'] for p in lock['package'])
    receipts=[]
    for name,ranges in SELECTED.items():
        data=(CACHE/name).read_bytes(); lines=data.decode('utf-8').splitlines()
        receipts.append({'crate_path':name,'sha256':hashlib.sha256(data).hexdigest(),
            'excerpts':[{'first_line':start,'last_line':min(end,len(lines)), 'text':'\n'.join(lines[start-1:end])} for start,end in ranges]})
    for path,key in [('assets/data/device-read-capabilities.json','products'),('assets/data/receiver-query-capabilities.json','capabilities')]:
        assert all(row['report_id']==0 for row in json.loads((ROOT/path).read_text('utf-8'))[key])
    return {'method':'Static PE bytes plus pinned dependency text; no LoadLibrary, tests or hardware access',
        'razer_original':{'path':original['path'],'sha256':original['sha256'],'functions':functions},
        'hidapi_crate':crate,'source_receipts':receipts,
        'implementation':'crates/razer-hid/src/native.rs',
        'scope':'Existing protocol25 read capabilities only; numbered/new product protocols not inferred',
        'policies':['Fresh unique path + usage + interface identity before/after open',
            'Descriptor-derived Feature size; reject mismatches rather than backend padding/truncation',
            'Shared macOS open; never seize ordinary keyboard/mouse input',
            'Owned worker timeout and cross-process path lock; not vendor protocol constants'],
        'limitations':['No hardware execution','Only Windows target statically compiled in this environment',
            'Portable nodes do not supply Windows physical ContainerId or invent receiver relay identity',
            'Other DLL/COM/driver/service chains remain partial and separate']}

if __name__=='__main__':
    parser=argparse.ArgumentParser(); parser.add_argument('--check',action='store_true'); args=parser.parse_args()
    text=json.dumps(audit(),ensure_ascii=False,indent=2)+'\n'
    if args.check: assert OUTPUT.read_text('utf-8')==text,'portable HID evidence changed'
    else: OUTPUT.write_text(text,encoding='utf-8',newline='\n')
    print('Portable HID: 4 original PE spans; 7 dependency source receipts; pinned crate/lock checksum verified')
