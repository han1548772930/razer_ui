"""Recover Audio Mixer DSP property recipes from current PE instructions.

Only dumpbin reads the binaries. No DLL, application or vendor JS is executed.
Runtime recipes are emitted only after the exact instruction gates below pass.
"""
import argparse
import hashlib
import importlib
import json
from pathlib import Path
import struct

base = importlib.import_module("audit-cmmixer-protocol-current")
ROOT = base.ROOT
OUTPUT = "docs/re/audio-mixer-controls-current-evidence.json"
RUNTIME = "assets/data/audio-mixer-protocol.json"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    original = json.loads((ROOT / base.DETAIL).read_bytes())
    assert original["product_id"] == 1342
    pe = base.pefile.PE(str(ROOT / base.CHILD))
    parent = base.pefile.PE(str(ROOT / base.PARENT))
    for binary in original["binaries"]:
        assert base.sha((ROOT / binary["path"]).read_bytes()) == binary["sha256"]
    source = original["source"]
    assert base.sha((ROOT / source["path"]).read_bytes()) == source["sha256"]
    entries = original["properties"]
    # Read whole adjacent property functions, including split unwind ranges.
    ranges = {p["index"]: base.disassembly(pe, base.CHILD, p["target_rva"],
              entries[i+1]["target_rva"] if i+1 < len(entries) else 0x100e0)
              for i, p in enumerate(entries)}

    def gate(index, *instructions):
        actual = {i["instruction"] for i in ranges[index]["instructions"]}
        for instruction in instructions:
            assert instruction in actual, (entries[index]["name"], instruction)

    def sequence(index, *instructions):
        actual = [i['instruction'] for i in ranges[index]['instructions']]
        assert any(actual[i:i+len(instructions)] == list(instructions)
                   for i in range(len(actual)-len(instructions)+1)), (
            entries[index]['name'], instructions)

    identity = base.disassembly(parent, base.PARENT, 0x35518, 0x35529)
    assert [i["instruction"] for i in identity["instructions"]][:3] == [
        "mov edx,53Eh", "mov ecx,1532h", "call 0000000180034FF0"]
    open_range = base.disassembly(parent, base.PARENT, 0x34ff0, 0x35230)
    # Parent OpenHID must call the slot loaded from the child's real OpenHID.
    slots = [i for i in open_range["instructions"] if "[rax+38h]" in i["instruction"]]
    assert slots, "Parent OpenHID no longer uses the observed child slot"
    parent_instructions = {i["instruction"] for i in open_range["instructions"]}
    assert {"mov r8,qword ptr [rax+38h]", "mov edx,esi", "mov ecx,r14d", "call r8"} <= parent_instructions
    open_slot = base.disassembly(parent, base.PARENT, 0x336de, 0x336f9)
    assert parent.get_string_at_rva(0x25a518) == b"CmMixerOpenHID"
    assert {"lea rdx,[000000018025A518h]", "call qword ptr [00000001801FE898h]",
            "mov qword ptr [rbx+38h],rax"} <= {i["instruction"] for i in open_slot["instructions"]}
    child_exports = {e.name.decode(): e.address for e in pe.DIRECTORY_ENTRY_EXPORT.symbols if e.name}
    assert child_exports["CmMixerOpenHID"] == 0xb7f0
    imports = {sym.address-pe.OPTIONAL_HEADER.ImageBase: sym.name.decode() if sym.name else sym.ordinal
               for dll in pe.DIRECTORY_ENTRY_IMPORT for sym in dll.imports}
    assert imports[0x12240] == "log10f" and imports[0x12248] == "powf"
    hid_open = base.disassembly(pe, base.CHILD, 0xb7f0, 0xb8b0)
    hid_select = original['child_code_ranges']['hid_interface_open']
    hid_close = base.disassembly(pe, base.CHILD, 0xc080, 0xc168)
    assert {'call 000000018000BA50','call 000000018000BDF0',
            'call 000000018000C080','mov eax,2'} <= {
        i['instruction'] for i in hid_open['instructions']}
    assert {'cmp r15w,word ptr [rsp+5Ch]', 'cmp r14w,word ptr [rsp+5Eh]',
            'lea rdx,[rbx+20h]','call qword ptr [0000000180012018h]'} <= {
        i['instruction'] for i in hid_select['instructions']}
    for slot, name in {0x12018:'HidP_GetCaps',0x12020:'HidD_GetAttributes',
                       0x12028:'HidD_GetHidGuid',0x120b8:'CancelIo',
                       0x12128:'CloseHandle'}.items():
        assert imports[slot] == name
    constants = {}
    for rva in [0x18064, 0x18068, 0x1806c, 0x18070, 0x18078,
                0x18080, 0x18090, 0x18098, 0x180a0, 0x180ac, 0x180b4, 0x180b8]:
        data = pe.get_data(rva, 4)
        constants[hex(rva)] = {"bytes_hex": data.hex(), "float32": struct.unpack("<f", data)[0]}
    for rva, expected in {0x18064: 1/32768, 0x18068: 1/4096, 0x1806c: 1/1024,
                          0x18070: struct.unpack('<f', struct.pack('<f', 0.1))[0],
                          0x18078: 0.5, 0x18080: 1, 0x18090: 2, 0x18098: 10,
                          0x180a0: 20, 0x180ac: 100, 0x180b4: 1024, 0x180b8: 4096}.items():
        assert constants[hex(rva)]["float32"] == expected, hex(rva)

    properties = []
    def add(index, key, command, recipe, **values):
        if index != 0:
            gate(index, "call 000000018000C170", "call 000000018000C460", "mov eax,10004h",
                 "mov ebx,10003h" if index == 32 else "mov eax,10003h")
        p = {"key": key, "source_property": entries[index]["name"], "command": command,
             "recipe": recipe, "source_entry_rva": entries[index]["target_rva"], **values}
        properties.append(p)

    gate(0, "mov edx,5FFC001Ch", "call 000000018000C170", "mov eax,10004h")
    add(0, "dsp_firmware", 0x5ffc001c, "firmware")
    for index, key, command, mask in [
        (7, "eq_enabled", 0x5ffc0034, 1),
        (10, "magic_voice_enabled", 0x5ffc0034, 2),
        (12, "echo_reverb_enabled", 0x5ffc0034, 12),
        (17, "noise_gate_enabled", 0x5ffc0030, 1),
        (22, "compressor_enabled", 0x5ffc0030, 2),
        (30, "vocal_fading_enabled", 0x5ffc0014, 8),
    ]:
        gate(index, f"mov edx,{command:X}h")
        if mask == 1:
            gate(index, "and r9d,1")
        elif mask == 2:
            gate(index, "shr r9d,1", "and r9d,1")
        elif mask == 12:
            gate(index, "and eax,0FFFFFFF3h", "test al,4", "test al,8", "shl r8d,2")
        else:
            gate(index, "and eax,0FFFFFFF7h", "shl r8d,3", "and r9d,8")
        add(index, key, command, "toggle", mask=mask)

    # Exact selector masks + flags distinguish these two hardware mailboxes.
    mailboxes = [
        (13, "reverb_room", 0x50, "signed_scaled", -100, 0, 100),
        (14, "reverb_decay", 0x60, "float_scaled", constants["0x18070"]["float32"], 20, 100),
        (15, "echo_gain", 0x10, "float_scaled", 0, 2, 4096),
        (16, "echo_delay", 0x30, "signed_scaled", 20, 250, 1),
        (18, "noise_gate_threshold", 0xb0, "decibel", -60, 0, 1024),
        (19, "noise_gate_target_gain", 0xc0, "decibel", -60, 0, 1024),
        (20, "noise_gate_attack", 0xe0, "unsigned_scaled", 1, 128, 1),
        (21, "noise_gate_release", 0xd0, "unsigned_scaled", 1, 128, 1),
        (23, "compressor_threshold", 0, "negative_unsigned", -38, 0, 1),
        (24, "compressor_knee", 0x40, "unsigned_scaled", 0, 76, 1),
        (25, "compressor_ratio", 0x20, "float_scaled", 1, 10, 100),
        (26, "compressor_makeup_gain", 0xa0, "unsigned_scaled", 0, 22, 1),
        (27, "compressor_attack", 0x60, "unsigned_scaled", 1, 50, 1),
        (28, "compressor_release", 0x80, "unsigned_scaled", 1, 200, 1),
        (29, "key_shift", 0xf0, "signed_scaled", -12, 12, 1),
        (31, "vocal_fading_level", 0x90, "signed_scaled", 0, 100, 1),
    ]
    for index, key, selector, encoding, minimum, maximum, scale in mailboxes:
        command = 0x5ffc0030 if 23 <= index <= 28 else 0x5ffc0034
        gate(index, f"mov edx,{command:X}h", "mov ecx,8")
        # Compressor selectors occupy bits 5..7; bit 4 is handshake state.
        # The other mailbox uses bits 4..7. Validate the exact source masks.
        register = 'ebx' if index in (18, 19) else 'eax'
        preserve = 0xffffff1f if command == 0x5ffc0030 else 0xffffff0f
        if index == 27:
            gate(index, 'btr eax,7', 'or eax,60h')
        elif index == 28:
            gate(index, 'and eax,0FFFFFF9Fh', 'bts eax,7')
        else:
            if preserve | selector != 0xffffffff:
                gate(index, f'and {register},0{(preserve | selector):X}h')
            if selector:
                gate(index, f'or {register},{"0" if selector >= 0xa0 else ""}{selector:X}h')
        gate(index, 'mov edi,5' if index in (18, 19) else 'mov ebx,5')
        if command == 0x5ffc0030:
            gate(index, "and eax,7F0000EFh", "or r8d,10h", "or eax,80000010h", "test al,10h")
        else:
            gate(index, "test eax,eax")
            if index not in (18, 19):
                gate(index, "and eax,3F0000FFh", "or eax,0C0000000h")
            else:
                gate(index, "and ebx,3F0000FFh", "or ebx,0C0000000h",
                     "call 00000001800111FF", "call 00000001800111F9")
        if encoding == "negative_unsigned":
            gate(index, "neg r8d", "neg r9d", "cmp ecx,26h")
        guards = {
            13: ["lea ecx,[r8+64h]", "cmp ecx,64h", "imul r8d,r8d,6400h"],
            14: ["movss xmm0,dword ptr [0000000180018070h]", "comiss xmm1,dword ptr [00000001800180A0h]", "mulss xmm1,dword ptr [00000001800180ACh]"],
            15: ["xorps xmm0,xmm0", "comiss xmm1,dword ptr [0000000180018090h]", "mulss xmm1,dword ptr [00000001800180B8h]"],
            16: ["lea ecx,[r8-14h]", "cmp ecx,0E6h", "movsx r9d,ax"],
            18: ["lea eax,[rcx+3Ch]", "cmp eax,3Ch", "mulss xmm0,dword ptr [00000001800180B4h]", "addss xmm0,dword ptr [0000000180018078h]"],
            19: ["lea eax,[rcx+3Ch]", "cmp eax,3Ch", "mulss xmm0,dword ptr [00000001800180B4h]", "addss xmm0,dword ptr [0000000180018078h]"],
            20: ["lea ecx,[r8-1]", "cmp ecx,7Fh"],
            21: ["lea ecx,[r8-1]", "cmp ecx,7Fh"],
            23: ["lea ecx,[r8+26h]", "cmp ecx,26h"],
            24: ["cmp r8d,4Ch"],
            25: ["movss xmm0,dword ptr [0000000180018080h]", "comiss xmm1,dword ptr [0000000180018098h]", "mulss xmm1,dword ptr [00000001800180ACh]"],
            26: ["cmp r8d,16h"],
            27: ["lea ecx,[r8-1]", "cmp ecx,31h"],
            28: ["lea ecx,[r8-1]", "cmp ecx,0C7h"],
            29: ["lea ecx,[r8+0Ch]", "cmp ecx,18h", "movsx r9d,ax"],
            31: ["cmp r8d,64h", "movsx r9d,ax"],
        }
        gate(index, *guards[index])
        add(index, key, command, "mailbox", selector=selector, encoding=encoding,
            min=minimum, max=maximum, scale=scale, selector_preserve=preserve,
            read_flag=0xc0000000 if command == 0x5ffc0034 else 0x80000010,
            write_flag=0x80000000 if command == 0x5ffc0034 else 0x10,
            write_preserve=0x3f0000ff if command == 0x5ffc0034 else 0x7f0000ef,
            busy_mask=0x80000000 if command == 0x5ffc0034 else 0x10)

    gate(8, "cmp ecx,9", "lea ebx,[rcx*4+000000005FFC0070h]", "cmp ecx,0FFFFFFE0h",
         "cmp ecx,1Fh", "and r8d,7FFFh", "and eax,3Fh", "shl eax,0Fh",
         "lea r8d,[rax+rax]", "and eax,7FFFFh", "shr ecx,0Eh")
    add(8, "eq_band", 0x5ffc0070, "eq_band", bands=10, min=-32, max=31)
    # MagicVoiceControl has a fixed code table and a selector mailbox read.
    gate(11, "and r8d,0FFFFFF0Fh", "and r8d,3FFFFFFFh", "bts r8d,1Fh",
         "or r8d,0C0000000h", "mov ecx,8", "test eax,eax")
    voice_codes = {-1: 0x8000, 0: 0x5333, 1: 0xe28f, 2: 0x6147, 3: 0xab85}
    gate(11, "and r8d,0FF8000FFh", "bts r8d,17h")
    for code in list(voice_codes.values())[1:]:
        gate(11, f"and r8d,0{(0xff0000ff | (code<<8)):X}h")
    add(11, "magic_voice", 0x5ffc0034, "magic_voice", codes=voice_codes)

    gate(32, 'mov edx,5FFC002Ch', 'and eax,0FF80FFFFh', 'shl r8d,10h',
         'shr ecx,10h', 'and ecx,7Fh', 'neg ecx')
    add(32, 'mic_monitor_volume', 0x5ffc002c, 'register_scalar',
        min=-45, max=0, shift=16, mask=0x7f0000, encoding='negative_unsigned', scale=1)
    limits = []
    for index, key, rva, expected in [(33, 'max', None, 0), (34, 'min', 0x180c0, -45),
                                       (35, 'step', 0x18088, 1)]:
        gate(index, 'test r9b,r9b', 'mov eax,0FFFFFFFFh', 'call 0000000180001500')
        if rva is None:
            gate(index, 'xorps xmm3,xmm3')
            value = 0.0
            data = bytes(8)
        else:
            gate(index, f'movsd xmm3,mmword ptr [00000001800{rva:05X}h]')
            data = pe.get_data(rva, 8)
            value = struct.unpack('<d', data)[0]
        assert value == expected
        limits.append({'key': key, 'value': value, 'source_property': entries[index]['name'],
                       'source_entry_rva': entries[index]['target_rva'], 'constant_rva': rva,
                       'bytes_hex': data.hex(), 'origin': 'source_constant_not_device_observation'})

    # Hardware endpoint branches of the shared Volume/Mute/Peak properties.
    # Other jack/flow tuples enter COM; they remain unresolved explicitly.
    endpoint_helper = base.disassembly(pe, base.CHILD, 0xb5f0, 0xb6bc)
    assert {'addss xmm2,dword ptr [000000018001809Ch]',
            'divss xmm2,dword ptr [000000018001807Ch]',
            'call 000000018000C2F0', 'call 000000018000C580'} <= {
        i['instruction'] for i in endpoint_helper['instructions']}
    endpoint_constants = {}
    for rva, expected in {0x18074: .375, 0x1807c: .75, 0x18094: 3, 0x1809c: 12,
                          0x180a4: 39.75, 0x180a8: 87, 0x180b0: 175,
                          0x180c8: -12, 0x180cc: -45, 0x180d0: -62.25,
                          0x180d4: -65.25, 0x180d8: -65.625}.items():
        raw = pe.get_data(rva, 4)
        actual = struct.unpack('<f', raw)[0]
        assert actual == expected
        endpoint_constants[hex(rva)] = {'bytes_hex': raw.hex(), 'float32': actual}
    for index in (1, 2, 3, 4, 5, 9):
        gate(index, 'cmp ecx,3E8h' if index in (4,5) else 'cmp eax,3E8h',
             'cmp ecx,3E9h' if index in (4,5) else 'cmp eax,3E9h')
    gate(4, 'mov edx,1800C02Ch', 'mov edx,1800C028h', 'mov edx,5FFC002Ch',
         'mov ecx,1800C00Eh', 'mov ecx,1800C010h', 'call 000000018000B5F0',
         'sub eax,0AFh', 'sub eax,57h', 'neg eax', 'cmova ecx,eax')
    # Native channel -1/0/1 and exact byte preservation are part of the
    # protocol. A generic volume-mask RMW incorrectly retains mute bits for
    # Mic/LineIn Both, and loses the distinct Console channel byte order.
    sequence(4, 'movzx r8d,word ptr [rbp+20h]', 'mov eax,0FFFF8080h',
             'and r8w,ax', 'cmp edi,0FFFFFFFFh')
    sequence(4, 'mov r8d,dword ptr [rbp+20h]', 'and r8d,0FFFF8080h',
             'cmp edi,0FFFFFFFFh')
    sequence(4, 'movzx ecx,sil', 'mov edx,5FFC002Ch', 'shl ecx,8',
             'or r8d,ecx', 'call 000000018000C460')
    sequence(4, 'movzx ecx,sil', 'mov edx,1800C028h', 'shl cx,8',
             'or r8w,cx')
    sequence(4, 'movzx eax,word ptr [rbp+20h]', 'mov ecx,0FFh',
             'and ax,cx', 'movzx r8d,sil')
    helper_instructions = [i['instruction'] for i in endpoint_helper['instructions']]
    for seq in [
        ['movzx eax,word ptr [rsp+40h]', 'mov ecx,0FFh', 'and ax,cx', 'movzx r8d,dil'],
        ['movzx eax,dil', 'movzx r8d,ax', 'shl r8w,8', 'or r8w,ax'],
    ]:
        assert any(helper_instructions[i:i+len(seq)] == seq
                   for i in range(len(helper_instructions)-len(seq)+1)), seq
    gate(5, 'mov edx,1800C002h', 'mov edx,1800C006h', 'mov edx,5FFC002Ch',
         'mov edx,1800C094h', 'mov edx,1800C09Ah', 'and r9d,8000h',
         'not r9d', 'not di', 'mov ecx,7F7Fh', 'and eax,0FFFF7F7Fh')
    gate(9, 'mov ebx,5FFC0058h', 'mov ebx,5FFC0054h', 'mov ebx,5FFC005Ch',
         'mov ebx,5FFC0060h', 'mov ebx,5FFC0064h', 'cmovne ebx,ebp',
         'mulss xmm0,dword ptr [0000000180018064h]', 'xor r8d,r8d',
         'call 000000018000C460')
    endpoint_rows = [
        # key, jack, flow, volume cmd/width, mute cmd, peak cmd, min/max/step,
        # raw zero, raw step, raw bias, volume mask, inverted mute
        ('headphones',1000,0,0x1800c028,16,0x1800c006,0x5ffc0054,-62.25,0,.75,3,.75,87,0x7f,True),
        ('line_out',1001,0,0x1800c02c,16,0x1800c002,0x5ffc0058,-65.625,0,.375,0,.375,175,0xff,False),
        ('microphone',2,1,0x1800c010,16,0x1800c094,0x5ffc005c,-12,39.75,.75,-12,.75,0,0x7f,False),
        ('line_in',3,1,0x1800c00e,16,0x1800c09a,0x5ffc0060,-12,0,.75,-12,.75,0,0x7f,False),
        ('console',4,1,0x5ffc002c,32,0x5ffc002c,0x5ffc0064,-45,0,1,0,-1,0,0x7f,False),
    ]
    endpoint_limits = []
    for (key,jack,flow,command,width,mute,peak,minimum,maximum,step,
         raw_zero,raw_step,bias,volume_mask,inverted) in endpoint_rows:
        common = {'source_jack_type':jack, 'source_data_flow':flow,
                  'source_branch':'hardware HID only; shared property COM branch unresolved'}
        for index, suffix, cmd, recipe, values in [
            (4,'volume',command,'endpoint_volume',dict(width=width,min=minimum,max=maximum,
                raw_zero=raw_zero,raw_step=raw_step,raw_bias=bias,volume_mask=volume_mask,
                channel_shifts=[0,8] if key=='console' else [8,0],
                volume_write_preserve={'headphones':0x8080,'console':0xffff8080}.get(key),
                decoded_max=0 if key=='headphones' else None)),
            (5,'muted',mute,'endpoint_mute',dict(width=width,mask=0x8080,inverted=inverted)),
            (9,'peak',peak,'endpoint_peak',dict(width=32,scale=32768,read_clears_peak=True)),
        ]:
            properties.append({'key':f'{key}_{suffix}','source_property':entries[index]['name'],
                               'source_entry_rva':entries[index]['target_rva'],
                               'command':cmd,'recipe':recipe,**common,**values})
        endpoint_limits.append({'key':key, **common,'min':minimum,'max':maximum,'step':step,
                                'origin':'source_constant_not_device_observation',
                                'source_properties':[entries[i]['name'] for i in (1,2,3)]})

    gate(6, 'test r9d,0FFFFFFFBh', 'cmp r9d,5', 'cmp ecx,2',
         'lea edx,[rcx+3]', 'lea edi,[rdx+3]', 'lea edi,[rdx+4]',
         'lea edi,[rdx+2]', 'mov edx,5FFC0010h', 'shl edi,cl',
         'call 000000018000C170', 'call 000000018000C460',
         'or edi,dword ptr [rsp+44h]', 'not edi', 'and edi,dword ptr [rsp+44h]')
    for input_id, input_name, offset in [(0,'microphone',2),(4,'console',4),(5,'line_in',3)]:
        for output_id, output_name, base_bit in [(0,'headphones',0),(2,'line_out',5)]:
            properties.append({'key':f'{input_name}_to_{output_name}',
                'source_property':entries[6]['name'],'source_entry_rva':entries[6]['target_rva'],
                'command':0x5ffc0010,'recipe':'toggle','mask':1 << (base_bit+offset),
                'source_input_type':input_id,'source_output_type':output_id,
                'source_branch':'hardware HID only; other matrix routes use driver IOCTL'})
    main_path = '.ref/middleware/1342/main.6861ee97d479a87c4d7a.js'
    main_text = (ROOT/main_path).read_text('utf-8')
    layout = 'p=(e,t)=>{const o=new ArrayBuffer(8),r=new DataView(o);return r.setInt32(0,e,!0),r.setInt32(4,t,!0),new Uint8Array(o)}'
    assert layout in main_text and 'E2:()=>p' in main_text
    property_text = (ROOT/source['path']).read_text('utf-8')
    assert 'l=a.E2(t,n)' in property_text and 'i=a.E2(t,n)' in property_text
    wire_proof = {'path':main_path,'sha256':base.sha((ROOT/main_path).read_bytes()),
                  'module':97829,'export':'E2','source':layout,
                  'offset':len(main_text[:main_text.index(layout)].encode('utf-16-le'))//2}

    # ResetStream is a driver operation, not a HID report. Recover the exact
    # interface GUID, control code, buffers and source caller's 0..8 loop.
    import uuid
    driver_guid = str(uuid.UUID(bytes_le=pe.get_data(0x1b168,16)))
    assert driver_guid == 'c129656a-b1ab-4adf-88de-8d2993eb1232'
    gate(36, 'mov edx,222440h', 'mov r9d,4', 'mov dword ptr [rsp+28h],4',
         'mov dword ptr [rsp+28h],80h', 'mov edx,0C0000000h',
         'lea rcx,[000000018001B168h]', 'call qword ptr [0000000180012140h]',
         'mov ebx,10003h', 'mov ebx,10001h')
    for slot, name in {0x12140:'DeviceIoControl',0x12178:'SetupDiDeleteDeviceInterfaceData',
                       0x12180:'SetupDiGetDeviceInterfaceDetailW',0x12188:'SetupDiGetClassDevsW',
                       0x12190:'SetupDiEnumDeviceInterfaces',0x12198:'SetupDiDestroyDeviceInfoList'}.items():
        assert imports[slot] == name
    assert 'for(let r=0;r<=8;++r)' in property_text
    reset_evidence = {'source_property':entries[36]['name'],'source_entry_rva':entries[36]['target_rva'],
        'platform':'windows','device_interface_guid':driver_guid,'ioctl':0x222440,
        'input':'caller uint32 stream index, 4 bytes', 'output':'4-byte native buffer; semantic fields unresolved',
        'caller_indices':list(range(9)),'source_flags':{'desired_access':0xc0000000,'share_mode':0,
            'creation_disposition':3,'attributes':0x80},
        'status':'Source recipe and Rust IPC/Windows adapter implemented; UI consumer and runtime target correlation unverified',
        'rust':'crates/razer-service/src/runtime/windows/mixer_driver.rs',
        'identity_difference':'Original visits all driver interfaces; Rust requires a unique interface sharing the observed HID ContainerId',
        'rust_lifecycle':'Retain HID identity lock across sequence, open/close an exclusive driver handle for each stream as the source does'}

    # Non-HID routing is a 28-float driver matrix, not COM endpoint volume.
    # Decode both jump tables rather than interpreting table data as code.
    input_targets = list(struct.unpack('<7I', pe.get_data(0xdc14, 28)))
    assert input_targets == [0xd879,0xd86a,0xd86f,0xd866,0xd883,0xd87e,0xd874]
    reject_targets = list(struct.unpack('<15I', pe.get_data(0xdc30, 60)))
    assert all(t in (0xdbf2,0xd8a7,0xd89d) for t in reject_targets)
    gate(6, 'mov edx,1D6144h', 'mov edx,1DA148h', 'mov dword ptr [rsp+28h],70h',
         'mov r9d,70h', 'movss dword ptr [r14+rax*4],xmm0',
         'movss xmm0,dword ptr [r14+rax*4]', 'ucomiss xmm0,xmm6',
         'mov edx,15h', 'mov edx,7', 'lea edx,[r8+0Dh]')
    sequence(6, 'mov qword ptr [rsp+30h],rbx', 'lea rcx,[r12+4]',
             'mov dword ptr [rsp+28h],ebx', 'xor r9d,r9d', 'xor r8d,r8d',
             'mov dword ptr [rsp+20h],3', 'mov edx,0C0000000h',
             'call qword ptr [00000001800120E0h]')
    gate(6, 'xor ebx,ebx')
    for offset, target in zip([4,1,2,0,6,5,3], input_targets):
        target_instructions = [x['instruction'] for x in ranges[6]['instructions']
                               if x['rva'] == target]
        assert target_instructions == [
            'mov edi,edx' if offset == 0 else f'lea edi,[rdx+{offset}]']
    # Validate the entire source route domain. No rejected index may be
    # promoted to the driver to bypass the original six HID branches.
    route_dispatch = []
    rejected_indices = [i-1 for i,t in enumerate(reject_targets) if t != 0xd8a7]
    for input_type, input_offset in enumerate([4,1,2,0,6,5,3]):
        for output_type, output_base in enumerate([0,21,7,14]):
            selected = input_offset + output_base
            hid = input_type in (0,4,5) and output_type in (0,2)
            assert hid == (selected in rejected_indices)
            assert selected * 4 + 4 <= 112
            route_dispatch.append({'input':input_type,'output':output_type,
                                   'transport':'hid' if hid else 'driver',
                                   'matrix_index':None if hid else selected})
    assert sum(r['transport'] == 'hid' for r in route_dispatch) == 6
    assert sum(r['transport'] == 'driver' for r in route_dispatch) == 22
    matrix_evidence = {'source_property':entries[6]['name'],
        'device_interface_guid':driver_guid, 'platform':'windows',
        'input_offsets':[4,1,2,0,6,5,3], 'output_bases':[0,21,7,14],
        'input_jump_table':{'rva':0xdc14,'targets':input_targets},
        'validation_jump_table':{'rva':0xdc30,'targets':reject_targets,
                                 'index':'matrix index + 1'},
        'read_ioctl':0x1d6144,'write_ioctl':0x1da148,
        'buffer_bytes':0x70,'entries':28,'element':'little-endian IEEE754 float32',
        'write':'read 112 bytes, replace index=input_offset+output_base with caller int converted to float, write 112 bytes',
        'read':'selected float exactly equals 1.0 => 1; otherwise including NaN => 0',
        'route_dispatch':route_dispatch,
        'dispatch':'Inputs 0/4/5 and outputs 0/2 use HID instead; some other index combinations reject via validation table',
        'lifecycle':'enumerates all class interfaces; opens each with exclusive read/write; errors 0x10001 open/read, 0x10003 write; closes resources',
        'source_flags':{'desired_access':0xc0000000,'share_mode':0,
                        'creation_disposition':3,'attributes':0},
        'status':'Rust matrix codec, scoped Windows adapter and IPC reads/writes implemented; UI consumer and runtime target correlation unverified',
        'rust':'crates/razer-service/src/runtime/windows/mixer_driver.rs',
        'confirmation_difference':'Rust checks actual returned length and reads back the selected boolean after writes; original does neither',
        'identity_difference':'Original visits all driver interfaces; Rust requires a unique interface sharing the observed HID ContainerId'}

    # Identify COM interfaces from their actual GUID bytes and the activation
    # call. Slots below are byte offsets in the original Windows vtables.
    com_ids = {}
    for name, rva, guid in [
        ('MMDeviceEnumerator',0x13748,'bcde0395-e52f-467c-8e3d-c4579291692e'),
        ('IMMDeviceEnumerator',0x138b8,'a95664d2-9614-4f35-a746-de8db63617e6'),
        ('IAudioEndpointVolume',0x13768,'5cdf2c82-841e-4546-9722-0cf74078229a'),
        ('IAudioMeterInformation',0x13778,'c02216f6-8c67-4b5b-9d00-d008e73e0064'),
    ]:
        raw = pe.get_data(rva,16)
        assert str(uuid.UUID(bytes_le=raw)) == guid
        com_ids[name] = {'rva':rva,'guid':guid,'bytes_hex':raw.hex()}
    activation = base.disassembly(pe, base.CHILD, 0x4e94, 0x4eed)
    assert {'lea rdx,[0000000180013768h]','lea rdx,[0000000180013778h]',
            'call qword ptr [rax+18h]'} <= {i['instruction'] for i in activation['instructions']}
    gate(4,'call qword ptr [rax+30h]','call qword ptr [rax+40h]',
         'call qword ptr [rax+50h]','call qword ptr [rax+60h]')
    gate(5,'call qword ptr [rax+70h]','call qword ptr [rax+78h]')
    com_evidence = {'interfaces':com_ids,'activation':activation,
        'volume_vtable_byte_offsets':{'set_master_db':0x30,'get_master_db':0x40,
            'set_channel_db':0x50,'get_channel_db':0x60,'set_mute':0x70,'get_mute':0x78},
        'selection':'DataFlow 0 render / 1 capture; endpoint linked list matches caller device ID; selected node +0x30 points to interface bundle, +0x18 to volume interface',
        'status':'GUIDs, activation and property slots recovered; endpoint enumeration/identity, callbacks, lifetime and OS adapter incomplete'}

    supported = {p["source_property"] for p in [*properties, *limits]}
    partial = [{'name': entries[i]['name'], 'entry_rva':entries[i]['target_rva'],
                'status':'Five hardware jack/flow branches implemented; COM endpoint branches unresolved'}
               for i in (1,2,3,4,5,9)]
    supported.add(entries[36]['name'])
    unresolved = [{"name": p["name"], "entry_rva": p["target_rva"],
                   "status": "No source-verified Rust implementation for this property"}
                  for p in entries if p["name"] not in supported and p['index'] not in (1,2,3)]
    runtime = {"schema_version": 1, "product_id": 1342, "vendor_id": 0x1532,
               "physical_product_id": 0x53e, "properties": properties, "mic_monitor_limits": limits,
               'endpoint_limits':endpoint_limits, 'partial_properties':partial,
               'driver':{'device_interface_guid':driver_guid,'matrix_read_ioctl':0x1d6144,
                         'matrix_write_ioctl':0x1da148,'reset_stream_ioctl':0x222440,
                         'matrix_bytes':112,'input_offsets':matrix_evidence['input_offsets'],
                         'output_bases':matrix_evidence['output_bases'],
                         'rejected_indices':[i-1 for i,t in enumerate(reject_targets) if t!=0xd8a7],
                         'stream_indices':reset_evidence['caller_indices']},
               "reports": {"query_id": 4, "query_response_id": 18, "write_id": 19,
                           'query_u16_response_id':2, 'write_u16_id':3,
                           "max_output_bytes": 66, "query_delay_ms": 4,
                           "poll_delay_ms": 8, "poll_attempts": 5},
               "unsupported_properties": unresolved}
    evidence = {"schema_version": 1, "generator_sha256": base.sha(Path(__file__).read_bytes()),
                "method": "Manifest-bound current binaries, full property disassembly, exact instruction gates and numeric constants; no vendor execution",
                "binary_identity": original["binaries"], "source": source,
                "parent_identity_call": identity, "parent_open_hid": open_range, "parent_open_slot": open_slot,
                "child_property_ranges": {entries[index]["name"]: row for index, row in ranges.items()},
                "float_constants": constants, "runtime": runtime,
                'endpoint_volume_helper':endpoint_helper,'endpoint_constants':endpoint_constants,
                'matrix_parameter_layout':wire_proof,
                'reset_stream_driver':reset_evidence,
                'matrix_driver':matrix_evidence,
                'com_endpoints':com_evidence,
                'hid_interface_lifecycle':{'open_export':hid_open,'selection':hid_select,'close':hid_close,
                    'original_selection':'Enumerate HID interfaces in OS order; first VID/PID attributes match, no interface/usage gate observed',
                    'original_io_size':'HIDP_CAPS collection maximum Input/OutputReportByteLength; Output min(66,caps), Input caps into 66-byte local buffer',
                    'rust_policy':'Retain exact re-observed collection path and lock; source recipe Report IDs required; caps over 66 rejected, no first-VID/PID fallback',
                    'cleanup_difference':'Original cancels IO and sleeps 500 ms before handle cleanup; Rust uses hidapi RAII, no equivalent delay/thread model claimed'},
                "limits": ["No COM endpoint enumeration, firmware flash or service replacement implemented.",
                           "Scoped driver adapter and IPC exist; original broadcast policy differs and actual ContainerId correlation remains unverified.",
                           "Device page does not consume the new driver/HID IPC yet; local drafts remain local.",
                           "Raw DSP firmware is uint32; version component naming remains unresolved.",
                           "Mailbox reads change hardware selector state; they are not pure reads.",
                           "Exact device descriptor, path identity and caller value validation are required at runtime.",
                           "No hardware or cross-platform runtime verification."]}
    for name, data in [(RUNTIME, runtime), (OUTPUT, evidence)]:
        encoded = json.dumps(data, ensure_ascii=False, indent=2) + "\n"
        if args.check:
            assert (ROOT / name).read_text("utf-8") == encoded, name
        else:
            (ROOT / name).write_text(encoded, encoding="utf-8", newline="\n")
    print(f"Audio Mixer: {len(properties)} expanded control recipes + {len(limits)} source constants; {len(partial)} partial native properties; {len(unresolved)} uncovered properties")


if __name__ == "__main__":
    main()
