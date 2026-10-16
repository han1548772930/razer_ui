# Resource delivery and optional container format

## Application resource delivery

`SynapseAssets` resolves aliases against a compiled, typed Rust index and obtains individual files from the private Hugging Face Dataset `han1548772930/razer-ui-assets`. The dataset preserves product/shared paths and original file bytes through its own Git LFS rules. It contains no required ZIP or runtime pack. `tools/asset-release.json` pins an immutable dataset commit and each file's length/SHA-256; `assets/synapse/tables/remote-assets.rs` carries the generated runtime metadata. The dataset's own `release-index.json` omits its storage commit to avoid a self-referential hash.

Requests follow the current UI's actual file use. Startup does not fetch every product or restore the source checkout's `assets/`. Four asynchronous workers deduplicate pending file requests, read a valid installation cache before networking, validate downloaded lengths and SHA-256, and atomically publish verified bytes. Cached files are under the current executable's `assets/cache/`, named with their content hash to keep valid versions distinct. Cache hits need no network or credential. An uncached file remains unavailable offline; load failures and retries do not imply device state changes.

Private access credentials are read from explicit `HF_TOKEN`, the local Windows credential store, or the local `hf auth login` token file. They do not belong in the source, manifest, installation package or logs. The installation directory must be writable by the ordinary user. Upgrades preserve caches; uninstall offers retention or cleanup limited to this application's cache. See [Windows installation](windows-installation.md).

The source checkout retains Rust lookup/maintenance tables and nine mandatory native RGBA icons. Image, video and font payloads are ignored by the source repository. Fixed protocol capabilities, pairing rules, defaults and page data are code-owned Rust types and constructors, independent of remote media or analysis inputs. Cargo has no asset downloading or pack staging build script; compilation and installation do not require `synapse.pack`.

Alias lookup keeps source priority and list order, including duplicates, audio-demo controls, dynamic SVG fragments, and GPUI fallback for unregistered keys. Declared files in pending or failed states are not silently replaced with fallback artwork. Resource completion notifies the owning views so source-derived presentation can refresh.

## Maintenance and verification

Run commands through `tools/workspace.py`:

```powershell
python tools/workspace.py verify-runtime-assets
python tools/workspace.py prepare-typed-data --check
python tools/workspace.py verify-asset-release --remote
```

`prepare-runtime-assets` regenerates readable Rust metadata from the source tables and pinned lock; `verify-runtime-assets` compares those exact outputs. Neither requires media payloads in the checkout. `--check-local` additionally verifies payload bytes when a maintainer has explicitly hydrated the originals. Source acquisition, original-image decoding, metadata closure, pure runtime tests, UI behavior and real-device acceptance are separate checks.

Maintainers can use `prepare-asset-release --repository OWNER/DATASET`, `verify-asset-release --check-local`, and `publish-asset-release` to publish an explicit resource update. Publication verifies the remote file set, every LFS length/SHA-256 and the manifest before pinning a commit; `pin-asset-release --revision COMMIT` verifies an existing upload without publishing payloads. These commands do not push or rewrite the source repository.

`fetch-asset-release --product PID` is an explicit maintenance restore of that product and its shared/transitive dependencies, not a product startup or compilation step. `--from-dir DIRECTORY` uses an existing raw snapshot offline. Modified files are preserved unless `--replace-existing` is explicit. Replacement is staged and hashed; failed replacement rolls back previous changes, and failed rollback retains original backups under `.work/asset-recovery/`. Normal application use does not invoke these commands.

## Independent container core

`razer-asset-pack` remains an OS-independent container core for deterministic format and corruption tests. The application resource crate does not depend on it. The retained `audit-asset-pack`, `prepare-asset-pack` and `verify-asset-pack` tools describe explicit local container maintenance using fully available originals and matching manifests. Their output `target/asset-pack/synapse.pack` is excluded from Git and the HF raw-file dataset and is not installed or opened by the application.

Version 1 below stores unmodified bytes with identical-content deduplication and no compression. The Rust/Python wire fixture is `crates/razer-asset-pack/testdata/store-v1.hex`; it does not contain the production image set. Container tests do not validate the current product resource runtime or complete any page's acceptance.

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
