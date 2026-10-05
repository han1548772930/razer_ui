"""Refresh just Macro's original SVGs and registrations without bitmap/font DLLs."""
import json
from pathlib import Path
from macro_assets import prepare

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets/synapse"
manifest_path = OUT / "manifest.json"
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
old = manifest["entries"]
indices = [index for index, entry in enumerate(old) if entry["output"].startswith("assets/synapse/macro/")]
assert indices and indices == list(range(indices[0], indices[-1] + 1)), "Macro resource block changed"
records = prepare(ROOT, OUT)
manifest["entries"] = old[:indices[0]] + records + old[indices[-1] + 1:]
manifest_path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
embedded_path = OUT / "embedded.rs"
lines = embedded_path.read_text(encoding="utf-8").splitlines(keepends=True)
slots = [index for index, line in enumerate(lines) if '("synapse/macro/' in line]
assert slots and slots == list(range(slots[0], slots[-1] + 1)), "Macro registration block changed"
new_lines = [f'    ("synapse/{(ROOT / r["output"]).relative_to(OUT).as_posix()}", include_bytes!("{(ROOT / r["output"]).relative_to(OUT).as_posix()}") as &[u8]),\n' for r in records]
embedded_path.write_text("".join(lines[:slots[0]] + new_lines + lines[slots[-1] + 1:]), encoding="utf-8")
print(f"Prepared {len(records)} current Macro SVGs; {len(manifest['entries'])} main resource registrations")
