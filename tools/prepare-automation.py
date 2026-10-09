"""Prepare exact current automation SVG resources; never execute downloaded code."""
import hashlib
import json
import xml.etree.ElementTree as ET
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import Request, urlopen
ROOT=Path(__file__).resolve().parents[1]
digest=lambda b:hashlib.sha256(b).hexdigest()
spec=json.loads((ROOT/'crates/razer-pages/src/features/automation_data.json').read_text(encoding='utf8'))
records=[]
for asset in spec['assets']:
    source,output=ROOT/asset['source'],ROOT/asset['output']
    assert source.resolve().is_relative_to((ROOT/'.ref/devices/3946').resolve())
    assert output.resolve().is_relative_to((ROOT/'assets/synapse').resolve())
    if not source.exists():
        with urlopen(Request(asset['url'],headers={'User-Agent':'razer-ui-static-resource-audit'}),timeout=30) as response:
            content=response.read()
            assert response.status==200
            receipt={'source_url':asset['url'],'final_url':response.url,'http_status':response.status,'sha256':digest(content),'fetched_at_utc':datetime.now(timezone.utc).isoformat()}
        source.parent.mkdir(parents=True,exist_ok=True)
        source.write_bytes(content)
        source.with_name(source.name+'.http.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf8')
    content=source.read_bytes()
    output_content=asset.get('inline_svg', '').encode('utf8') or content
    if 'inline_svg' in asset:
        fragment=content.decode('utf8').encode('utf-16-le')[asset['source_offset']*2:asset['source_end']*2].decode('utf-16-le')
        assert fragment==asset['source_fragment']
    svg=ET.fromstring(output_content)
    assert svg.tag.endswith('svg') and not any(n.tag.endswith('script') for n in svg.iter())
    if not output.exists() or output.read_bytes() != output_content:
        output.write_bytes(output_content)
    records.append({**asset,'source_sha256':digest(content),'output_sha256':digest(output_content)})
(ROOT/'assets/synapse/automation-manifest.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf8')
# A prepared SVG must also reach the production AssetSource. Preserve unrelated
# resources and merge this generator's own entries using actual output bytes.
manifest_path = ROOT/'assets/synapse/manifest.json'
manifest = json.loads(manifest_path.read_text(encoding='utf8'))
embedded_path = ROOT/'assets/synapse/embedded.rs'
embedded = embedded_path.read_text(encoding='utf8')
for record in records:
    entry = {'source': record['source'], 'source_sha256': record['source_sha256'],
             'output': record['output'], 'sha256': record['output_sha256'],
             'evidence': 'assets/synapse/automation-manifest.json'}
    existing = next((item for item in manifest['entries'] if item['output'] == record['output']), None)
    if existing:
        existing.update(entry)
    else:
        manifest['entries'].append(entry)
    name = Path(record['output']).name
    asset_key = record['output'].removeprefix('assets/')
    if f'"{asset_key}"' not in embedded:
        embedded = embedded.rstrip().removesuffix(']') + f'    ("{asset_key}", include_bytes!("{name}") as &[u8]),\n]\n'
manifest_path.write_bytes((json.dumps(manifest, ensure_ascii=False, indent=2)+'\n').encode('utf8'))
embedded_path.write_bytes(embedded.encode('utf8'))
print(f'Prepared {len(records)} current automation SVG resources.')
