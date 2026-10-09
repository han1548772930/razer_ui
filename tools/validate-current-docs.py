"""Validate maintained Markdown encoding, local links, and current-only policy."""
from __future__ import annotations

import re
from pathlib import Path
from urllib.parse import unquote

ROOT = Path(__file__).resolve().parents[1]
LINK = re.compile(r"\[[^\]\n]*\]\(([^)\n]+)\)")
# Bracketed function calls inside Markdown code are not links. Matching the
# exact backtick delimiter also excludes fenced examples from link scanning.
CODE = re.compile(r"(?P<ticks>`+)(?!`)[\s\S]*?(?<!`)(?P=ticks)(?!`)")
OBSOLETE = (".ref/frontend/", ".ref/synapse-asar/", ".ref/host-4.0.821/",
            ".work/latest-source-check/host-4.0.821/")


def main() -> int:
    errors: list[str] = []
    documents = [ROOT / "README.md", *sorted((ROOT / "docs").rglob("*.md"))]
    links = 0
    for doc in documents:
        name = doc.relative_to(ROOT).as_posix()
        try:
            content = doc.read_text(encoding="utf-8")
        except UnicodeDecodeError as error:
            errors.append(f"{name}: invalid UTF-8 at byte {error.start}")
            continue
        if "\ufffd" in content:
            errors.append(f"{name}: replacement character")
        if "来源迁移（" in content:
            errors.append(f"{name}: unreconciled obsolete-source notice")
        if re.search(r"(?:continuation|followup|review)-20\d\d-\d\d-\d\d\.md$", doc.name):
            errors.append(f"{name}: dated process report; consolidate current facts")
        for match in LINK.finditer(CODE.sub(lambda match: " " * len(match[0]), content)):
            target = match[1].strip().strip("<>")
            if re.match(r"(?:[a-z]+:|#|//)", target, re.I):
                continue
            target = unquote(target.split("#", 1)[0]).replace("\\", "/")
            if not target:
                continue
            path = (doc.parent / target).resolve()
            if not path.is_relative_to(ROOT):
                continue
            relative = path.relative_to(ROOT).as_posix()
            if relative.startswith(OBSOLETE):
                errors.append(f"{name}: obsolete source link {target}")
                continue
            links += 1
            if not path.exists():
                errors.append(f"{name}: missing link {target}")
    if errors:
        print("\n".join(errors))
        return 1
    print(f"Current documentation: {len(documents)} UTF-8 documents; {links} local links valid")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
