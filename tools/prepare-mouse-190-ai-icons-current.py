"""Copy only the two icons referenced by current PID190 AI mapping CSS."""
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parent.parent
DIRECTORY = ROOT / 'local-ui-reverse/source/official/apps.razer.com/synapse/products/190/ui'
manifest = json.loads((DIRECTORY/'asset-manifest.json').read_text(encoding='utf-8'))
hash_bytes = lambda b: hashlib.sha256(b).hexdigest()
icons = {'synapse-ai.aa585cbf.svg': 'mapping-ai.svg',
         'synapse-ai-open.2fb76614.svg': 'mapping-ai-active.svg'}
receipts = []
for name, output_name in icons.items():
    css_evidence = []
    for relative in set(manifest['files'].values()):
        if not relative.endswith('.css'):
            continue
        sheet = DIRECTORY/relative.removeprefix('./')
        raw = sheet.read_bytes();text = raw.decode('utf-8')
        for match in re.finditer(r'[^{}]*AI_LAUNCHER[^{}]*\{[^{}]*'+re.escape(name)+r'[^{}]*\}', text):
            css_evidence.append({'path':sheet.relative_to(ROOT).as_posix(), 'sha256':hash_bytes(raw),
                                 'rule':match[0]})
    assert css_evidence, name
    source = DIRECTORY/'static/media'/name
    raw = source.read_bytes();output = ROOT/'assets/synapse'/output_name
    output.write_bytes(raw)
    receipts.append({'source':source.relative_to(ROOT).as_posix(),
                     'source_url':f'https://apps.razer.com/synapse/products/190/ui/static/media/{name}',
                     'output':output.relative_to(ROOT).as_posix(),'source_sha256':hash_bytes(raw),
                     'sha256':hash_bytes(raw),'source_bytes':len(raw),'output_bytes':len(raw),
                     'preparation':'Exact current PID190 AI mapping CSS icon bytes.',
                     'css_evidence':css_evidence,'evidence':'docs/re/mouse-190-ai-icons-current-source.json'})
(ROOT/'docs/re/mouse-190-ai-icons-current-source.json').write_text(json.dumps(receipts,indent=2)+'\n',encoding='utf-8')
target = ROOT/'assets/synapse/manifest.json'
assets = json.loads(target.read_text(encoding='utf-8'))
outputs = {r['output'] for r in receipts}
assets['entries'] = [r for r in assets['entries'] if r['output'] not in outputs] + receipts
target.write_text(json.dumps(assets,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(f'Prepared {len(receipts)} exact current AI icons with CSS and hash evidence')
