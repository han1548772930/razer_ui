"""Prepare declared wireless ARGB assets without running vendor code."""
import base64
import hashlib
import io
import json
import xml.etree.ElementTree as ET
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import Request, urlopen
from PIL import Image

ROOT = Path(__file__).resolve().parents[1]
digest = lambda data: hashlib.sha256(data).hexdigest()
specs = json.loads((ROOT / 'src/features/wireless_argb_data.json').read_text(encoding='utf8'))
records = []
for spec in specs:
    for asset in spec['assets']:
        output = ROOT / asset['output']
        assert output.resolve().is_relative_to((ROOT / 'assets/synapse').resolve())
        source_path = asset['bundle'] if 'data' in asset else asset['source']
        source = ROOT / source_path
        assert source.resolve().is_relative_to((ROOT / f'.ref/devices/{spec["product_id"]}').resolve())
        if 'data' in asset:
            content = base64.b64decode(asset['data'].split(',', 1)[1], validate=True)
        else:
            if not source.exists():
                with urlopen(Request(asset['url'], headers={'User-Agent': 'razer-ui-static-resource-audit'}), timeout=30) as response:
                    content = response.read()
                    assert response.status == 200
                    receipt = {'source_url': asset['url'], 'final_url': response.url, 'sha256': digest(content),
                               'http_status': response.status, 'fetched_at_utc': datetime.now(timezone.utc).isoformat()}
                source.parent.mkdir(parents=True, exist_ok=True)
                source.write_bytes(content)
                source.with_name(source.name + '.http.json').write_text(json.dumps(receipt, indent=2)+'\n', encoding='utf8')
            content = source.read_bytes()
        if output.suffix == '.svg':
            root = ET.fromstring(content)
            assert root.tag.endswith('svg') and not any(n.tag.endswith('script') for n in root.iter())
            output.write_bytes(content)
        else:
            with Image.open(io.BytesIO(content)) as bitmap:
                bitmap.convert('RGBA').save(output, optimize=True)
        record = {**{k:v for k,v in asset.items() if k != 'data'},
                  'source': source_path, 'source_sha256': digest(source.read_bytes()),
                  'output_sha256': digest(output.read_bytes())}
        if 'data' in asset:
            record['embedded_sha256'] = digest(content)
        records.append(record)
(ROOT / 'assets/synapse/wireless-argb-manifest.json').write_text(json.dumps(records, indent=2)+'\n', encoding='utf8')
print(f'Prepared {len(records)} wireless ARGB assets.')
