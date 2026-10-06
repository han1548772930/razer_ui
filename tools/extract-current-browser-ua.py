"""Recover the current host browser version and its upstream UA CSS statically.

The official inner EXE is an archive only. No installer, engine, DLL, or vendor
JavaScript is executed. The fetched CSS is source data, never executable code.
"""
import base64
import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path
from urllib.request import urlopen

ROOT = Path(__file__).resolve().parents[1]
check = sys.argv[1:] == ['--check']
if sys.argv[1:] not in ([], ['--check']):
    raise ValueError('Unknown argument')
archive = ROOT / '.work/latest-source-check/host-current/static-unpack/RazerAppEngineUpgradeSetup-Internal-v.exe'
archive_hash = '9d4765d46c5c5e1ff9c11d16452bd12a9eb43f14cd70be893fa843df3882cd90'
digest = lambda data: hashlib.sha256(data).hexdigest()
assert digest(archive.read_bytes()) == archive_hash
engine = ROOT / '.work/banner-ua/engine.bin'
receipt_path = ROOT / 'docs/re/current-browser-ua-evidence.json'
if not check:
    result = subprocess.run(['C:/Program Files/7-Zip/7z.exe', 'e', '-so', str(archive),
                             'win-unpacked/RazerAppEngine.exe'], capture_output=True)
    assert result.returncode in (0, 1), result.stderr.decode(errors='replace')
    assert len(result.stdout) == 222780104
    engine.parent.mkdir(parents=True, exist_ok=True)
    engine.write_bytes(result.stdout)
data = engine.read_bytes()
chrome = re.search(rb'Chrome/(\d+\.\d+\.\d+\.\d+)\x00', data)
electron = re.search(rb'Electron/(\d+\.\d+\.\d+)\x00', data)
assert chrome and electron
version = chrome[1].decode('ascii')
url = (f'https://chromium.googlesource.com/chromium/src/+/{version}/'
       'third_party/blink/renderer/core/html/resources/html.css?format=TEXT')
css_path = ROOT / '.ref/host-4.0.827/source-evidence/browser-ua/html.css'
if not check:
    with urlopen(url, timeout=30) as response:
        css = base64.b64decode(response.read(), validate=True)
    css_path.parent.mkdir(parents=True, exist_ok=True)
    css_path.write_bytes(css)
css = css_path.read_bytes()
text = css.decode('utf-8')
textarea = re.search(r'(?m)^textarea \{[^}]+\}', text)
assert textarea and 'padding: 2px;' in textarea[0] and 'white-space: pre-wrap;' in textarea[0]
assert 'line-height: normal;' in text
record = {
    'method': 'Static archive extraction, ASCII browser-version strings, version-matched upstream Chromium CSS; browser not executed.',
    'host_version': '4.0.827', 'archive': archive.relative_to(ROOT).as_posix(), 'archive_sha256': archive_hash,
    'archive_entry': 'win-unpacked/RazerAppEngine.exe', 'engine_sha256': digest(data),
    'chromium': {'version': version, 'offset': chrome.start(), 'source': chrome[0].rstrip(b'\x00').decode()},
    'electron': {'version': electron[1].decode(), 'offset': electron.start(), 'source': electron[0].rstrip(b'\x00').decode()},
    'css': {'path': css_path.relative_to(ROOT).as_posix(), 'sha256': digest(css), 'url': url},
    'textarea': {'offset': textarea.start(), 'end': textarea.end(), 'source': textarea[0], 'padding_css_px': 2},
    'scope': 'Upstream html.css for the exact binary-reported Chromium version. This does not claim binary-byte equivalence of all compiled Chromium styles or native font line-gap parity.'
}
encoded = json.dumps(record, indent=2) + '\n'
if check:
    assert receipt_path.read_text(encoding='utf-8') == encoded
else:
    receipt_path.write_text(encoded, encoding='utf-8')
print(f'Current host: Electron {electron[1].decode()}, Chromium {version}; textarea UA padding 2 px audited statically.')
