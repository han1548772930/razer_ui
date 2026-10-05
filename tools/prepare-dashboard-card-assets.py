"""Refresh only the Dashboard state-artwork registrations."""
import json
from pathlib import Path
from dashboard_card_assets import prepare

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "assets/synapse"
records = prepare(ROOT, OUT)
manifest_path = OUT / "manifest.json"
manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
old = manifest["entries"]
slots = [i for i, entry in enumerate(old) if entry["output"].startswith("assets/synapse/dashboard-card/")]
assert not slots or slots == list(range(slots[0], slots[-1] + 1))
first, last = (slots[0], slots[-1] + 1) if slots else (len(old), len(old))
manifest["entries"] = old[:first] + records + old[last:]
manifest_path.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
embedded_path = OUT / "embedded.rs"
lines = embedded_path.read_text(encoding="utf-8").splitlines(keepends=True)
slots = [i for i, line in enumerate(lines) if '("synapse/dashboard-card/' in line]
assert not slots or slots == list(range(slots[0], slots[-1] + 1))
first, last = (slots[0], slots[-1] + 1) if slots else (len(lines) - 1, len(lines) - 1)
new_lines = [f'    ("synapse/dashboard-card/{Path(r["output"]).name}", include_bytes!("dashboard-card/{Path(r["output"]).name}") as &[u8]),\n' for r in records]
embedded_path.write_text("".join(lines[:first] + new_lines + lines[last:]), encoding="utf-8")
print(f"Prepared {len(records)} current Dashboard state images")
