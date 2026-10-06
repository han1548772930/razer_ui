# 1383 Animation, Image and Emote editors

The current 1383 manifest selects `6141.5d00192e.chunk.js` for module 51278.
Its `Mv` mounts `au` (Animation), `Su` (Image) and `Vu` (Emote) inside the
800px CustomizeModal. These three branches use the same current dialog shell
as Media. No earlier product bundle is used as implementation evidence.

`tools/extract-audio-oled-artwork.cjs` statically resolves the 15 FPS branch in
module 22502, the image array in 37769, all lazy emote loaders in 8816 and their
104 manifest-declared webpack asset modules. `audio-oled-artwork-current-evidence.json`
records full source file hashes, UTF-16 AST intervals, original expressions,
CSS, defaults and literal SVGs. Original JavaScript is never executed.

## Implemented behavior

- Animation has six fixed slots; Image has ten. Both start with selectedIdx 0,
  custom false and enabled true in the source reducer. A tile selects only
  while enabled. Disabling the selected slot chooses the first enabled slot;
  the last enabled switch cannot be disabled. The source has no add/delete-slot
  command and no global reset for these editors.
- The source `custom && enabled` tile reset restores the exact indexed source
  preset. Source reset sizes are 60 for Animation and 1 for Image. The native
  draft uses zero for absent size, preserving JavaScript's falsey filter and
  the workspace's typed snapshot merge. The source estimate is displayed only
  for a real source reset size: ceil(size sum × 1.4), then rounded minutes.
- Emote has all 104 source entries in source order. Search follows JavaScript
  whitespace removal and lowercase matching. Selecting an entry stages its
  prepared current asset; Apply changes the owner's emote. The current default
  is Face Tongue Animated. `isDataMatch:false` retains the unavailable-preview
  branch and disables Apply until an emote is chosen.
- Apply writes only the chosen `oledHome.animation/image/emote` draft and emits
  AudioProductChanged. Cancel and closing the modal discard its isolated draft.
  The owner's selected Home Screen mode is not silently changed by editing.

## Presentation and crop

Animation/Image use the source three-column grid, 20px gaps and 30px top margin.
The content-box tile is 232×64, with 1px border plus 1px margin when idle and
2px border without margin when selected/hovered. Enabled tiles retain a 236×68
footprint. The later disabled rule overrides the hovered border back to 1px,
so its visible frame becomes 234×66 without the idle margin.
Hover magnification is immediate scale(1.1), with the source black 50% overlay,
2px action insets, 32×18 switch and 28×27 replace/reset buttons. Disabled images
use opacity 0.1 and the source disabled border. The action images are literal
SVGs from the same module.

Emote uses five columns, 15px row gaps, 132×65 cells, a 132×46 frame, a 116×42
contained image, 5px label gap and 12px labels. Its search wrapper is 290px with
3px padding and 20px source icon. The Home Screen preview keeps the source
232×64 area, #101010 background and -11px image position.

The real file picker accepts GIF for Animation and PNG/JPEG/BMP for Image.
Files are decoded off the UI thread, checked for supported headers and bounded
local allocation, and displayed through a retained ImageSource::Custom cache.
Data URLs never go through GPUI's asset-path/HTTP string loader. A generation
token and the editor's live state reject late results after another import or
close. These are native lifecycle safeguards, not invented source restrictions.

`rh` fixes the crop viewport at 232×190 and the centered crop at 232×64,
aspect 3.625, viewMode 0, minCanvasHeight 64, no wheel/touch zoom and no crop-box
resize/movement. `nh` has a 1..10 integer slider: plus/minus call zoom(±0.1),
while a slider increase calls zoom(next/10) and a decrease zoom((next−11)/10).
Reset restores the original canvas and slider position 1. Dragging continues
outside the viewport through the native window event path. These props were
independently checked before reusing the existing local CropCanvas arithmetic,
decoder and cache; that infrastructure is not a substitute for 1383 evidence.

There is an upstream label condition: `rh` passes `CROPPER_CROP_BTN`, but `oh`
uses the Crop locale only when that prop equals the literal `Crop`. Thus the
actual crop footer still says APPLY, which the native branch preserves.

The native crop stores the original data URL and `local_crop` geometry for
faithful local previews and profile restoration. **It does not claim to have
produced the vendor worker's encoded GIF, frame trimming, frame-size response
or a device upload acknowledgement.** Custom imports have size zero until a
real encoder result exists, so no fabricated upload-time estimate appears.
The original worker path and its 15 FPS delay/150-frame informational copy are
recorded, but native worker-equivalent output remains a service implementation
boundary.

Replace/reset help uses the current module 58837 portal. Its module-level
`h.defaultProps` sets bottom-right (the constructor's fallback is a different
value, so it must not be used as mount evidence). Both call sites omit position
and add their source class's margin-left 270px. For each 28×27 action target,
the 300px main begins at target.left − 320 + target.width + 270. Vertically it
begins at target.bottom + 5. The shared native helper applies the source right-before-left
8px horizontal correction, no vertical flip, a 100ms entrance fade and portal
priority 1060. The tooltip target owns the action's 2px margin; its button does
not duplicate that margin. The full tooltip module, call sites and applicable
CSS are included in this audit's source receipts.

## Resources and permitted validation

`prepare-audio-oled-artwork-assets.py` emits a separate native asset table.
It checks decoded RGBA frames and total animation timing against every source
emote, then writes lossless WebP. Seven action/search/info/not-found SVGs retain
their literal source geometry. Existing current-1383 Animation/Image assets are
reused after their own source preparation, not sourced from another product.

`extract-audio-oled-artwork.cjs --check`, the asset preparer's `--check`,
`validate-audio-oled-artwork.py`, formatting and the parent task's
`cargo check --locked --all-targets` provide static verification. No app, build,
test, installer, downloaded JavaScript or DLL was run. No live pixel/interaction
acceptance claim is made.
