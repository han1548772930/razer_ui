"""Statically verify embedded tray pixels against the current ICO frames."""
import hashlib
import json
import tomllib
from pathlib import Path
from PIL import Image, IcoImagePlugin

ROOT = Path(__file__).resolve().parents[1]
manifest = tomllib.loads((ROOT / 'Cargo.toml').read_text(encoding='utf8'))
assert manifest['dependencies']['tray-icon']['default-features'] is False
assert 'ksni' in manifest['dependencies']['tray-icon']['features']
assert all('tray-icon' not in target.get('dependencies', {}) for target in manifest.get('target', {}).values())
records = json.loads((ROOT / 'assets/synapse/manifest.json').read_text(encoding='utf-8'))['entries']
icons = [entry for entry in records if entry.get('conversion') == 'exact-ico-frame']
assert len(icons) == 8  # seven notification-area sizes and the 20px application menu icon
checked = []
for entry in icons:
    source, target = ROOT / entry['source'], ROOT / entry['output']
    assert source.relative_to(ROOT).as_posix() == '.ref/host-4.0.827/electron/resources/images/rzAppEngine.ico'
    with source.open('rb') as stream:
        size = (entry['width'], entry['height'])
        decoded = IcoImagePlugin.IcoFile(stream).getimage(size).convert('RGBA')
        assert decoded.size == size
        assert decoded.tobytes() == target.read_bytes(), target
    assert hashlib.sha256(target.read_bytes()).hexdigest() == entry['sha256']
    checked.append({'output': entry['output'], 'size': size, 'exact_pixels': True})
native, = [entry for entry in records if entry.get('conversion') == 'exact-png']
assert native['source'] == '.ref/host-4.0.827/electron/resources/images/rzAppEngine.png'
with Image.open(ROOT / native['source']) as decoded:
    assert decoded.size == (16, 16)
    assert decoded.convert('RGBA').tobytes() == (ROOT / native['output']).read_bytes()
assert hashlib.sha256((ROOT / native['output']).read_bytes()).hexdigest() == native['sha256']
checked.append({'output': native['output'], 'size': [16, 16], 'exact_pixels': True})
(ROOT / 'docs/re/tray-icon-validation.json').write_text(
    json.dumps({'verification': 'Static resource comparison; no application executed.', 'icons': checked}, indent=2)
    + '\n', encoding='utf-8')
print(f'Validated {len(checked)} tray rasters against exact official ICO/PNG pixels.')
