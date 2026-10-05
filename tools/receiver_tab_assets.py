"""Register exact receiver SVG layers and source-backed final favicon assets."""
import hashlib
import json
import shutil
from host_tab_ico import prepare as prepare_ico


def prepare(root, output):
    evidence = json.loads((root / 'docs/re/receiver-current-evidence.json').read_text(encoding='utf8'))
    records = evidence['assets'].copy()
    records.extend(prepare_ico(root, output))
    applications = json.loads((root / 'docs/re/app-favicons-current-fetch.json').read_text(encoding='utf8'))
    names = {'synapse/armory:workshop':'armory', 'synapse/armory:exchange':'armory-exchange', 'synapse/profiles':'profiles', 'feedback':'feedback',
             'chroma-app/dashboard':'chroma', 'synapse/introduction-tour':'tour', 'synapse/dashboard:runtime':'dashboard'}
    for entry in applications['entries']:
        if entry['route'] not in names:
            continue
        source = root / entry['path']
        target = output / ('host-app-' + names[entry['route']] + '.svg')
        if hashlib.sha256(source.read_bytes()).hexdigest() != entry['sha256']:
            raise ValueError('Changed current application favicon: ' + entry['path'])
        shutil.copyfile(source, target)
        records.append(dict(source=entry['path'], output=target.relative_to(root).as_posix(),
                            source_sha256=entry['sha256'], sha256=entry['sha256'], source_url=entry['url']))
    favicons = json.loads((root / 'docs/re/host-device-favicons-current-evidence.json').read_text(encoding='utf8'))
    seen = set()
    for entry in favicons['products']:
        if 'source_sha256' not in entry or entry['output'] in seen:
            continue
        seen.add(entry['output'])
        source, target = root / entry['source'], root / entry['output']
        if hashlib.sha256(source.read_bytes()).hexdigest() != entry['source_sha256']:
            raise ValueError('Changed current favicon: ' + entry['source'])
        shutil.copyfile(source, target)
        records.append(dict(source=entry['source'], output=entry['output'],
                            source_sha256=entry['source_sha256'], sha256=entry['source_sha256'],
                            source_url=entry['source_url']))
    for record in records:
        if hashlib.sha256((root / record['output']).read_bytes()).hexdigest() != record['sha256']:
            raise ValueError('Changed prepared SVG: ' + record['output'])
    return records
