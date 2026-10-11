# Current keyboard artwork and conditional source audit

Current source was read directly from `local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/`. Earlier audit documents and obsolete snapshots were not implementation evidence. This closes the independently traced base artwork and key geometry branch for this product; it does **not** mean other products or the whole Customize page have been finished.

The current entry is `static/js/main.194d8a7b.js`, SHA-256 `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`. Full original source slices, UTF-16 boundaries, byte offsets where available, file hashes and slice hashes are preserved in [keyboard-679-source-artwork-current.json](keyboard-679-source-artwork-current.json). Source product identity is independently captured from module 78193 `DeviceInfo.productId`; the Rust selector matches that source metadata rather than a handcrafted product-ID conditional.

## Why the previous image and geometry were wrong

The native page used one static product image and one static key group. Current `Sh` Customize receives genuine `deviceReducer.editionId`, `layoutId` and `wristrestConnected`, passes edition/layout to ProductImage, and calls `updatedProductWithLayoutAndEdition(layoutId)` for the physical key group. A product route or feature flag cannot represent those observations.

The current `BM` ProductImage initializer is captured at UTF-16 `[7156496,7159692)`; its dynamic context is module 58388 `[5209404,5212491)`. It independently loads `./679_${edition}/img_prods/${layout}-1x.png` and `-3x.png`. Each failed request retries edition zero with the **same layout**. It does not replace layout zero or an unavailable layout with layout one. Its DOM uses `src=1x` and `srcset="1x 1x, 3x 3x"`, leaving density selection to the browser. There is no vendor `devicePixelRatio > 1` conditional.

`keyboard_source_artwork.rs` exposes a default 1x selector and a selector accepting the native display scale. This native adaptation chooses 3x for scale above one and 1x otherwise; this adaptation is recorded separately from the vendor srcset contract. It never silently substitutes another density if that requested candidate is missing. Both image resolutions retain original pixel dimensions; source CSS bounds are 730 × 340.

Module 21368 owns the US fallback key literals. Module 99095 owns the alternative groups and resolver; module 13254 owns layout enumeration. Fourteen explicitly recognized layout IDs are decoded with their own shapes. Unknown layout IDs use the original resolver's US fallback and are marked `used_source_default_group`; this geometry fallback does not invent a missing base image. The source SVG default viewbox is 730 × 387. Every original SVG path, circle and rectangle is decoded into native `KeyboardKey` geometry without substituting rectangular hit boxes for path outlines.

## Wrist observation and CSS cascade

Current source event `MW_ATTACHED_THUMB_MODULE` copies its payload into `deviceReducer.wristrestConnected`. Missing observation remains absent; there is no fabricated connected or disconnected read. On mount, the source local `wrist` state takes the observed value. Initially disconnected means no wrist element; after connection, the wrist element remains mounted and changes `show`/`hide` when disconnected. The caller must reset artwork state when the actual device/workspace is replaced, including replacement by another physical device with the same product ID.

The exact current `.wrist-rest` declaration is captured at CSS UTF-16 `[391466,391828)`. Multiple declarations cascade: the last background is `wrist-without-gap.8e806975.avif`, width 731, height 154, top 255, translateY 5. It is a plain CSS `div`, so its image is fixed by CSS rather than edition-bound. In a centered 730-pixel frame its left edge is -0.5. Current `.padding-for-wrist-rest-device` adds 25 pixels. The Rust artwork state and selector preserve these observations and bounds; parent rendering owns actual mounting.

## Dial and multimedia popovers remain an implementation gap

`Sh` render explicitly selects edition 128 artwork only when `editionId === 128`; all other editions use the standard dial and media image. Original modules are dial 835/50721 and button 72970/19828. These are **popover artwork**, not extra base-image overlays. Selectors and assets are prepared, but popover rendering, positioning, projected mapping labels, hover tooltips and state transitions are not finished in this change.

The source owning classes `RM`, `DM`, child `SM`, aliases `NM`/`CM`/`uM`, images and all nine SVG prop imports are preserved in the receipt. RM displays truthy `button.display` entries; DM selects entries present in `mediaButtonImg`. SM consumes the actual projected `buttonList` assignment value and group; static `default_buttons` must not be mistaken for the real mapped state. Source Sh blocks clicks for MEDIA_PREV/MEDIA_PAUSE_PLAY/MEDIA_NEXT and blocks special remapping in macro mode. Its dirty-edit selection routes through `displaySaveAlert`, preserving cancellation and confirmation.

RM hover anchor is source `mediaVolume`, shape `DIAL_CLICK`, center (599,91.3), radius 17.5; DM anchor is `pauseandplay`, shape `DKM_KBMK_02`, center (565.3,91.5), radius 8.4. Their `isEnabled:false` must not accidentally remove the source hover trigger. Sh positions popovers using anchor client bounds, drawer width, main/body heights, and scroll offsets; macro positions are (694.5,106) and (654,110). Resize and scrolling reposition an existing popup. The full positioning and hide guards are retained in the source receipt, not summarized as a guessed fixed layout.

The source has no loose generic row for all keys lacking main SVG geometry. Those dial/media child inputs belong to their special popovers. Their absence from physical key shapes does not authorize rendering invented buttons elsewhere.

## Scope-wide findings and remaining gaps

An earlier independent static comparison checked 961 literal DeviceInfo fields across 71 native keyboard products against each current main bundle and found zero literal mismatches. Eleven OBMSpecs branches still contain unresolved member references, so that comparison is not full capability verification. Affected broader gaps include generic mapping editors, ordinary Quick/Advanced lighting state, projected assignment labels and source-specific special input branches; product descriptors alone do not close any of them.

Current 678 and 679 callers both use the 25-pixel wrist padding; 688 explicitly uses 2-percent padding. Their source `.toggle-controller` div does not directly clear key selection: source floating-controller lifecycle owns its behavior. These findings were handed to the keyboard renderer owner for source-backed changes. The full-program scope remains all products and all original UI/state/service/device chains.

## Preparation and static validation

Maintained tools are `tools/audit-keyboard-679-artwork-current.cjs`, `tools/prepare-keyboard-679-artwork-current.py` and `tools/validate-keyboard-679-artwork-current.py`. Audit statically parses current JS with Acorn; it never evaluates vendor code. The preparation output contains 39 image requests resolved into 33 unique lossless PNG assets, nine unmodified SVG resources and fourteen layout tables. The durable asset manifest retains original/output SHA-256, source path, conversion and original dimensions; the embedded table contains all 42 assets.

Static validation passed: 140 original JS/CSS/module slices match exact source bytes and hashes, all 42 assets match provenance hashes, each PNG's RGBA pixels exactly match the decoded original at unchanged size, each SVG is byte-identical, and all fourteen prepared layout tables exactly match deterministic decoding of captured source groups. Rust formatting completed. This agent did not execute application/vendor JS/DLLs, run real device operations, or run cargo; the parent consolidates permitted Rust checks/tests. Runtime visual acceptance remains unperformed.

## Parent verification, 2026-10-11

Current source receipts: 5393 exact slices across 286 current source files validated. Resource registration independently checked 30 shared floating-controller originals across three products and 42 artwork source/display assets. `cargo check --locked --all-targets --offline` passed with warnings. Pure Rust tests passed: floating/controller-drop 7, mouse button-state 5, source artwork 3 (15 distinct tests). No application, vendor JavaScript, DLL, helper, installer or real device operation was executed; visual/runtime and real persistence acceptance remain absent.
