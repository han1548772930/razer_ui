"""Refresh current source font faces without loading bitmap libraries or DLLs."""
import json
from pathlib import Path
from font_assets import prepare

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets/synapse"
manifest_path = OUT / "manifest.json"
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
old = manifest["entries"]
slots = [i for i, entry in enumerate(old) if entry["output"].endswith(".ttf")]
assert slots and slots == list(range(slots[0], slots[-1] + 1)), "Font block changed"
records = prepare(ROOT, OUT)
manifest["entries"] = old[:slots[0]] + records + old[slots[-1] + 1:]
manifest_path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
embedded_path = OUT / "embedded.rs"
lines = embedded_path.read_text(encoding="utf-8").splitlines(keepends=True)
slots = [i for i, line in enumerate(lines) if '.ttf"' in line]
assert slots and slots == list(range(slots[0], slots[-1] + 1)), "Embedded font block changed"
new_lines = [f'    ("synapse/{Path(r["output"]).name}", include_bytes!("{Path(r["output"]).name}") as &[u8]),\n' for r in records]
embedded_path.write_text("".join(lines[:slots[0]] + new_lines + lines[slots[-1] + 1:]), encoding="utf-8")
print(f"Prepared {len(records)} CSS font faces; outline, cmap, layout and advance tables unchanged")
