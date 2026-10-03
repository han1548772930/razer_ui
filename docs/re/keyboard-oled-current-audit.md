# Current OLED controls and device ownership

Product 691's mounted `OLED.b7b95581.chunk.js` and current `main.e51ce83a.js`
are the implementation evidence. `tools/extract-keyboard-oled.cjs` verifies
source hashes, component slices and reducer defaults without executing vendor
code. Exact source receipts are in `keyboard-oled-current-evidence.json`.

The partial native page now includes brightness (20–100, step 1, initial 50),
language (English / Simplified Chinese, initial English, explicit Apply),
screensaver (none or three original animated previews, initial none),
return-to-home time (5/10/15/20 seconds, initial 5), and dim time (1/2/3/4
minutes, initial 1). Dim time is disabled when its source `enabled` state is
false; the source widget does not add an enable switch. Labels and descriptions
use the existing official localization keys. These are local settings mirrors,
not hardware writes.

OLED reducers are independent from `DEFAULTPROFILE`. The local device model
therefore stores these values in `source_device_settings.oled`. This local
storage envelope is not an invented vendor profile field. Profile switching
restores the same device values. The profile selector remains visible, as in
the enclosing keyboard workspace, but does not own OLED state.

The same boundary fixes product 3595's previously profile-scoped `deviceMic`:
legacy values migrate out of both direct profile settings and `_supplement`.
Existing device values win; otherwise the active profile wins conflicting
legacy values, and inactive profiles only supply missing fields. Old copies are
removed from every profile. Dirty comparison includes device settings, and the
existing whole-device save queue serializes them alongside profiles. Discard
restores the saved device mirror as well as profiles.

Screensaver cards use the current OLED CSS's 2×2 arrangement, 260×68 outer
size, 10-pixel spacing, black surface and selected/hover borders. Original
256×64 GIFs (270/300/300 frames) are declared in the current product asset
manifest and imported by the mounted OLED component. `prepare-oled-assets.py`
downloads only these declared assets, records hashes and converts them to
lossless animated WebP with frame pixel and duration validation. The native
selector retains the reducer's IDs 0–3; eventual device transport must encode
ID 0 as disabled and IDs 1–3 as enabled with values 0–2, matching the reducer.

Still pending: home-screen cards and editors, language download conditions,
live device transport, original
layout and tooltips. Language selection is now staged outside the persisted
snapshot until Apply, with the source complemented-byte decoding (`raw < 127 ?
raw : 255 & ~raw`) and unchanged-value disable behavior. BLE disables both the
select and Apply. Leaving the page drops unapplied selection. The source
download-in-progress disable branch and system-slide label updates still need
the corresponding transport and editor state; neither is simulated here.
This page is partial, not a full reproduction. Application
execution and runtime interaction tests are prohibited by the workspace source
requirements; validation is limited to static checks and `cargo check`.

The mounted root `na` verifies the two source columns: brightness/language on
the left and home delay/dim delay/screensaver on the right. Native descriptors
now retain this grouping and source order. Narrow windows wrap the columns;
the precise shared-source column breakpoint and width still require auditing.
The home-screen region belongs above both columns, not inside either.
Column presence does not count as completion of its previews and editors.

`extract-oled-editor-data.cjs` now statically parses the current webpack modules
22502, 37769, 9483 and 79826. It extracts six animation presets, eight embedded
PNG and two manifest-declared AVIF image presets, 64 banner font choices, 14 font sizes, and the source home
and media defaults into `keyboard_oled_editor_data.json`. Animation branch
selection uses this product's `OLED_ANIMATION_FPS` configuration; each imported
animation must also be declared in its current asset manifest. Module ranges
and hashes accompany the data. These editor descriptors are not yet mounted
UI and do not increase the native coverage count.

The source home mode order is animation (0), image (1), emote (4), banner (2),
media (5), system (6), keyboard (3). Home defaults to enabled, animation selected.
Media/system cards are disabled over BLE. BLE also blocks editing animation,
image, emote and banner, but does not prohibit applying their existing content.
The keyboard card is not editable. Media defaults enable both information and
visualizer. System reducer sample telemetry is preview data, not live readings.

## Continuation on 2026-10-03

The native home region now mounts its enable switch and seven mode choices
above the two settings columns. Both paths are inside the device-owned `oled`
envelope. Disabling home prevents mode edits without resetting the selection.
Only media (5) and system (6) choices are disabled over BLE; the same predicate
guards mutations as well as button presentation. Existing saved settings merge
into source defaults, retaining enabled/animation for previously absent fields.
These choices are still a button group; source preview cards and editors are
not yet mounted and must not be counted as complete.

`prepare-oled-home-assets.py` prepares all six animation and ten image presets
at their original 232×64 dimensions, separately from the 256×64 screensavers.
The preparer checks the current module hash and manifest declarations, records
source/output hashes, composites transparent pixels on the source black preview
surface, and applies the CSS grayscale luminance coefficients. PNG pixels and
decoded WebP frame pixels/durations are checked after encoding. All 16 assets
are included by the resource preparation manifest for subsequent preview/editor
work. This does not imply that selecting a mode sends content to hardware.

The preceding image extractor incorrectly asserted that all ten images were
inline PNGs despite presets 7 and 9 being AVIF imports. It now requires each
external image to be declared in the current manifest, and regeneration passes.
The current stylesheet confirms 236-pixel card wrappers and 232×64 preview
content, with a 10-pixel wrapping gap; implementing the complete seven-card home region remains pending.

## Preset editor continuation on 2026-10-03

`source_controls/oled_presets.rs` now mounts animation and image preview/editor
entries. The six animation and ten image presets use the prepared current-source
assets. Each dialog owns a cloned selection: Apply writes `oled.animation` or
`oled.image`; Cancel, Escape and close discard the clone. The last enabled
preset cannot be disabled, and disabling the selected preset selects the first
remaining enabled item. These paths remain device-owned, outside profile data.
BLE and disabled home state block editing and applying an open editor.

The source reducer's preset IDs, enabled/custom flags and selected index are
now extracted independently from its asset arrays. Mounted wt/Vt/jt/Ht/pi/ve
function receipts accompany those defaults; validation checks their source
slices and all sixteen embedded resource paths.

This is **preset selection only**, not the complete OLED editor. Upload, crop,
custom-content reset, transfer/progress/error handling, emote/banner/media/system
editors and the complete home card layout remain pending. Native enable switches
are visible below previews for keyboard access; the source hover toolbar and
its animation have not been reproduced. The original mode button group remains.
No hardware success or progress is simulated.

Three Rust regression cases cover selection fallback, the last-enabled invariant,
isolated drafts and invalid saved indices. They are type-checked by
`cargo check --locked --all-targets`, **not executed**, under AGENTS.md.

Verification: `cargo check --locked --all-targets` passes with existing unused
import/dead-code warnings; native descriptor validation and all 605 embedded
resource entries pass static validation. No application, build, test, installer,
downloaded JavaScript or DLL was executed.
