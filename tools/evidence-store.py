"""Lossless storage for large reverse-engineering JSON evidence.

ZIP members retain the exact original bytes, paths and content digests. Existing
static tools run against temporary materialized originals, without changing
their parsers, source receipts, generator identities or output schemas.
"""
from __future__ import annotations

import argparse
import gzip
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import zipfile

ROOT = Path(__file__).resolve().parents[1]
STORE = ROOT / "docs/re/evidence"
INDEX = STORE / "index.json"


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def target(relative: str) -> Path:
    resolved = (ROOT / relative).resolve()
    if not resolved.is_relative_to(ROOT / "docs/re"):
        raise ValueError(f"Evidence path escapes docs/re: {relative}")
    return resolved


def index() -> dict:
    if INDEX.exists():
        data = json.loads(INDEX.read_text("utf-8"))
        if data["schema_version"] != 1:
            raise ValueError("Unsupported evidence index")
        return data
    return {"schema_version": 1, "method": "Lossless ZIP; original evidence bytes unchanged", "files": []}


def save(data: dict) -> None:
    STORE.mkdir(parents=True, exist_ok=True)
    INDEX.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")


def unpack(entry: dict) -> bytes:
    archive = target(entry["archive_path"])
    raw = archive.read_bytes()
    if sha(raw) != entry["archive_sha256"]:
        raise ValueError(f"Archive identity mismatch: {archive}")
    compression = entry.get("compression", "zip")
    if compression == "gzip":
        body = gzip.decompress(raw)
    elif compression == "zip":
        with zipfile.ZipFile(archive) as zipped:
            if zipped.namelist() != [entry["member"]]:
                raise ValueError(f"Unexpected archive members: {archive}")
            member = zipped.getinfo(entry["member"])
            if member.file_size != entry["original_bytes"]:
                raise ValueError(f"Member size mismatch: {archive}")
            body = zipped.read(member)
    else:
        raise ValueError(f"Unsupported evidence compression: {compression}")
    if len(body) != entry["original_bytes"] or sha(body) != entry["original_sha256"]:
        raise ValueError(f"Original evidence identity mismatch: {archive}")
    return body


@contextmanager
def lock():
    directory = ROOT / ".work"
    directory.mkdir(exist_ok=True)
    with (directory / "evidence-store.lock").open("a+b") as handle:
        if handle.tell() == 0:
            handle.write(b"0")
            handle.flush()
        handle.seek(0)
        if os.name == "nt":
            import msvcrt
            msvcrt.locking(handle.fileno(), msvcrt.LK_LOCK, 1)
        else:
            import fcntl
            fcntl.flock(handle, fcntl.LOCK_EX)
        try:
            yield
        finally:
            handle.seek(0)
            if os.name == "nt":
                msvcrt.locking(handle.fileno(), msvcrt.LK_UNLCK, 1)
            else:
                fcntl.flock(handle, fcntl.LOCK_UN)


def archive(min_mib: float) -> None:
    data = index()
    known = {entry["original_path"]: entry for entry in data["files"]}
    # Only individual JSON files under the named evidence directory are removed.
    # No recursive delete, vendor source or runtime asset is involved.
    for source in sorted((ROOT / "docs/re").glob("*.json")):
        relative = source.relative_to(ROOT).as_posix()
        if source.stat().st_size < min_mib * 1024 * 1024 and relative not in known:
            continue
        body = source.read_bytes()
        json.loads(body)
        existing = known.get(relative)
        if existing and sha(body) == existing["original_sha256"]:
            if unpack(existing) != body:
                raise ValueError(f"Archive roundtrip differs: {source}")
            cleanup([existing])
            continue
        archive_path = STORE / (source.name + ".zip")
        if archive_path.exists() and relative not in known:
            raise ValueError(f"Unregistered existing archive: {archive_path}")
        member = zipfile.ZipInfo(source.name, date_time=(2000, 1, 1, 0, 0, 0))
        member.compress_type = zipfile.ZIP_DEFLATED
        member.external_attr = 0o100644 << 16
        temporary = archive_path.with_suffix(".zip.tmp")
        STORE.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(temporary, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zipped:
            zipped.writestr(member, body, compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
        compressed = temporary.read_bytes()
        entry = {"original_path": relative, "archive_path": archive_path.relative_to(ROOT).as_posix(),
                 "member": source.name, "original_bytes": len(body), "original_sha256": sha(body),
                 "archive_bytes": len(compressed), "archive_sha256": sha(compressed)}
        temporary.replace(archive_path)
        if unpack(entry) != body:
            raise ValueError(f"Archive roundtrip differs: {source}")
        known[relative] = entry
        data["files"] = sorted(known.values(), key=lambda entry: entry["original_path"])
        # Persist the restore map before removing any original.
        save(data)
        if sha(source.read_bytes()) != entry["original_sha256"]:
            raise ValueError(f"Evidence changed during packing: {source}")
        target(relative).unlink()
    rewrite_links(data)
    report(data)


def rewrite_links(data: dict) -> None:
    import re
    links = re.compile(r"(\[[^\]\n]*\]\()([^)\n]+)(\))")
    mapped = {target(entry["original_path"]): target(entry["archive_path"]) for entry in data["files"]}
    for document in [ROOT / "README.md", *sorted((ROOT / "docs").rglob("*.md"))]:
        text = document.read_text("utf-8")

        def replace(match):
            name, separator, fragment = match[2].partition("#")
            original = (document.parent / name.strip("<>")).resolve()
            archived = mapped.get(original)
            if archived is None:
                return match[0]
            relative = os.path.relpath(archived, document.parent).replace("\\", "/")
            return match[1] + relative + (separator + fragment if separator else "") + match[3]

        updated = links.sub(replace, text)
        if updated != text:
            document.write_text(updated, encoding="utf-8", newline="\n")


def materialize(data: dict) -> list[dict]:
    created = []
    try:
        for entry in data["files"]:
            destination = target(entry["original_path"])
            if destination.exists():
                # Existing work is never overwritten. It may be a newer audit.
                continue
            body = unpack(entry)
            with destination.open("xb") as output:
                output.write(body)
            created.append(entry)
    except BaseException:
        cleanup(created)
        raise
    return created


def cleanup(created: list[dict]) -> None:
    failures = []
    for entry in created:
        destination = target(entry["original_path"])
        # Windows indexers may briefly hold freshly restored files. Retry only
        # sharing/lock violations, recheck the digest each time and continue
        # cleaning the remaining copies if a file stays locked.
        for attempt in range(6):
            try:
                if not destination.exists():
                    break
                if sha(destination.read_bytes()) != entry["original_sha256"]:
                    print(f"Retained modified evidence: {entry['original_path']}", file=sys.stderr)
                    break
                destination.unlink()
                break
            except OSError as error:
                if getattr(error, "winerror", None) not in {32, 33} or attempt == 5:
                    failures.append((entry["original_path"], error))
                    break
                time.sleep(0.1 * (attempt + 1))
    if failures:
        for relative, error in failures:
            print(f"Retained evidence copy: {relative}: {error}", file=sys.stderr)
        raise OSError("Some temporary evidence copies could not be removed; archives are intact")


def report(data: dict) -> None:
    original = sum(entry["original_bytes"] for entry in data["files"])
    packed = sum(entry["archive_bytes"] for entry in data["files"])
    print(f"Evidence: {len(data['files'])} exact-byte archives; "
          f"{original / 2**20:.2f} -> {packed / 2**20:.2f} MiB; saved {(original-packed) / 2**20:.2f} MiB")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    packing = commands.add_parser("archive", help="Pack large JSON and update Markdown links")
    packing.add_argument("--min-mib", type=float, default=2)
    commands.add_parser("check", help="Verify archives, originals, schemas and all digests")
    commands.add_parser("materialize", help="Restore all originals for browsing or existing tools")
    commands.add_parser("dematerialize", help="Remove unchanged restored copies; preserve all archives and edits")
    runner = commands.add_parser("run", help="Temporarily restore evidence for a maintained static check")
    runner.add_argument("script")
    runner.add_argument("arguments", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    with lock():
        data = index()
        if args.command == "archive":
            archive(args.min_mib)
        elif args.command == "check":
            for entry in data["files"]:
                json.loads(unpack(entry))
                restored = target(entry["original_path"])
                if restored.exists() and sha(restored.read_bytes()) != entry["original_sha256"]:
                    raise ValueError(f"Unpacked evidence changed; regenerate archive: {restored}")
            report(data)
        elif args.command == "materialize":
            restored = materialize(data)
            print(f"Restored {len(restored)} evidence files; existing files left intact")
        elif args.command == "dematerialize":
            cleanup(data["files"])
            print("Removed unchanged restored copies; archives and modified files retained")
        elif args.command == "run":
            script = (ROOT / args.script).resolve()
            if not script.is_relative_to(ROOT / "tools") or script.suffix not in {".py", ".cjs"}:
                raise ValueError("Only maintained Python/Node static tools under tools/ can be run")
            if "--check" not in args.arguments and not script.name.startswith("validate-"):
                raise ValueError("Use --check; this runner does not invoke applications or hardware")
            executable = [sys.executable, "-X", "utf8"] if script.suffix == ".py" else ["node"]
            created = materialize(data)
            try:
                return subprocess.call([*executable, str(script), *args.arguments], cwd=ROOT)
            finally:
                cleanup(created)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
