"""Prepare exact current automation SVG resources; never execute downloaded code."""
import hashlib
import json
import xml.etree.ElementTree as ET
from datetime import datetime, timezone
from pathlib import Path
from urllib.request import Request, urlopen
ROOT=Path(__file__).resolve().parents[1]
digest=lambda b:hashlib.sha256(b).hexdigest()
spec=json.loads((ROOT/'src/features/automation_data.json').read_text(encoding='utf8'))
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
    svg=ET.fromstring(content)
    assert svg.tag.endswith('svg') and not any(n.tag.endswith('script') for n in svg.iter())
    output.write_bytes(content)
    records.append({**asset,'source_sha256':digest(content),'output_sha256':digest(content)})
(ROOT/'assets/synapse/automation-manifest.json').write_text(json.dumps(records,indent=2)+'\n',encoding='utf8')
print(f'Prepared {len(records)} current automation SVG resources.')
