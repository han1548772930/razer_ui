# Local asset pack format

`razer-asset-pack` is an OS-independent, read-only asset container core. Production `SynapseAssets` delegates its large static resource tables to the indexed package reader rather than compiling those image bytes into the executable. The change does not convert images, package original analysis inputs, or replace GPUI fallback behavior. Version 1 stores unmodified bytes: it deduplicates identical content but does not compress it. Source coverage, full-resource verification and application runtime acceptance remain separate checks; a loader implementation alone does not complete the migration.

## Production loading and preparation

Original resource files can also be restored from the private Hugging Face Dataset
`han1548772930/razer-ui-assets`. The dataset uses Git LFS and preserves the complete
`assets/` paths, without ZIP archives or the generated runtime pack. Rust resource
tables and the nine native tray/menu RGBA files remain owned by the source checkout.
`tools/asset-release.json` records every file's length/SHA-256, product dependencies
and a verified immutable dataset commit. The storage commit is omitted from the
dataset's own `release-index.json` to avoid a self-referential hash.

Configure Git LFS and Git Credential Manager access to the private dataset, then run
`python tools/workspace.py fetch-asset-release` before the offline preparation below.
`--product PID` restores that product's source files plus shared/transitive dependencies;
full workspace compilation and pack preparation require all products. A prepared raw
snapshot can be restored offline with `--from-dir DIRECTORY`. Modified local files are
preserved unless `--replace-existing` is explicit. All requested changes are staged
and hashed before replacement; failed replacement rolls back previous changes, and
failed rollback retains original backups under `.work/asset-recovery/`.

Maintainers use `prepare-asset-release --repository OWNER/DATASET`,
`verify-asset-release --check-local`, and `publish-asset-release` through the same
`tools/workspace.py` entry point. Publication verifies the remote file set, every LFS
SHA-256/length and the manifest before pinning the commit. `verify-asset-release --remote`
repeats that verification. These commands do not push or rewrite the source repository.

The source declarations remain in `assets/synapse/tables/asset-tables-source.rs` for the maintained tool to parse; they are not compiled into the application. `assets/synapse/indexes/asset-list.json` preserves the original table enumeration order and duplicate aliases. The loader retains the dynamic SVG fragment selection and small demo-control provider ahead of static lookup, plus GPUI's fallback for genuinely unregistered keys. Missing/corrupt declared package entries are errors, not fallback placeholders. Font registration and direct JSON/locale consumers retain their original owners rather than receiving invented package keys.

`python tools/workspace.py audit-asset-pack --prepare` captures the complete static source graph and emits its list contract. `prepare-asset-pack` requires stable input identities and no conflicting aliases, writes a new temporary pack in 1 MiB chunks, validates all unique blobs and alias mappings, rechecks source snapshots, then atomically publishes `target/asset-pack/synapse.pack`. This generated build resource stays outside the source `assets/` tree and the HF raw-file snapshot. `verify-asset-pack` repeats the stream checks independently. Analysis receipts stay in `.analysis/results/`; production reads neither those receipts nor scratch paths. The Python wire writer is checked against the crate-owned `testdata/store-v1.hex` also tested by Rust.

The production loader opens `assets/synapse.pack` relative to the executable, validates its complete key set and index once, retains the file/index behind a mutex, and reads/hashes only requested entries. Application startup validates availability without unpacking the entire container. Cargo's `razer-assets/build.rs` stages `target/asset-pack/synapse.pack` to the profile's `assets/` directory using bounded byte comparisons and temporary-file replacement; a missing generated pack is a build error with preparation instructions. Test binaries in `deps/` use the profile's staged pack. Installed distributions must include the same relative `assets/synapse.pack`; no Internet fallback is provided. These build/test paths perform resource preparation only and do not launch the application.

## Layout

All integers are unsigned little-endian. Offsets are absolute from byte zero. There is no alignment padding, timestamp, platform path, or compression dictionary. The 96-byte header is followed by the entire index and then contiguous unique payloads.

| Header offset | Bytes | Value |
| --- | --- | --- |
| 0 | 8 | ASCII `RZASPACK` |
| 8 | 4 | Version, currently `1` |
| 12 | 4 | Flags, must be zero |
| 16 | 8 | Key entry count |
| 24 | 8 | Unique blob count |
| 32 | 8 | Index byte length |
| 40 | 8 | Payload start, exactly `96 + index length` |
| 48 | 8 | Complete file length |
| 56 | 32 | SHA-256 of the index bytes |
| 88 | 8 | Reserved, must be zero |

The index begins with key entries sorted strictly by UTF-8 bytes. Each is `key_length: u32`, exactly that many UTF-8 bytes, and `blob_index: u32`. Keys are exact, case-sensitive relative virtual paths; no Unicode normalization occurs. Empty keys, control characters, colon, backslash, empty slash-separated segments, `.` and `..` segments are rejected. Keys have no trailing NUL. Duplicate keys fail, even if their content matches.

Following key entries are fixed 64-byte blob records, strictly sorted by SHA-256 bytes:

| Blob field offset | Bytes | Value |
| --- | --- | --- |
| 0 | 4 | Codec, `0` means Store |
| 4 | 4 | Flags, must be zero |
| 8 | 8 | Absolute payload offset |
| 16 | 8 | Stored byte length |
| 24 | 8 | Decoded byte length; must equal stored length for Store |
| 32 | 32 | SHA-256 of the unmodified content bytes |

Different paths may refer to the same blob index. All blobs must be referenced. Duplicate blob hashes, gaps, overlaps, trailing bytes, truncated records, invalid UTF-8, unknown versions, flags and codecs fail. Zero-length content is allowed and deduplicated. The writer compares actual bytes before accepting hash-based deduplication and reports any hash collision instead of silently substituting content.

## Reader and writer contracts

- `PackReader::open(Read + Seek)` validates the fixed header, index hash, lengths, ordering, path rules and blob ranges. Opening reads only header/index bytes; it does not decompress or read any payload.
- `entry`, `keys` and `list` inspect the index without loading content. The format's `list` returns unique keys in canonical order, not the legacy `AssetSource` enumeration sequence.
- `read(key)` seeks to only the requested blob, reads at most that entry's validated length, and verifies SHA-256 before returning bytes. Missing keys return `None`; corrupt entries return errors, never missing or empty substitutes.
- `verify_all` is an explicit preparation/validation operation. It reads and hashes each unique blob once, retaining at most one payload at a time. It is not startup behavior.
- `write_pack` accepts borrowed input bytes and a `Write` sink. Input order does not affect the result. All semantic validation finishes before the first output byte. I/O errors may leave partial output, so the publication owner must write to a new temporary file, validate it, and publish atomically; this core never overwrites source files or performs installation.
- `write_pack_from_loader` accepts keys and a fallible loading callback. It retains only metadata plus at most two loaded entries during deduplication, rechecks each representative's length/hash before writing its payload, and rejects concurrent source changes. Callback failures during the second pass may leave partial output, subject to the same temporary-file publication contract. This supports full `AssetSource::list/load` roundtrips without retaining the complete resource set in memory.
- Default limits: one million keys, one million blobs, 4096 bytes/key, 64 MiB index, 256 MiB/entry, 16 GiB/file. `Limits` can be explicitly adjusted by the owning tool. Count/length/offset arithmetic is checked and allocation failures are errors.
- SHA-256 detects accidental corruption; this format is not a signature or authenticity mechanism. A malicious party able to replace data and hashes can construct a valid package. Publication provenance and trust are separate concerns.

Before feeding the writer, the publication manifest must resolve current `AssetSource::load` precedence, report same-key/different-content collisions, and normalize only reviewed identical duplicates. Preserve the original `list` sequence separately, including aliases, duplicates, dynamic sprite/fragment behavior and the GPUI fallback. A sorted unique pack index is not a replacement for those source semantics.

## Dependency and verification boundary

The core uses the already locked `sha2 0.10.9` and its shipped RustCrypto `src/lib.rs` one-shot `Digest` API documentation. Context7 lookup was unavailable due to its configured API key; API signatures were checked against the actual locked crate source. No new codec dependency or unsafe mapping is introduced. Pure Rust tests cover deterministic bytes, aliases, lazy reads, corruption, truncation, duplicate keys, unknown codec/version/flags, bounds/overflow, invalid paths and I/O failure. Tests use in-memory owned fixtures and do not depend on `.analysis/` or `.work/`. No application, vendor code or device is run.
