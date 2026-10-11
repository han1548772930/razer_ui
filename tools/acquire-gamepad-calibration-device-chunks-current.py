"""Acquire exact AST-declared middleware chunks as bytes; never execute scripts."""
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import urllib.request

ROOT = Path(__file__).resolve().parent.parent

def product(pid):
    source = json.loads((ROOT / f'docs/re/gamepad-{pid}-calibration-middleware-live-source.json').read_text(encoding='utf-8'))
    receipts = []
    for chunk in source['runtime_chunks']:
        name = chunk['file']
        assert '/' not in name and '..' not in name
        url = f'https://apps.razer.com/synapse/products/{pid}/mw/{name}'
        request = urllib.request.Request(url, headers={'User-Agent':'RazerStaticSourceAudit/1.0'})
        with urllib.request.urlopen(request, timeout=35) as response:
            raw = response.read()
            assert response.status == 200 and response.url == url
        path = ROOT / f'local-ui-reverse/source/official/apps.razer.com/synapse/products/{pid}/mw/{name}'
        path.write_bytes(raw)
        receipt = {**chunk, 'path':path.relative_to(ROOT).as_posix(), 'url':url,
                   'retrieved_utc':datetime.now(timezone.utc).isoformat(),
                   'sha256':hashlib.sha256(raw).hexdigest(), 'bytes':len(raw),
                   'http_status':200, 'declaration_source':source['source'],
                   'declaration_sha256':source['sha256']}
        path.with_name(path.name+'.http.json').write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
        receipts.append(receipt)
    (ROOT / f'docs/re/gamepad-{pid}-calibration-device-chunks-current-acquisition.json').write_text(json.dumps({'product_id':pid,'method':'AST declared filenames downloaded as inert data','receipts':receipts},indent=2)+'\n',encoding='utf-8')
    return {'product_id':pid,'chunks':len(receipts),'bytes':sum(x['bytes'] for x in receipts)}

if __name__ == '__main__':
    with ThreadPoolExecutor(max_workers=2) as pool:
        for result in pool.map(product,[2676,2684]):
            print(json.dumps(result))
