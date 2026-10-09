"""Static package/reference/include checks. Never runs an app, test or device API."""
from pathlib import Path
import hashlib
import json
import re
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
root = tomllib.loads((ROOT/'Cargo.toml').read_text('utf-8'))
assert root['package']['autobins'] is False
assert root['bin'] == [{'name':'razer_ui','path':'app/main.rs'}]
assert set(root['dependencies']) == {'razer-app'}
packages={}
for directory in sorted((ROOT/'crates').iterdir()):
    manifest=tomllib.loads((directory/'Cargo.toml').read_text('utf-8'))
    assert manifest['package']['name']==directory.name
    if directory.name=='razer-agent':
        assert manifest['bin']==[{'name':'razer_agent','path':'src/main.rs'}]
        assert (directory/'src/main.rs').is_file()
    else:
        assert (directory/'src/lib.rs').is_file()
    dependencies={key for key in manifest.get('dependencies',{}) if key.startswith('razer-')}
    packages[directory.name]=dependencies
assert len(packages)==21
visited=set()
def visit(name, stack=()):
    assert name not in stack, ('dependency cycle',stack,name)
    if name in visited: return
    for dep in packages[name]: visit(dep,(*stack,name))
    visited.add(name)
for name in packages: visit(name)
assert all('razer-app' not in deps for deps in packages.values())
assert all('razer-shell' not in deps for name,deps in packages.items() if name!='razer-app')
assert packages['razer-device']==set()
assert packages['razer-hid']=={'razer-device'}
assert packages['razer-assets']==set()
assert 'razer-app-pages' in packages['razer-shell']
assert 'razer-shell' not in packages['razer-app-pages']
assert 'razer-app-pages' not in packages['razer-pages']
assert packages['razer-agent']=={'razer-service'}
assert packages['razer-ipc']=={'razer-device'}
for name in ['razer-shell','razer-pages','razer-app-pages','razer-settings','razer-dashboard','razer-tray']:
    assert not packages[name] & {'razer-service','razer-hid'},name

def closure(name):
    result={name}
    for dep in packages[name]: result.update(closure(dep))
    return result

for name in ['razer-device','razer-model','razer-catalog','razer-assets','razer-widgets']:
    assert not closure(name) & {'razer-hid','razer-service','razer-shell','razer-app','razer-app-pages'},name
for name in ['razer-agent','razer-service','razer-ipc','razer-discovery','razer-platform']:
    assert not closure(name) & {'razer-shell','razer-app-pages','razer-pages','razer-widgets','razer-assets','razer-state','razer-settings','razer-tray','razer-dashboard'},name
for directory in (ROOT/'crates').iterdir():
    manifest=tomllib.loads((directory/'Cargo.toml').read_text('utf-8'))
    for name,dependency in manifest.get('dependencies',{}).items():
        if name.startswith('razer-'):
            assert dependency=={'workspace':True},(directory,name)
assert len(list((ROOT/'crates/razer-app/src').rglob('*.rs')))==1
architecture=json.loads((ROOT/'docs/re/workspace-openlogi-architecture-current.json').read_text('utf-8'))
for entry in architecture['source_receipts']:
    source=ROOT/entry['cache_path']
    if source.exists():
        assert hashlib.sha256(source.read_bytes()).hexdigest()==entry['sha256'],entry['path']
for entry in architecture['current_packages']:
    assert set(entry['dependencies'])==packages[entry['name']],entry['name']
    assert len(list((ROOT/'crates'/entry['name']/'src').rglob('*.rs')))==entry['rust_files'],entry['name']

reference=json.loads((ROOT/'docs/re/reference-src-current.json').read_text('utf-8'))
if (ROOT/'src').is_dir():
    for entry in reference['files']:
        assert hashlib.sha256((ROOT/entry['path']).read_bytes()).hexdigest()==entry['sha256'],entry['path']
    reference_location='local reference'
else:
    # The obsolete Rust tree was removed in the current repository. Verify its
    # preserved Git bytes without restoring files or using them as build input.
    # The receipt was made from a Windows git archive: 443 files have CRLF,
    # whereas Git stores LF. Accept only the exact recorded byte digest of the
    # blob or its LF-to-CRLF checkout representation, never normalized content.
    specifications=''.join(f"{reference['commit']}:{entry['path']}\n" for entry in reference['files'])
    process=subprocess.run(['git','cat-file','--batch'],input=specifications.encode(),
                           stdout=subprocess.PIPE,stderr=subprocess.PIPE,cwd=ROOT,check=True)
    output=process.stdout;offset=0;checkout_files=0
    for entry in reference['files']:
        end=output.index(b'\n',offset);header=output[offset:end].split()
        assert len(header)==3 and header[1]==b'blob',entry['path']
        size=int(header[2]);offset=end+1;body=output[offset:offset+size]
        if hashlib.sha256(body).hexdigest()!=entry['sha256']:
            assert b'\r' not in body,entry['path']
            checkout=body.replace(b'\n',b'\r\n')
            assert hashlib.sha256(checkout).hexdigest()==entry['sha256'],entry['path']
            checkout_files+=1
        offset+=size;assert output[offset:offset+1]==b'\n';offset+=1
    assert offset==len(output)
    reference_location=f'Git reference ({checkout_files} exact CRLF archive representations)'
relocation=json.loads((ROOT/'docs/re/workspace-relocation-current.json').read_text('utf-8'))
for entry in relocation['current_files']:
    assert hashlib.sha256((ROOT/entry['path']).read_bytes()).hexdigest()==entry['sha256'],entry['path']
source_files=[*(ROOT/'crates').rglob('*.rs'),*(ROOT/'app').rglob('*.rs')]
include=re.compile(r'include(?:_bytes|_str)?!\(\s*"([^"]+)"')
for source in source_files:
    raw=source.read_bytes(); text=raw.decode('utf-8')
    assert b'\r\r\n' not in raw, source
    assert '\ufffd' not in text and '???' not in text, source
    assert not re.search(r'^pub use razer_\w+\s+as\s+(?:resources|product|i18n|backend|preferences|ui);',text,re.M),source
    assert not re.search(r'\brazer_(?:service::backend|widgets::ui|pages::ui)\b',text),source
    for match in include.finditer(text):
        target=(source.parent/match[1]).resolve()
        assert target.exists(),(source,match[1])
        assert not target.is_relative_to(ROOT/'src'),('reference used as build input',source,target)
for name in ['razer-hid','razer-device']:
    for p in (ROOT/'crates'/name/'src').rglob('*.rs'):
        text=p.read_text('utf-8')
        assert not re.search(r'\b(?:use|extern crate)\s+(?:windows_sys|libloading|gpui_kit)\b',text),p
transport=(ROOT/'crates/razer-service/src/runtime_hid_transport.rs').read_text('utf-8')
assert 'libloading' not in transport and 'include_bytes!' not in transport
assert 'with_backend' in transport
print(f'Workspace: {len(packages)-1} independent libraries + UI/agent executables; {len(reference["files"])} original files verified in {reference_location}; {len(source_files)} active Rust files UTF-8; include/dependency boundaries and relocation fingerprints valid')
