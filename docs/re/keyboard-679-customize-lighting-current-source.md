# PID 679 Huntsman V3 Pro Tenkeyless — Customize and Lighting

Audit date: 2026-10-10. Current source is the mounted PID 679 bundle, not a historical snapshot.

* JavaScript: `local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js`, SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`.
* CSS: `.../static/css/main.1c5a651a.css`, SHA-256 `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`.

The current Customize caller is `Sh` (UTF-8 byte offset 8,009,553): `uh` mounts `EP` (gaming mode) and `AP` (analog Windows properties) on the left, and `ZM` (quick remapping) plus `Oh` (gamepad tester) on the right. `ZM` is at byte offset 7,978,015 and `Oh` at 8,000,078. The shared control row uses `toggle-controller` and `toggle-drawer`; CSS gives these 38×27 px dimensions (CSS offsets 399,737 and 402,812). The Hypershift wrapper is 36 px high and uses `#44d62c` / `#fd8611` (`hyper-wrapper` CSS offsets 403,905–404,629). Keyboard key hover is transparent fill with a green 2 px stroke; Hypershift changes the stroke to orange (`svg-key` offsets 209,498–209,867).

The current Lighting caller is `$L`/`em` (byte offset 7,888,670). Its effects card mounts `VL`/`wL`, which uses the two-way `QUICK_EFFECTS`/`ADVANCED_EFFECTS` tabs and the `jp` quick-effects renderer. `xp` renders a 27 px dropdown (`chroma-flex-row` CSS offset 181,535; `s3-dropdown` offsets 33,891–34,628), a Chroma sync action, then effect-specific controls. It does not mount a product keyboard image in this page. PID 679's source QUICK_EFFECTS IDs are 11, 12, 2, 8, 17, 5, 6, 3, 7, 1, 19, 4 and 13.

The native implementation is `crates/razer-pages/src/features/keyboard_huntsman679.rs`. It keeps gamepad values observation-only, emits local profile mutation events for quick remapping/effect selection, uses the analogV2 WM curve `[0.1, 1.4, 2.8, 4.0]`, refuses conflicting quick remaps like the source, and does not claim device acknowledgement. `keyboard_products.rs` gates the old inline mapping editor, generic quick-effect buttons, generic lighting image, and generic SnapTap column away from PID 679. Current source media copied for this page is recorded by SHA-256 in `assets/synapse/keyboard-679-*.svg`.

Remaining acceptance gap: the real host transport for `ZM`, `Oh`, and `VL` is not invoked; Rust callbacks consume only externally supplied observations and local drafts until the corresponding service adapters are completed.
