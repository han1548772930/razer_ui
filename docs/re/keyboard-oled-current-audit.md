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

Still pending: language download conditions, live device transport, original
layout and tooltips. Home-screen cards and local preset editors are mounted;
their GIF processing and host transfer remain unavailable. Language selection is now staged outside the persisted
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

The preset editor now also mounts the source upload affordances. Animation slots
accept local GIFs and image slots accept PNG/JPG/JPEG/BMP through the host path
picker. The crop dialog keeps the source's fixed 232×64 (3.625) frame, local
zoom, reset control and Apply/Cancel footer. The original cropper has a 1–10
slider whose callbacks change its zoom ratio; the current local 1×–10× preview
is still a partial implementation, not an assertion that its geometry and zoom
steps equal CropperJS. Source crop settings are recorded independently below.
Local staging does not claim GIF frame processing, hardware transfer, progress
or success: those operations remain outside the current service boundary.

The home selector now mounts source-shaped branches for emote, banner, media,
system and keyboard-information modes. Media uses the source's black OLED
preview surface and visualizer-bar arrangement; system intentionally shows
placeholders instead of battery/temperature/date telemetry. Emote and banner
open explicit editor dialogs whose unavailable service boundary is visible.
Native enable switches remain below previews for keyboard access and the source
hover toolbar animation is still represented by the stable controls rather than
runtime hover transitions.

Five Rust regression cases cover selection fallback, the last-enabled invariant,
isolated drafts, invalid saved indices, imported-payload restoration and the
current preset schema. They are type-checked by
`cargo check --locked --all-targets`, **not executed**, under AGENTS.md.

Verification: `cargo check --locked --all-targets` passes with existing unused
import/dead-code warnings; native descriptor validation and all 605 embedded
resource entries pass static validation. No application, build, test, installer,
downloaded JavaScript or DLL was executed.
## Media editor continuation on 2026-10-04

The product 691 `Os`/`Ns` receipts now have a native counterpart in
`source_controls/oled_presets.rs`. The source keeps cloned `media.info` and
`media.visualizer` objects until Apply: media information can be disabled or
placed `top`/`bottom`, and the visualizer can be disabled or selected from the
three current assets. The source invariant is retained: disabling the
visualizer enables media information, while disabling information enables the
visualizer; enabling either preserves the other setting. Both cannot be
disabled at once. Reset restores `{info:{enabled:true,selected:"top"},
visualizer:{enabled:true,selected:0}}`. Cancel/Escape/close discard the clone.

The preview follows `Ns`: black OLED surface, 232x64 content, 14px source
text and the source order (top text, visualizer, bottom text). The mounted CSS
receipt sets the visualizer card to 232x44 with a 1px `#5d5d5d` border, 2px
`#44d62c` selected border, 5px grid gap, and a `#5d5d5d` disabled overlay.
The three current GIFs are preserved as grayscale animated WebP at their source
696x132 raster dimensions and displayed at 232x44, matching browser image
scaling rather than inventing a resize filter; frame count and durations are
validated in `prepare-oled-home-assets.py`.

The media editor is available only for wired OLED mode and an enabled home
screen, matching the source `requireSynapse`/BLE branch. Applying only changes
the local device-owned `oled.media` mirror and emits the existing local change
event; no audio metadata, telemetry, OLED packet, or host transfer is
claimed. Current source receipts: `Os` 60934--63382, `Ns` 60612--60930 in
`OLED.b7b95581.chunk.js`; visualizer CSS is in
`OLED.a636cf4a.chunk.css`.

## Import draft and language-state review on 2026-10-04

The current `be` cropper uses a 190px container, fixed 3.625 aspect ratio,
`cropBoxMovable:false`, `cropBoxResizable:false`, `viewMode:0`, `dragMode:"move"`,
and minimum crop box 232×64. Image Apply invokes the shared image cropper with
232×64 output; animation Apply sends the original bytes and container/canvas/
crop-box geometry to worker 8609. The callbacks replace only the isolated
`wt`/`Vt` list item and leave the parent OLED mirror unchanged until its Apply.
`Vt` suppresses the animation-only crop information; reset is available only
for an enabled custom item. These are vendor source facts, not claims that the
local renderer produces the same processed payload.

Two native data-loss bugs are corrected: crop Apply now retains
`local_crop.zoom`, and the OLED restore hook restores `src`, `size` and this
placement despite their absence from reducer default records. Crop, preset
editor and home previews use one fixed 232×64 viewport so the staged zoom is
still visible after Apply and restore. Restoration matches the current preset
ID at each fixed index, accepts the import branch's data-URL MIME types, keeps
enabled/selected values, and normalizes malformed selection and zoom values.
`size` remains the original local file byte count; no processed GIF size or
device transfer estimate is inferred. Reset discards these local fields and
returns to the prepared current-source resource.

Language `wi` is disabled for BLE or `oledLoading.type === "progress"`.
Selection stages the raw value and displays its complemented-byte form;
the source's unchanged predicate compares both raw and displayed values.
`oledLanguageChanged` increments when middleware sends
`MW_SET_OLED_LANGUAGE_TO_UI`. When system mode (6) is active, later language
updates replace system slide labels; they are not a local select side effect.
`Wi` mounts the language progress dialog only when the loading reducer has
`target:"language", type:"progress"`; `Ui` exposes its cancel confirmation and
dispatches `CANCEL_OLED_LANGUAGE_UPDATE`. The native UI has no middleware
loading/reply source, therefore it keeps language Apply as a local mirror and
does not fabricate download progress, cancellation or completed status.

Reproducible source and boundary receipts are in
`oled-crop-language-current-evidence.json`, generated by
`tools/audit-oled-crop-language.cjs`. GIF worker execution, complete cropper
canvas/pan/zoom geometry, original slider callbacks, language download UI state,
hardware transfer and pixel-level runtime validation remain open.

## Worker resources and crop zoom continuation on 2026-10-04

`fetch-oled-worker-resources.py` now covers the manifest-declared worker and
ImageMagick JS dependency, six 15fps animation GIFs, the `lets-go` GIF, the
14,422,294-byte `magick.b51fdc57.wasm`, and both declared source maps. All ten
required resources are available with HTTP and SHA-256 receipts. The two source
maps return HTTP 404; their response bodies and hashes are retained as absence
evidence. Each animation preset is 232x64 with 60 frames and 60/70ms delays;
`lets-go` is 232x64 with 82 frames and 70ms delays. GIF validation decodes media
only. WASM validation reads its version-1 header and section bounds without
instantiating or executing it. The reproducible acquisition report is
`oled-worker-resources-current-evidence.json`.

The acquisition and worker audits now bind every dependency to its exact
manifest key, URL, local path, HTTP receipt and response body. They require the
complete unique twelve-resource inventory, matching HTTP status/result,
captured timestamp, byte count, SHA-256 and validation metadata. The manifest
itself is checked against its successful HTTP receipt. A source-map absence
must be `not_found` with HTTP 404 and the exact `.response` body; an unrelated
body, omitted dependency or stale size is rejected. The WASM section-length
reader rejects values that overflow its unsigned 32-bit length field.

`audit-oled-worker.cjs` statically parses the current worker module 71568,
ImageMagick module 64175 and WASM URL module 29623. Its receipt connects the
mounted `be` component to worker chunk 8609 and the worker to chunk 9537 and
the exact manifest WASM URL. The processing contract coalesces frames before
limiting to 150, processes batches of 20, constructs a black canvas from the
Cropper geometry, resizes/composites/crops, converts to grayscale, and resizes
to 232x64. Each output delay is at least 6 GIF centiseconds for 15fps mode or
3 for the alternate mode. `optimizePlus()` precedes GIF encoding. A complete
reply contains a GIF data URL, encoded byte count and frame count. This is
static source evidence, not an implemented native encoder or an executed
worker. The detailed report is `oled-worker-current-evidence.json`.

The bridge receipt also records its lifecycle. Animation Apply sends the
original bytes and geometry; image Apply uses the separate shared image
cropper at 232x64. Apply sets loading, progress retains loading, and
complete/error clears it. Complete forwards the processed data URL and encoded
byte count to the isolated preset callback. Complete, a worker error message,
Cancel and unmount terminate the worker. The separate worker `onerror`
callback clears loading and logs the error without itself terminating it.
These are source behaviors; native local staging does not synthesize these
processing states or output values.

The native crop editor now has the current `xe` slider with retained positions
1 through 10 and step 1. The source positions are control state, not absolute
zoom ratios: buttons move one position and pass +/-0.1 to the zoom function;
slider movement passes `(next-11)/10` when decreasing and `next/10` when
increasing. Cropper converts a negative input to `1/(1-input)` and a positive
input to `1+input`, then multiplies the current ratio. Native callbacks now
follow these expressions and keep button changes and Reset synchronized with
the slider. Reset returns both states to 1. The slider has the source CSS's
150px width and 4px/10px margins; Reset occupies its own centered row.
The ratio formula is independently checked against the `zoom` AST node in
the current product's `3725.ac580dd5.chunk.js`, including its full-file and
function-slice hashes. Both zoom buttons have localized accessible names from
the existing `ZOOM_IN`/`ZOOM_OUT` keys.

Reading and base64-encoding an imported file now runs on the background
executor so a large animation does not synchronously occupy the UI thread.
Completion uses the existing weak editor reference, rejects results from older
file selections, and rechecks the slot's enabled state before opening the crop draft. It still stages the original
bytes and size; background file loading is not GIF processing.

The existing local preview still clamps magnification to 1..10 and uses its
existing centered viewport. This guard is not the source's `minCanvasHeight`
constraint. Full image-dependent canvas geometry, panning, GIF encoding and
hardware transport remain pending. Apply still retains original local bytes
and the local preview placement in the isolated preset draft. The previous
limitation concerning missing slider callbacks is superseded by this section;
the complete crop renderer remains partial.

Validation for this continuation: offline resource/hash/media validation,
Acorn parsing and source-contract checks, current crop/language audit, and
Rust formatting. The parent task performs the consolidated
`cargo check --locked --all-targets`. No application, tests, build, downloaded
JavaScript, WASM, installer or DLL was executed.

Reproduction commands for the focused static checks:

```powershell
.work/resource-env/Scripts/python.exe tools/fetch-oled-worker-resources.py --offline
node tools/audit-oled-worker.cjs
node tools/audit-oled-crop-language.cjs
rustfmt --edition 2024 --check src/features/source_controls/oled_presets.rs
```

## Canvas geometry and local media continuation on 2026-10-04

The current `be` component and OLED stylesheet fix the actual Cropper
container to **232x190**, not the previous native 580px width. With
`viewMode:0`, `minCropBoxWidth:232`, aspect ratio 3.625 and container width
232, the fixed crop box is exactly `{left:0,top:63,width:232,height:64}`.
Eleven current Cropper AST functions, their byte-for-byte source slices,
the mounted options and relevant CSS are recorded in
`oled-canvas-current-evidence.json` by `tools/audit-oled-canvas.cjs`.

The native canvas now implements that initialization and geometry. It first
contains the decoded image within 232x190, then applies `minCanvasHeight:64`
while preserving the image aspect, and centres the result. Consequently a
100x1000 portrait begins at 19x190 with horizontal black space in the OLED
output, while a 1000x100 image begins at 640x64 with both sides extending
outside the crop. The crop editor shows the full image canvas with the source
grayscale treatment, transparent container background, 50% black top/bottom
masks, 10% white crop-face overlay, dashed outline and corner marks.

Dragging moves the canvas, keeping the crop box fixed. Position follows the
source's view-mode-0 bounds: left `[-canvasWidth,232]`, top
`[-canvasHeight,190]`; blank crop regions are valid. Window capture listeners
continue dragging outside the image area, and release inside/outside clears
the drag, including release before a new frame installs those listeners.
Pointer deltas are converted back from current UI scale to source CSS units.
The focused crop group also supports arrow keys for 1px movement and
Shift+arrow for 10px movement. Slider/button zoom remains relative to the
current canvas centre. If it reaches the minimum size, source
`renderCanvas` preserves the old position before applying height 64. There
is no 10x cap on the active geometry. Reset restores the initial rectangle,
clears dragging and returns the slider to position 1.

The background import now reuses GPUI's existing decoder rather than inferring
image aspect from the OLED output dimensions. This keeps GIF frames and uses
the decoder's EXIF-adjusted dimensions for images. The crop dialog directly
uses the resulting `RenderImage`. Another native defect was corrected:
passing a `data:image/...` string directly to `img` takes GPUI's URI/HTTP
loading route. Saved local data URLs now use a native `Asset` cache that
decodes base64 and image bytes on GPUI's background executor. The source
inspection records the pinned GPUI 0.3.7 decoder and cache hashes. No new
dependency was added, and local image data is not submitted to an HTTP client.

Apply stores the exact canvas width, height, left and top in
`local_crop.canvas`. Both preset and home previews render that same image
rectangle relative to crop-top 63; restore preserves the rectangle and clamps
invalid saved positions. Previously saved zoom-only drafts retain their
earlier placement. The original file data URL and byte count remain intact.
The isolated editor draft, Cancel/Escape behavior, enabled-slot check and
latest-file-selection generation check remain in force.

This section supersedes the earlier statements that canvas geometry, panning
and active zoom limits are still represented only by a centred 1..10 preview.
Native processed image/GIF encoding, ImageMagick equivalence, processed byte
counts and device transport remain separate unfinished work. No runtime
pixel or interaction parity is claimed: verification is static source
inspection, formatting and the parent task's permitted consolidated
`cargo check --locked --all-targets`.

Focused reproduction also includes:

```powershell
node tools/audit-oled-canvas.cjs --check
rustfmt --edition 2024 --check src/features/source_controls/oled_presets.rs src/features/source_controls/oled_crop.rs
```

The subsequent static self-review found and corrected two lifecycle defects.
GPUI retains animated frame state only when the image has an `ElementId`;
both the crop image and preset/home images now have stable identities. Slot
and content identity keep redraws on the same animation while replacement
starts a new animation. The existing media visualizer images received the
same identity correction. GPUI selects frames using their original delays,
pauses when the window is inactive and respects reduced-motion settings. Its
decoder can skip individually damaged GIF frames and returns an error if
none decode; this preview behavior does not imply the worker's coalescing,
150-frame output limit or optimized encoding.

GPUI's asset cache also retains pending and completed results until explicit
removal. Each displayed local image now owns a keyed lease. Concurrent home
and editor previews share one decoded data-URL asset; replacement or removal
releases the lease, and the final release removes the asset. The GPUI cached
load keeps completion state weakly, so a removed pending entry cannot later
publish into a replacement. The original saved bytes remain in the preset
draft independently of whether a preview is currently decoded.

Local resource safeguards now run before decoding files and restored data
URLs. Encoded input is limited to 64MiB, including a bounded file read that
also handles a file growing after its metadata was read. PNG IHDR, JPEG SOF
and BMP DIB dimensions are checked; JPEG rejects conflicting frame dimensions,
and BMP header bounds are verified. GIF scanning checks the logical canvas,
each frame rectangle, palette and sub-block bounds and all frame records
without running LZW. The estimated resident RGBA frames plus per-frame
records must fit 256MiB. This is **not a hard process-memory limit**: codec
working buffers, compressed bytes and metadata are outside the estimate, and
compressed-data validity still belongs to the existing decoder. These are
native safeguards, not source UI limits.

Both load paths additionally reject decoded results with zero dimensions,
inconsistent frame canvas sizes or excessive resident frame bytes before
handing them to GPUI layout. This post-decode check does not bound transient
memory already used inside the codec.

Invalid imports preserve the previous draft; malformed restored data URLs
produce a cached image error and remain replaceable or resettable. Invalid
saved rectangles fall back to the older zoom placement. A 2^40 CSS-pixel
canvas-edge guard rejects extreme finite floats before layout arithmetic;
it exceeds the largest initial canvas possible from u32 image dimensions
with minimum height 64, so it does not limit a valid default Cropper canvas.
The self-review is static and does not claim exhaustive codec safety or
executed interaction coverage.

## Preset/card presentation continuation on 2026-10-05

The current `wt/Vt/jt/Ht/pi/Q/G` audit now restores the editor titles and
descriptions, three-column preset grid, card-local hover controls, exact
source disabled/selected border precedence, 1.1 preview hover scale and
original Replace/Reset SVGs. Home titles track body hover, and source
requires-Synapse/BLE explanations are mounted. The existing native import,
isolated crop/editor drafts and Apply boundaries remain intact. Full evidence,
static reproduction commands and remaining tooltip/service limitations are
in [oled-preset-cards-current-audit.md](oled-preset-cards-current-audit.md).
