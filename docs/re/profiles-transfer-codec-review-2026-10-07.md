# Profiles transfer codec independent review — 2026-10-07

Reviewed `src/shell/profiles_page/transfer_codec.rs` against current Profiles application receipts in `profiles-transfer-current-evidence.json`: 43/Ea and Da, 1867/D, stable-stringify module 5371, MD5 module 2937, and base64 module 7207. These are current `.ref/applications/synapse/profiles/` files, not the obsolete historical frontend. Review used static source reading only.

Confirmed:

- 1867/D clones the decoded JSON object, removes top-level `hash` and `gamemode`, applies stable JSON serialization and MD5, and 43/Ea compares the outer hash exactly. Native code preserves that order, validates the hash before transformation, and then applies the outer profile name.
- Stable stringify sorts keys lexically by JavaScript UTF-16 code units and manually emits object fields. Native sorting matches that; it does not accidentally use UTF-8 order or numeric-index enumeration. Arrays preserve order, and strings use JSON escaping.
- Source category/PID/dongle/BLE compatibility is tested before profile import. The differing-product warning follows the source condition. Outer names are deliberately required to be strings for native editable rows. Incompatible or malformed rows never bypass hash verification.

Corrected during this review:

1. `dynamicKeyStrokeGroup` filtering now uses JavaScript truthiness. `0`, `-0` and `""` no longer incorrectly cause a mapping to be removed; empty arrays/objects remain truthy. `is8kAnalogDevice` uses the same semantics.
2. 7207/R first normalizes URL-safe `-`/`_`, and y removes all non-base64 characters, including `=`. The native decoder follows that normalization instead of rejecting source-accepted padding/noise.
3. JavaScript has one numeric type: document PID `2636.0` equals target PID `2636`, whereas a numeric string does not. Native compatibility and warning checks now use finite numeric equality rather than serde's integer/float representation equality.
4. Outer file decoding now matches the FileReader UTF-8 text step, including consuming one BOM and replacement characters for malformed byte sequences. Inner decoded payloads remain subject to strict JSON/UTF-8 parsing and their hash check.

Three regression tests were added for truthiness and numeric identity, normalized base64, and stable UTF-16 ordering/JavaScript notation thresholds. They are **compile-only**; none was executed. Source code, applications and DLLs were not executed. Parent owns `cargo check --locked --all-targets` and regeneration of the transfer evidence's native hashes.

Numeric presentation handles ordinary finite values, zero, the fixed-decimal interval `[1e-6, 1e21)`, and signed exponent notation. Arbitrary float edge cases have not been validated against a JavaScript runtime; this report does not claim full ECMAScript number serialization equivalence. The native parser also rejects unsupported lone-surrogate strings, non-finite number encodings and excessive nesting. These produce an import warning/rejection rather than an accepted unhashed profile. Full vendor conversion/export and actual device application remain separate work.
