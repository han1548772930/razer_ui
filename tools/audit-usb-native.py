"""Read the current detection.node GUID and PE bytes; do not load the module."""
import argparse
import hashlib
import json
from pathlib import Path
import uuid
import pefile

ROOT = Path(__file__).resolve().parents[1]
NATIVE = ROOT / '.ref/host-4.0.827/native-evidence/node_modules/rz-usb-detect/build/Release/detection.node'

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    data = NATIVE.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    assert digest == '2009eb2e0853c329612c53416664ab016f287d601edb9ba07dfd981e16cf6f3b'
    pe = pefile.PE(data=data)
    assert pe.FILE_HEADER.Machine == 0x8664
    guid = 'a5dcbf10-6530-11d2-901f-00c04fb951ed'
    rva = pe.get_rva_from_offset(data.index(uuid.UUID(guid).bytes_le))
    assert rva == 0x45cc0
    # The node addon has no standalone C enumeration export. Rust uses Windows
    # SetupAPI directly; this receipt must not claim an original callable ABI.
    result = dict(method='Static PE/GUID inspection only; no LoadLibrary or DLL calls',
                  path=NATIVE.relative_to(ROOT).as_posix(), sha256=digest,
                  usb_device_guid=guid, usb_guid_rva=rva,
                  exports=[x.name.decode() for x in getattr(getattr(pe, 'DIRECTORY_ENTRY_EXPORT', None), 'symbols', []) if x.name],
                  runtime_transport='Windows SetupAPI USB_DEVICE enumeration, not a vendor C export')
    output = ROOT / 'docs/re/usb-native-current-evidence.json'
    text = json.dumps(result, indent=2) + '\n'
    if args.check:
        assert output.read_text('utf-8') == text, 'Stale USB native evidence'
    else:
        output.write_text(text, encoding='utf-8')
    print('Current USB addon GUID and SHA-256 verified statically; no callable vendor ABI asserted')

if __name__ == '__main__':
    main()
