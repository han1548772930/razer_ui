"""Prepare manifest-declared dock artwork; parse assets as data only."""
import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path
from urllib.request import Request, urlopen
from datetime import datetime, timezone
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
digest = lambda data: hashlib.sha256(data).hexdigest()
specs = json.loads((ROOT / 'src/features/dock_pairing_data.json').read_text(encoding='utf-8'))
records = []
for spec in specs:
    for asset in spec['assets']:
        source, output = ROOT / asset['source'], ROOT / asset['output']
        assert source.resolve().is_relative_to((ROOT / f'.ref/devices/{spec["product_id"]}').resolve())
        assert output.resolve().is_relative_to((ROOT / 'assets/synapse').resolve())
        if not source.exists():
            with urlopen(Request(asset['url'], headers={'User-Agent':'razer-ui-static-resource-audit'}), timeout=30) as response:
                content = response.read()
                assert response.status == 200
                metadata = {'source_url':asset['url'], 'final_url':response.url, 'http_status':response.status,
                            'content_type':response.headers.get('Content-Type'), 'sha256':digest(content),
                            'fetched_at_utc':datetime.now(timezone.utc).isoformat()}
            if source.suffix == '.svg':
                ET.fromstring(content)
            source.parent.mkdir(parents=True, exist_ok=True)
            source.write_bytes(content)
            source.with_name(source.name+'.http.json').write_text(json.dumps(metadata,indent=2)+'\n',encoding='utf-8')
        content = source.read_bytes()
        if source.suffix == '.svg':
            svg = ET.fromstring(content)
            assert svg.tag.endswith('svg') and not any(el.tag.endswith('script') for el in svg.iter())
            output.write_bytes(content)
        else:
            assert source.suffix == '.avif'
            with Image.open(source) as image:
                image.convert('RGBA').save(output, optimize=True)
        records.append({**asset, 'source_sha256':digest(content), 'output_sha256':digest(output.read_bytes())})
(ROOT / 'assets/synapse/dock-pairing-manifest.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf-8')
print(f'Prepared {len(records)} declared dock pairing assets.')
