"""Read-only validation of current UI fixes; never bless changed fingerprints."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REGISTRY = ROOT / "docs/re/ui-fix-registry.json"
FORBIDDEN = (".ref/frontend/", ".ref/synapse-asar/", ".ref/host-4.0.821/",
             ".work/latest-source-check/host-4.0.821/")


def checked_path(name: str) -> Path:
    normalized = name.replace("\\", "/")
    if normalized.startswith(FORBIDDEN):
        raise ValueError(f"obsolete source: {name}")
    path = (ROOT / normalized).resolve()
    if not path.is_relative_to(ROOT):
        raise ValueError(f"outside workspace: {name}")
    if path.relative_to(ROOT).as_posix().startswith(FORBIDDEN):
        raise ValueError(f"obsolete source: {name}")
    return path


def main() -> int:
    data = json.loads(REGISTRY.read_text(encoding="utf-8"))
    assert data["schema_version"] == 1
    assert data["fingerprints"] == {
        "implementation": "sha256_lf", "evidence": "sha256_lf", "source_inputs": "sha256_bytes"
    }
    ids: set[str] = set()
    errors: list[str] = []
    checked: dict[tuple[str, str], str] = {}
    for entry in data["fixes"]:
        key = entry["id"]
        assert key and key not in ids, f"duplicate fix: {key}"
        ids.add(key)
        assert entry["behavior"] and entry["checks"] and entry["remaining"]
        assert entry["state"] == "implemented_static_checked"
        assert entry["runtime_validation"] == "not_run"
        assert checked_path(entry["contract"]).is_file(), entry["contract"]
        for group in ("implementation", "evidence", "source_inputs"):
            assert entry[group], f"missing {group}: {key}"
            for name, expected in entry[group].items():
                path = checked_path(name)
                if not path.is_file():
                    errors.append(f"{key}: missing {name}")
                    continue
                cache_key = (group, name)
                if cache_key not in checked:
                    content = path.read_bytes()
                    if group != "source_inputs":
                        content = content.replace(b"\r\n", b"\n")
                    checked[cache_key] = hashlib.sha256(content).hexdigest()
                if checked[cache_key] != expected:
                    errors.append(f"{key}: changed {name}; review this fix before updating its record")
    if errors:
        print("\n".join(errors))
        return 1
    print(f"Current UI fixes: {len(ids)}; unchanged inputs: {len(checked)}; runtime not run")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
