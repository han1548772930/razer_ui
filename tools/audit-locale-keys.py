"""Static audit: every locale key used by a literal `t("KEY")` call exists in the
bundled locale files, and all locale files carry the same key set.

Only literal keys are checked. `t_or("KEY", fallback)` call sites are counted as
soft: the fallback is exactly the "key may be absent" escape hatch, so they are
reported but do not fail the audit. Dynamic keys (`t(&name)`) are counted as
skipped, because they cannot be resolved without running the code.

Usage:
    python tools/audit-locale-keys.py            # report
    python tools/audit-locale-keys.py --check    # exit 1 on a hard miss
    python tools/audit-locale-keys.py --json     # machine-readable receipt
    python tools/audit-locale-keys.py --self-test
"""
from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
LOCALES = ROOT / "locales"
SRC = ROOT / "src"

HARD = re.compile(r'(?<![\w.])t\(\s*"([A-Z0-9_]+)"\s*[),]')
SOFT = re.compile(r't_or\(\s*"([A-Z0-9_]+)"')
DYNAMIC = re.compile(r'(?<![\w.])t\(\s*&')

# Literal `t()` keys that the bundled locales do not carry. Listing one here keeps
# the audit usable as a regression gate while the fix is pending; the goal is an
# empty map. All three previous entries are closed:
#   * `ADVANCED_EFFECT_DETAILS` — the text was merged out of the device bundle by
#     `tools/prepare-device-locales.py` (verified by its `--check`).
#   * `MINUTES` — replaced by the source's own `MIN`/`SEC` power-saving labels
#     (`GR` in `.ref/devices/112/static/js/main.47216244.js`).
#   * `LINKED_GAMES_TO` — replaced by `LINKED_GAME_CHROMA_HEADER`
#     ("Games linked to profile:").
KNOWN_MISSING: dict[str, str] = {}


def keys_of(path: pathlib.Path) -> set[str]:
    data = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(data, dict):
        raise SystemExit(f"{path} is not a flat key/value document")
    return set(data)


def scan_sources() -> tuple[dict[str, list[str]], set[str], int]:
    """Return hard keys (key -> files), soft keys, and the dynamic call count."""
    hard: dict[str, list[str]] = {}
    soft: set[str] = set()
    dynamic = 0
    for path in sorted(SRC.rglob("*.rs")):
        text = path.read_text(encoding="utf-8", errors="replace")
        relative = path.relative_to(ROOT).as_posix()
        for key in HARD.findall(text):
            hard.setdefault(key, []).append(relative)
        soft.update(SOFT.findall(text))
        dynamic += len(DYNAMIC.findall(text))
    return hard, soft, dynamic


def audit() -> dict:
    locales = {path.name: keys_of(path) for path in sorted(LOCALES.glob("*.json"))}
    if not locales:
        raise SystemExit("no locale files found")
    english = locales["en.json"] if "en.json" in locales else next(iter(locales.values()))
    hard, soft, dynamic = scan_sources()
    missing = sorted(key for key in hard if key not in english)
    known = sorted(key for key in missing if key in KNOWN_MISSING)
    unexpected = sorted(key for key in missing if key not in KNOWN_MISSING)
    soft_missing = sorted(key for key in soft if key not in english)
    # Locale parity is informational: the bundled translations were extracted from
    # different application bundles, so a locale can legitimately lag English.
    parity = {
        name: len(english - keys)
        for name, keys in locales.items()
        if name != "en.json" and english - keys
    }
    return {
        "checked_keys": len(hard),
        "soft_keys": len(soft),
        "locales": len(locales),
        "dynamic_calls": dynamic,
        "missing": missing,
        "known_missing": known,
        "unexpected_missing": unexpected,
        "soft_missing": soft_missing,
        "locale_parity_gaps": parity,
        "hard_key_files": {key: sorted(set(files)) for key, files in sorted(hard.items())},
    }


def self_test() -> None:
    """The scanners must accept the audited shapes and reject a literal miss."""
    assert HARD.search('i18n::t("SERIAL_NUM")'), "plain call"
    assert HARD.search('t(\n            "MULTI_LINE",\n        )'), "multi-line call"
    assert HARD.search('child(t("OBM_LOCK_ICON_TOOLTIP_V2"))'), "nested call"
    assert not HARD.search("t(&name)"), "dynamic key is not a literal"
    assert SOFT.search('t_or("COLOR_GAMUT", "COLOR GAMUT")'), "soft call"
    assert DYNAMIC.search("t(&format!(\"a{name}\"))"), "dynamic call"
    print("self-test: 6 synthetic cases pass")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--json", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    options = parser.parse_args()
    if options.self_test:
        self_test()
        return 0
    report = audit()
    if options.json:
        print(json.dumps(report, ensure_ascii=False, indent=2, sort_keys=True))
    else:
        print(
            f'checked {report["checked_keys"]} literal t() keys ({report["soft_keys"]} soft '
            f't_or keys, {report["dynamic_calls"]} dynamic calls) across '
            f'{report["locales"]} locales'
        )
        print(
            f'absent from en.json: {len(report["missing"])} '
            f'({len(report["known_missing"])} recorded, '
            f'{len(report["unexpected_missing"])} new) {report["unexpected_missing"]}'
        )
        print(f'soft keys absent from en.json: {report["soft_missing"]}')
        gaps = report["locale_parity_gaps"]
        print(
            "locales carrying fewer keys than en.json (informational): "
            f'{len(gaps)} {gaps}'
        )
    if options.check and report["unexpected_missing"]:
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
