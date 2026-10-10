"""Recheck original PE spans and pinned cross-platform transport source, statically."""
from pathlib import Path
import argparse
import hashlib
import json
import re
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
    # Reviewed connection boundaries are distinct from original protocol proof.
    # These locators and hashes do not constitute execution/OS acceptance.
    consumers={
        'crates/razer-discovery/src/direct/mod.rs':['mod windows;', 'cap.direct_pids.contains(&observed.physical_product_id())', 'ServiceRequest::HidNodes', 'ServiceRequest::HidNodeReports', 'setting.matches_value(&observed)', 'self.check_reply(&reply)?'],
        'crates/razer-discovery/src/direct/platform/windows.rs':['ServiceRequest::HidDevices', 'paths.len() == 1', 'eq_ignore_ascii_case(observed.container())'],
        'crates/razer-discovery/src/discovery_portable.rs':['node.interface_number < 0', 'crate::direct::collection_scope(node)', 'hid_node: Some(node.clone())', 'crate::direct::reports_match', 'DeviceConnectionObservation::HidPresent'],
        'crates/razer-discovery/src/receiver.rs':['Self::from_observation(observed)', 'ServiceRequest::HidNodes', 'candidate| *candidate == node', 'direct::reports_match', 'ServiceRequest::HidNodeReceiverStatus', 'discovery::project_hid_receiver_query'],
        'crates/razer-settings/src/runtime_page.rs':['#[cfg(not(windows))]', 'ServiceRequest::HidNodes', 'discovery::discover_portable', 'hid_nodes: Some(nodes)', 'device.hid_node().is_some()'],
        'crates/razer-settings/src/runtime_page/diagnostics.rs':['"hid_nodes": query(&self.hid_nodes)', '"hid_node": device.hid_node()'],
        'crates/razer-service/src/runtime/portable.rs':['ServiceRequest::HidNodeReports', 'device.report_lengths()?', '"identity_scope":"hid_collection"'],
        'crates/razer-hid/src/transport/hidapi.rs':['lock.try_lock()', '_lock: lock', 'opened == *selected'],
        'crates/razer-shell/src/shell/device_discovery.rs':['observed.hid_node() == retained_node.as_ref()', 'route.write(&mut client, setting)?', 'receiver_route_for_owner', 'let result = query_route.query(&mut client);', 'retained_route.matches(observed)', 'this.discovery_revision != revision'],
        'crates/razer-pages/src/features/source_workspace.rs':['// Scheduling hint only; discovery owns exact node validation.', 'receiver_read_owner(&self.device)'],
        'crates/razer-shell/src/shell/mouse_polling_write.rs':['current.hid_node() == route.hid_node()', 'DeviceWriteSetting::Polling { hz }'],
        'crates/razer-shell/src/shell/keyboard_brightness_write.rs':['direct::resolve', 'write_keyboard'],
        'crates/razer-shell/src/shell/keyboard_brightness_read.rs':['direct::resolve', 'read_keyboard', 'current.hid_node() == route.hid_node()'],
    }
    implementation_receipts=[]
    for path,anchors in consumers.items():
        body=(ROOT/path).read_bytes(); text=body.decode('utf-8')
        compact=re.sub(r'\s+','',text)
        for anchor in anchors:
            assert re.sub(r'\s+','',anchor) in compact,(path,anchor)
        implementation_receipts.append({'path':path,'sha256':hashlib.sha256(body).hexdigest(),'reviewed_boundary_anchors':anchors})
    route=(ROOT/'crates/razer-discovery/src/direct/mod.rs').read_text('utf-8')
    assert '#[cfg(windows)]\n#[path = "platform/windows.rs"]\nmod windows;' in route
    portable=(ROOT/'crates/razer-discovery/src/discovery_portable.rs').read_text('utf-8')
    assert not any(request in portable for request in ['ServiceRequest::UsbDevices','ServiceRequest::HidDevices','ServiceRequest::NativeLibrarySnapshot'])
    protocol_evidence=[]
    for name in ['mouse-read-capabilities-current-evidence.json','device-write-capabilities-current-evidence.json','receiver-discovery-current-evidence.json','keyboard-settings-current-evidence.json']:
        path='docs/re/'+name; body=(ROOT/path).read_bytes(); json.loads(body)
        protocol_evidence.append({'path':path,'sha256':hashlib.sha256(body).hexdigest()})
    return {'method':'Static PE bytes plus pinned dependency text; no LoadLibrary, tests or hardware access',
        'razer_original':{'path':original['path'],'sha256':original['sha256'],'functions':functions},
        'hidapi_crate':crate,'source_receipts':receipts,
        'implementation':'crates/razer-hid/src/transport/hidapi.rs',
        'scope':'Source-gated portable collection discovery, subsequent receiver binding queries, direct mouse reads/writes and keyboard brightness; no unknown interface or receiver relay inference',
        'protocol_evidence':protocol_evidence,
        'implementation_receipts':implementation_receipts,
        'connection_status':'Portable UI discovery and direct routes connected; Windows-only target checked; runtime acceptance not run',
        'policies':['Fresh unique path + usage + interface identity before/after open',
            'Descriptor-derived Feature size; reject mismatches rather than backend padding/truncation',
            'Shared macOS open; never seize ordinary keyboard/mouse input',
            'Owned worker timeout and cross-process path lock; not vendor protocol constants'],
        'limitations':['No hardware execution','Only Windows target statically compiled in this environment',
            'Portable nodes do not supply Windows physical ContainerId or invent receiver relay identity',
            'Unknown interface_number including macOS -1 is explicitly rejected; source-equivalent mapping remains unimplemented',
            'Portable physical grouping, hotplug and receiver relay remain gaps; products without proved collection selection are not discovered',
            '179 subsequent binding queries use the retained physical receiver route; full 164/241 pairing/scan/write lifecycle remains incomplete',
            'Other DLL/COM/driver/service chains remain partial and separate']}

if __name__=='__main__':
    parser=argparse.ArgumentParser(); parser.add_argument('--check',action='store_true'); args=parser.parse_args()
    text=json.dumps(audit(),ensure_ascii=False,indent=2)+'\n'
    if args.check: assert OUTPUT.read_text('utf-8')==text,'portable HID evidence changed'
    else: OUTPUT.write_text(text,encoding='utf-8',newline='\n')
    print('Portable HID: 4 original PE spans; 7 dependency source receipts; pinned crate/lock checksum verified')
