"""Prepare current camera manifest SVGs and register audited preview JSX SVGs."""
import hashlib
import json
import argparse
import re
from pathlib import Path
import urllib.request

ROOT = Path(__file__).resolve().parents[1]
parser = argparse.ArgumentParser(description=__doc__)
registration = parser.add_mutually_exclusive_group()
registration.add_argument('--register', action='store_true', help='Update the shared manifest and embedded asset table')
registration.add_argument('--check-registration', action='store_true', help='Validate prepared bytes and shared registration without fetching or writing')
options = parser.parse_args()

def emit(path, data):
    if isinstance(data, str):
        data = data.encode('utf-8')
    if options.check_registration:
        if path.read_bytes() != data:
            raise ValueError(f'Stale camera asset or receipt: {path}')
    else:
        path.write_bytes(data)
NAMES = {
    'step_down': 'camera-section-down.svg',
    'tooltip_questionmark': 'camera-section-help.svg',
    'warning-icon': 'camera-section-warning.svg',
    'icon_reset': 'camera-section-reset.svg',
    'icon_open_eye': 'camera-preview-open.svg',
    'icon_close_eye': 'camera-preview-close.svg',
    'icon_camera_unable': 'camera-preview-unable.svg',
}
rows = []
for source_name, prepared in NAMES.items():
    payload = None
    origins = []
    for pid in (3592, 3594, 3595, 3596):
        product = ROOT / '.ref' / 'devices' / str(pid)
        manifest_path = product / 'asset-manifest.json'
        manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
        key = f'static/media/{source_name}.svg'
        relative = manifest['files'][key].removeprefix('./')
        source_path = product / relative
        url = f'https://apps.razer.com/synapse/products/{pid}/ui/{relative}'
        if not source_path.exists():
            if options.check_registration:
                raise ValueError(f'Missing source SVG: {source_path}')
            with urllib.request.urlopen(url, timeout=30) as response:
                data = response.read()
            if b'<svg' not in data:
                raise ValueError(f'Not SVG: {url}')
            source_path.parent.mkdir(parents=True, exist_ok=True)
            source_path.write_bytes(data)
        data = source_path.read_bytes()
        if payload is not None and payload != data:
            raise ValueError(f'Different current camera SVG: {source_name}')
        payload = data
        origins.append({'product_id': pid, 'manifest_path': manifest_path.relative_to(ROOT).as_posix(),
                        'manifest_sha256': hashlib.sha256(manifest_path.read_bytes()).hexdigest(),
                        'manifest_key': key, 'path': source_path.relative_to(ROOT).as_posix(), 'url': url,
                        'sha256': hashlib.sha256(data).hexdigest()})
    destination = ROOT / 'assets' / 'synapse' / prepared
    emit(destination, payload)
    rows.append({'prepared_path': destination.relative_to(ROOT).as_posix(),
                 'sha256': hashlib.sha256(payload).hexdigest(), 'sources': origins})
output = {'schema_version': 1, 'assets': rows}
emit(ROOT / 'docs/re/camera-section-assets-current-evidence.json',
     json.dumps(output, ensure_ascii=False, indent=2) + '\n')
if options.register or options.check_registration:
    manifest_path = ROOT / 'assets/synapse/manifest.json'
    manifest = json.loads(manifest_path.read_text(encoding='utf-8'))
    embedded_path = ROOT / 'assets/synapse/embedded.rs'
    embedded = embedded_path.read_text(encoding='utf-8')
    entries = []
    for row in rows:
        source = row['sources'][0]
        entries.append({'source': source['path'], 'source_sha256': source['sha256'],
                        'output': row['prepared_path'], 'sha256': row['sha256'],
                        'source_url': source['url'], 'source_kind': 'downloaded_static_asset',
                        'evidence': 'docs/re/camera-section-assets-current-evidence.json'})
    # camera-preview.cjs emits these from literal JSX props while statically
    # auditing the camera page. Its receipt hashes the excerpt, not the file.
    preview_receipt = 'docs/re/camera-preview-current-evidence.json'
    preview_assets = json.loads((ROOT / preview_receipt).read_text(encoding='utf-8'))['assets']
    expected_preview = {f'assets/synapse/camera-preview-{name}.svg' for name in ('external', 'arrow')}
    if len(preview_assets) != 2 or {row['output'] for row in preview_assets} != expected_preview:
        raise ValueError('Changed preview JSX asset set')
    source_cache = {}
    for row in preview_assets:
        if len(row['sources']) != 3:
            raise ValueError('Preview JSX requires all three mounted current camera sources')
        products = set()
        for source in row['sources']:
            source_path = Path(source['path'])
            if source_path.parts[:2] != ('.ref', 'devices'):
                raise ValueError(f'Invalid current camera JSX source: {source_path}')
            pid = int(source_path.parts[2])
            products.add(pid)
            current_manifest = json.loads((ROOT / '.ref/devices' / str(pid) / 'asset-manifest.json')
                                          .read_text(encoding='utf-8'))
            if source_path.as_posix() != f'.ref/devices/{pid}/' + current_manifest['files']['main.js'].removeprefix('./'):
                raise ValueError(f'JSX source is not the current manifest main.js: {source_path}')
            if source['path'] not in source_cache:
                data = (ROOT / source_path).read_bytes()
                source_cache[source['path']] = (data, data.decode('utf-8').encode('utf-16-le'))
            data, utf16 = source_cache[source['path']]
            excerpt = utf16[source['offset'] * 2:source['end'] * 2].decode('utf-16-le')
            if excerpt != source['source'] or hashlib.sha256(excerpt.encode('utf-8')).hexdigest() != source['sha256']:
                raise ValueError(f'Stale preview JSX receipt: {source_path}')
            literals = {}
            for key in ('width', 'height', 'viewBox', 'd'):
                values = re.findall(r'(?<![\w])' + key + r':"([^"\\]*)"', excerpt)
                if len(values) != 1:
                    raise ValueError(f'Expected one literal JSX {key}: {source_path}')
                literals[key] = values[0]
            if 'style:{fill:"currentColor"}' not in excerpt:
                raise ValueError(f'Changed preview JSX fill: {source_path}')
            payload = (f'<svg xmlns="http://www.w3.org/2000/svg" width="{literals["width"]}" '
                       f'height="{literals["height"]}" viewBox="{literals["viewBox"]}">'
                       f'<path fill="currentColor" d="{literals["d"]}"/></svg>\n').encode('utf-8')
            if ((ROOT / row['output']).read_bytes() != payload
                    or hashlib.sha256(payload).hexdigest() != row['sha256']):
                raise ValueError(f'Prepared preview SVG differs from literal JSX: {row["output"]}')
        if products != {3594, 3595, 3596}:
            raise ValueError('Unexpected preview JSX source products')
        source = row['sources'][0]
        entries.append({'source': source['path'],
                        'source_sha256': hashlib.sha256(source_cache[source['path']][0]).hexdigest(),
                        'output': row['output'], 'sha256': row['sha256'],
                        'source_kind': 'current_jsx_svg', 'evidence': preview_receipt,
                        'source_excerpt': {key: source[key] for key in ('offset', 'end', 'sha256')}})
    for entry in entries:
        existing = next((item for item in manifest['entries'] if item['output'] == entry['output']), None)
        asset = entry['output'].removeprefix('assets/')
        if options.check_registration:
            if existing != entry or f'"{asset}"' not in embedded:
                raise ValueError(f'Missing exact camera resource registration: {asset}')
        else:
            if existing:
                existing.clear()
                existing.update(entry)
            else:
                manifest['entries'].append(entry)
            if f'"{asset}"' not in embedded:
                embedded = embedded.rstrip().removesuffix(']') + (
                    f'    ("{asset}", include_bytes!("{Path(entry["output"]).name}") as &[u8]),\n]\n')
    if options.register:
        manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
        embedded_path.write_text(embedded, encoding='utf-8')
print(f'Validated {len(entries)} registered current camera SVGs.' if options.check_registration else
      f'Prepared {len(rows)} current camera manifest SVGs; all four manifests agree.' +
      (f' Registered {len(entries)} current camera SVGs.' if options.register else ''))
