"""Extract every current official ASAR entry as bytes; never execute entries."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]
DEST = ROOT / ".ref/host-4.0.827"
OUTPUT = ROOT / "docs/re/host-full-asar-current-evidence.json"


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    chain = json.loads((DEST / "ARCHIVE-CHAIN.json").read_text(encoding="utf-8"))
    archive = ROOT / chain["asar"]["source_archive"]
    body = archive.read_bytes()
    assert len(body) == chain["asar"]["source_archive_bytes"]
    assert sha(body) == chain["asar"]["source_archive_sha256"]
    package = ROOT / ".work/latest-source-check/host-current/static-unpack" / chain["internal_exe"]["entry"]
    assert package.stat().st_size == chain["internal_exe"]["bytes"]
    assert sha(package.read_bytes()) == chain["internal_exe"]["sha256"]
    size_pickle, header_size, header_pickle, json_size = struct.unpack("<4I", body[:16])
    assert size_pickle == 4 and json_size <= header_pickle <= header_size
    data_start = 8 + header_size
    assert 16 + json_size <= data_start <= len(body)
    header = json.loads(body[16:16 + json_size])
    rows = []

    def entries(files: dict, prefix: str = ""):
        for name, metadata in sorted(files.items()):
            assert name not in ("", ".", "..") and "/" not in name and "\\" not in name and ":" not in name
            entry = prefix + name
            if "files" in metadata:
                yield from entries(metadata["files"], entry + "/")
            else:
                yield entry, metadata

    for entry, metadata in entries(header["files"]):
        assert not metadata.get("link"), "Symlink entries are not extracted"
        target = DEST.joinpath(*PurePosixPath(entry).parts)
        assert target.resolve().is_relative_to(DEST.resolve())
        size = metadata["size"]
        assert isinstance(size, int) and size >= 0
        unpacked = bool(metadata.get("unpacked"))
        if unpacked:
            package_entry = "win-unpacked/resources/app.asar.unpacked/" + entry
            if args.check:
                assert target.is_file(), "Missing extracted unpacked entry: " + entry
                source = target.read_bytes()
            else:
                completed = subprocess.run(
                    ["C:/Program Files/7-Zip/7z.exe", "e", "-so", "-bd", str(package), package_entry],
                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=False)
                assert completed.returncode in (0, 1) and completed.stdout, completed.stderr.decode("utf-8", errors="replace")
                source = completed.stdout
            # Authenticode signing can enlarge native entries after ASAR header
            # generation. Record the difference; do not truncate signed bytes.
            if entry.endswith((".node", ".dll", ".exe")):
                assert source.startswith(b"MZ")
        else:
            offset = int(metadata["offset"])
            assert offset >= 0 and data_start + offset + size <= len(body)
            source = body[data_start + offset:data_start + offset + size]
            package_entry = None
        integrity = metadata.get("integrity")
        integrity_matches = None
        blocks_match = None
        if integrity:
            assert integrity["algorithm"] == "SHA256"
            integrity_matches = sha(source) == integrity["hash"]
            block_size = integrity["blockSize"]
            assert isinstance(block_size, int) and block_size > 0
            blocks = [sha(source[i:i + block_size]) for i in range(0, len(source), block_size)]
            blocks_match = blocks == integrity["blocks"]
            if not unpacked:
                assert integrity_matches, "ASAR integrity mismatch: " + entry
                assert blocks_match, "ASAR block integrity mismatch: " + entry
        if target.exists():
            assert target.read_bytes() == source, "Existing current source differs; preserved: " + entry
        else:
            assert not args.check, "Missing packed source: " + entry
            target.parent.mkdir(parents=True, exist_ok=True)
            with target.open("xb") as output:
                output.write(source)
        rows.append({"entry": entry, "path": target.relative_to(ROOT).as_posix(),
                     "bytes": len(source), "sha256": sha(source), "asar_declared_size": size,
                     "unpacked": unpacked, "archive_entry": package_entry,
                     "asar_integrity_verified": integrity_matches,
                     "asar_integrity_hash": integrity.get("hash") if integrity else None,
                     "asar_blocks_match": blocks_match,
                     "size_matches_asar_metadata": len(source) == size})
        if len(rows) % 1000 == 0:
            print(f"ASAR: {len(rows)} entries inspected", flush=True)
    result = {"schema_version": 1, "method": "Verified current ASAR header/boundary/integrity parsing and 7-Zip static unpacked byte extraction; no code executed",
              "source_archive_sha256": chain["asar"]["source_archive_sha256"],
              "internal_package_sha256": chain["internal_exe"]["sha256"],
              "generator_sha256": sha(Path(__file__).read_bytes().replace(b"\r\n", b"\n")),
              "summary": {"entries": len(rows), "bytes": sum(row["bytes"] for row in rows),
                          "packed_entries": sum(not row["unpacked"] for row in rows),
                          "unpacked_entries": sum(row["unpacked"] for row in rows),
                          "integrity_verified_entries": sum(row["asar_integrity_verified"] is True for row in rows),
                          "unpacked_integrity_differences": [row["entry"] for row in rows if row["asar_integrity_verified"] is False],
                          "unpacked_size_differences": [row["entry"] for row in rows if not row["size_matches_asar_metadata"]]},
              "files": rows,
              "boundaries": ["Complete ASAR extraction includes third-party dependencies, not just first-party semantic review.",
                             "An extraction/hash check does not recover native machine-code semantics, original C++ source or a remote service implementation.",
                             "ASAR unpacked sizes may predate Authenticode signing; official archive bytes are preserved in full.",
                             "Unpacked ASAR integrity hash/block differences are recorded, not suppressed; source provenance is the hash-verified official internal archive. This scan does not claim the pre-signing native bytes match the final signed entry.",
                             "--check compares packed bytes against ASAR; unpacked files against the extraction evidence hash, without executing or re-extracting them."],
              "runtime_validation": "not_run"}
    serialized = json.dumps(result, ensure_ascii=False, indent=2) + "\n"
    if args.check:
        assert OUTPUT.read_text(encoding="utf-8").replace("\r\n", "\n") == serialized, "Host full ASAR evidence changed"
    else:
        OUTPUT.write_text(serialized, encoding="utf-8")
    print(json.dumps(result["summary"], ensure_ascii=False))


if __name__ == "__main__":
    main()
