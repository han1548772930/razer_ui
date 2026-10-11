"""Prepare the exact current mouse mapping Requires Synapse logo."""
import hashlib
import json
from pathlib import Path
import re

ROOT=Path(__file__).resolve().parent.parent
directory=ROOT/'local-ui-reverse/source/official/apps.razer.com/synapse/products/190/ui'
name='logo_synapse.bc241e5e.svg'
source=directory/'static/media'/name
raw=source.read_bytes();digest=hashlib.sha256(raw).hexdigest()
assert digest=='f74f198ee96c805884128990125507816603ce9d28db5d5e25685198c2f43f79'
manifest=json.loads((directory/'asset-manifest.json').read_text(encoding='utf-8'))
rules=[]
for relative in set(manifest['files'].values()):
    if not relative.endswith('.css'):continue
    sheet=directory/relative.removeprefix('./');data=sheet.read_bytes();text=data.decode('utf-8')
    for m in re.finditer(r'[^{}]*\.require-synapse\s+\.logo[^{}]*\{[^{}]*'+re.escape(name)+r'[^{}]*\}',text):
        rules.append({'path':sheet.relative_to(ROOT).as_posix(),'sha256':hashlib.sha256(data).hexdigest(),'rule':m[0]})
assert rules
output=ROOT/'assets/synapse/mapping-190-requires-synapse.svg';output.write_bytes(raw)
receipt={'source':source.relative_to(ROOT).as_posix(),'output':output.relative_to(ROOT).as_posix(),
         'source_url':f'https://apps.razer.com/synapse/products/190/ui/static/media/{name}',
         'source_sha256':digest,'sha256':digest,'source_bytes':len(raw),'output_bytes':len(raw),
         'css_evidence':rules,'preparation':'Exact current source logo bytes; no substitution.',
         'evidence':'docs/re/mouse-190-requires-synapse-logo-current-source.json'}
(ROOT/receipt['evidence']).write_text(json.dumps(receipt,indent=2)+'\n',encoding='utf-8')
target=ROOT/'assets/synapse/manifest.json';assets=json.loads(target.read_text(encoding='utf-8'))
assets['entries']=[e for e in assets['entries'] if e['output']!=receipt['output']]+[receipt]
target.write_text(json.dumps(assets,indent=2,ensure_ascii=False)+'\n',encoding='utf-8')
print(f'Prepared exact Requires Synapse logo: {digest}')
