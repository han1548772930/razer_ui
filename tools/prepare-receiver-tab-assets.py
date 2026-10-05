"""Update only receiver/favicon resources, preserving unrelated resource order."""
import json
from pathlib import Path
from receiver_tab_assets import prepare

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / 'assets/synapse'
records = prepare(ROOT, OUT)
selected = {r['output'] for r in records}
manifest_path = OUT / 'manifest.json'
manifest = json.loads(manifest_path.read_text(encoding='utf8'))
manifest['entries'] = [r for r in manifest['entries'] if r['output'] not in selected] + records
manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n', encoding='utf8')
embedded_path = OUT / 'embedded.rs'
lines = embedded_path.read_text(encoding='utf8').splitlines(keepends=True)
paths = [r['output'].removeprefix('assets/') for r in records]
lines = [line for line in lines if not any(f'("{p}",' in line for p in paths)]
end = next(i for i, line in enumerate(lines) if line.strip() in (']', '];'))
new_lines = [f'    ("{p}", include_bytes!("{p.removeprefix("synapse/")}") as &[u8]),\n' for p in paths]
embedded_path.write_text(''.join(lines[:end] + new_lines + lines[end:]), encoding='utf8')
print(f'Prepared {len(records)} receiver/favicon SVGs; {len(manifest["entries"])} main registrations')
