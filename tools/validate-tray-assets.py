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
pngs = [entry for entry in records if entry.get('conversion') == 'exact-png'
        and entry['output'].startswith('assets/synapse/tray-')]
assert {Path(entry['source']).name for entry in pngs} == {
    'rzAppEngine.png', 'gear-black.png', 'gear-white.png', 'user-black.png', 'user-white.png'}
for entry in pngs:
    assert entry['source'].startswith('.ref/host-4.0.827/electron/resources/images/')
    with Image.open(ROOT / entry['source']) as decoded:
        assert decoded.size == (entry['width'], entry['height'])
        assert decoded.convert('RGBA').tobytes() == (ROOT / entry['output']).read_bytes()
    assert hashlib.sha256((ROOT / entry['output']).read_bytes()).hexdigest() == entry['sha256']
    checked.append({'output': entry['output'], 'size': [entry['width'], entry['height']], 'exact_pixels': True})
(ROOT / 'docs/re/tray-icon-validation.json').write_text(
    json.dumps({'verification': 'Static resource comparison; no application executed.', 'icons': checked}, indent=2)
    + '\n', encoding='utf-8')
print(f'Validated {len(checked)} tray rasters against exact official ICO/PNG pixels.')
