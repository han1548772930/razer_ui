# Resource delivery and optional container format

## Application resource delivery

`SynapseAssets` resolves aliases against typed snapshots decoded from the local SQLite catalog and obtains individual media files as any screen, dialog or window actually uses them (including fixed settings, help and macro pages) from the private Hugging Face Dataset `han1548772930/razer-ui-assets`. The dataset preserves product/shared paths and original file bytes through its own Git LFS rules. It contains no required ZIP or runtime pack. `tools/asset-release.json` pins an immutable dataset commit and each file's length/SHA-256; `assets/synapse/resource-catalog.sqlite3` carries the generated resource index and complete emoji profiles. The dataset's own `release-index.json` omits its storage commit to avoid a self-referential hash.

The approximately 45.5 MiB catalog (47,710,208 bytes, schema 4) is a standalone installed file beside the executable; it is never embedded with `include_bytes!`. Only the SQLite engine is statically supplied by `rusqlite`'s bundled feature. `crates/razer-resource-db/installation.json` owns the source path, installed filename and cache location for Cargo, runtime and installer. Before opening the UI, a valid existing cache is reused; otherwise the external installed catalog is verified and atomically published to the current executable's `assets/cache/catalogs/<content-sha256>.sqlite3` and opens that file readonly. A missing or corrupt copy is repaired from that external installed file without networking; absence of both a valid cache and matching installed file is an explicit initialization failure. Content versions have separate names; upgrades keep usable previous catalogs and media. The catalog's identity, schema version, integrity and references are checked before typed decoding. Publication, schema or decoding failures stop initialization with a real error. No installation, analysis or developer path is compiled as a runtime lookup location.

The database normalizes shared emoji entries, category value lists, variants and exclusions. It retains the complete 1,936-entry search order over 1,857 distinct records, including source duplicates, empty glyphs and ZWJ sequences. Mapping, mapping-without-exclusions and Macro retain their different labels, group ordering, tab targets and Windows filters. The index retains all alias/list entries and first-match priority, SVG viewports, fonts and file identities. No image/video/font BLOBs or user settings are stored in this immutable catalog. User documents keep their existing migration, conflict and save semantics in `razer-storage`; device protocol tables and defaults remain typed Rust data. Validated catalog snapshots are retained by their domain owners; rendering never performs SQL or file I/O.

The normalized `ui_locale` and `ui_translation` tables hold 10 locales and 58,587 translations. `razer-resource-db::localization::snapshot()` retains an immutable `DictionarySnapshot`; `razer-i18n` resolves translations from that snapshot and shares the current-language owner with GPUI. The former production `locales/*.json` files and embedded dictionary reads are removed. Migrating their bytes preserves the existing vocabulary; it does not establish complete vendor provenance for the legacy entries.

`razer-resources` owns the typed media catalog, cache, credentials, transport and `ResourceRuntime` without a GPUI dependency. The application creates one runtime and injects its client into `razer-assets::SynapseAssets`; the GPUI adapter owns window decode caches, resource scopes and completion notifications. Mutable resource sessions are application scoped. The runtime owns a named I/O thread and its Tokio driver, and closes the queue, cancels in-flight requests and joins that thread during shutdown. A retained client cannot keep a stopped runtime alive or reopen it.

Delayed overlays retain the same window image-cache owner through `deferred_resource`. Its `resource_scope` preserves the child's existing layout node and bounds. Custom overlay elements use `ResourceLayer` before layout and retain it through layout, prepaint and paint. Mouse, keyboard and audio mapping callers share these boundaries; requests still use the single runtime and file cache. Resource-scope checks and pure tests do not establish real-window image acceptance or complete every product page.

Requests follow actual file use in every mounted interface, including shared controls, settings, macros, help, dialogs, tray popups and product views. The background loader coalesces requests over an 8 ms window into batches of at most 64 files, deduplicates shared paths and downloads up to four files concurrently through one pooled HTTP client. Startup does not fetch unopened pages or other products, restore the source checkout's `assets/`, or download an entire archive. HTTP/TLS initialization, credential lookup, cache reads, hashing and atomic publication all run on the resource I/O thread without holding the in-memory state lock. Image availability checks only consult in-memory state and enqueue work; large ready payloads are not copied by these checks. The UI receives completion notifications, and decoding starts once verified bytes are available. Tutorial clips use the same pending-aware cache and release decoded frames when their subtree unmounts.

Cached files are under the current executable's `assets/cache/`, named with their content hash to keep valid versions distinct. A cache hit needs no HTTP client, networking or credential. An uncached file remains unavailable offline; failures stay visible and can be retried without implying device state changes. The application owns and cancels the loader and its I/O driver; a cancelled download cannot publish late bytes.

Private access credentials are read from explicit `HF_TOKEN`, the local Windows credential store, or the local `hf auth login` token file. They do not belong in the source, manifest, installation package or logs. The installation directory must be writable by the ordinary user. Upgrades preserve caches; uninstall offers retention or cleanup limited to this application's cache. See [Windows installation](windows-installation.md).

Credential lookup stops at the first nonempty value: `HF_TOKEN`, then the Windows generic credential named `git:https://huggingface.co`, then `HF_HOME/token` (or `%USERPROFILE%/.cache/huggingface/token` when `HF_HOME` is unset on Windows). Updating only the token file does not override a credential found earlier. HTTP 401 with a supplied credential means the resource request failed authentication; HTTP 403 means the server denied access. These responses do not establish that a media file is missing, and a final response after a redirect does not identify which server rejected the request. Correct the selected local credential or its repository read access before retrying. Existing verified cache entries remain usable without credentials.

Resource diagnostics retain the failed path and current error. SVG availability errors are logged when the error changes, rather than once per paint; pending resources do not produce paint errors. A successful completion clears the retained availability error and refreshes the windows. Pure transport and cache tests verify these state transitions separately from actual download and window acceptance.

The source checkout retains the standalone database, nine mandatory native RGBA icons and four fixed window-control SVGs. Image, video and font payloads are ignored by the source repository. Fixed protocol capabilities, pairing rules, defaults are code-owned Rust types and ordinary constructors, independent of remote media or analysis inputs. Presentation catalogs, image selectors and geometry metadata also reside in SQLite and decode through ordinary typed functions. `tools/resource-mappings.json` is the single maintenance mapping input; source-constructor projections are kept separately under analysis inputs and never read by Cargo or runtime. Superseded emoji literals, resource-table fragments, data macros and the generated Rust resource index are removed after full roundtrip verification. Cargo has no asset downloading or pack staging build script; compilation and installation do not require `synapse.pack`.

Alias lookup keeps source priority and list order, including duplicates, audio-demo controls, dynamic SVG fragments, and GPUI fallback for unregistered keys. Declared files in pending or failed states are not silently replaced with fallback artwork. Resource completion notifies the owning views so source-derived presentation can refresh.

## Maintenance and verification

Run commands through `tools/workspace.py`:

```powershell
python tools/workspace.py verify-runtime-assets
python tools/workspace.py prepare-resource-database --check
python tools/workspace.py prepare-typed-data --check
python tools/workspace.py verify-asset-release --remote
```

`prepare-resource-database` generates the single SQLite seed from the audited emoji maintenance inputs, UI translation maintenance input, complete typed maintenance projections, `tools/resource-mappings.json` and the pinned resource lock. It compares every decoded field, locale, translation, product profile, duplicate and list order against those inputs. `prepare-runtime-assets` and `prepare-emoji-search` use the same generator; their verification modes check the same database rather than maintaining parallel production tables. The generation receipt records Python SQLite's actual version, schema/input byte hashes and output identity. None require media payloads in the checkout. `--check-local` additionally verifies payload bytes when a maintainer has explicitly hydrated the originals. Source acquisition, original-image decoding, metadata closure, pure runtime tests, UI behavior and real-device acceptance are separate checks.

`prepare-locale-exports --acquisition RECEIPT --source JS [--check]` statically decodes complete current dictionary exports and their locale routing after validating the acquisition and source bytes. The canonical maintenance dictionary and original legacy baseline remain under `.analysis/inputs/typed-tables/razer-i18n/`; neither is a runtime input. The current merge adds six missing keys per locale, retains all legacy values and records 94 differing existing values in [locale-preparation.json](../.analysis/results/locale-preparation.json). `prepare-locale-keys` updates the same canonical dictionary and SQLite generator rather than recreating the removed production JSON directory. Export extraction, dictionary equivalence and unresolved legacy provenance remain separate evidence boundaries.

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
