# Current camera groups and processing conditions (2026-10-06)

This audit re-reads the current manifests and main bundles for 3592, 3594,
3595 and 3596. It supersedes earlier claims that 3592 has no resolution
selector anywhere, that every extracted MJPEG child is mounted, and that the
camera section titles have no disclosure behavior. It does not use any of the
deleted historical source directories.

`tools/generate-native-camera-controls.cjs` now calls
`tools/camera-presentation.cjs`. The latter validates the existing component
excerpts against the current bundle, parses each actual JSX call with Acorn,
resolves its effective mount properties, and emits `camera_groups` alongside
the controls. [camera-presentation-current-evidence.json](camera-presentation-current-evidence.json)
records full bundle/CSS hashes, exact JSX excerpts and offsets, the actual
wrapper and tooltip functions, and applicable CSS rules. No downloaded
JavaScript is executed.

## Mounted behavior

- **3592 PROCESSING:** its resolution/MJPEG component contains the resolution
  selector. The descriptor now includes `3592:processing-resolution` with the
  component's `Pr` array and `/camera/resolution` binding. The CAMERA page still
  does not mount the separate preview-source/resolution component.
- **3594 PROCESSING:** the root passes `supportMJPEG:!1`. The whole MJPEG group
  is therefore absent. HDR remains mounted and interactive.
- **3595 PROCESSING:** the root passes `disableHDRSwitch:!0`, but the callee
  actually uses that flag in `E?null:o.autoQuality?...` around the quality-mode
  and manual-quality children. Only those two children are removed. Automatic
  MJPEG quality and HDR both remain mounted.
- Processing rows follow the root's component order: resolution/quality and
  HDR first where present, then LDC, low-light compensation and noise reduction.
  The IMAGE page follows image settings, anti-flicker, watermark where supported,
  then mirror video. Image brightness/contrast/saturation/white-balance rows keep
  the source's transparent separators, while the image-preset boundary is solid.
- The LDC processing explanation is visible only when LDC is enabled and
  `resolution.height >= 1440`. This is a different predicate from the framing
  pad's exact 4K/1440p 30FPS disable condition. Low-light descriptions are mounted
  for 3592/3594/3595 and absent for 3596, matching their actual JSX.
- The three mounted preview-resolution rows show the warning icon only when
  `autoFraming.isEnabled || hdr.isEnabled`. The warning text is the current
  localization export `pKo`. Missing runtime fields do not become enabled.

## Wrapper and tooltip presentation

The native renderer mounts 64 source groups, of which 51 are collapsible.
This includes the six Camo/preview-source disclosures audited in
[camera-preview-current-audit.md](camera-preview-current-audit.md).
`useState(!0)` initializes the original wrapper open; its click handler does
`noToggle || setOpen(!open)`, and its content is conditionally mounted without
an animation. Native Base `Collapsible` follows that immediate mount/unmount
contract. Group IDs use product, page and source title. Page changes reset the
disclosure state, matching remounting the original page components.

Header switches, reset buttons, numeric editors and the white-balance Auto
checkbox remain outside the collapsed content. They occur in the source order
and no longer duplicate the title in a second settings row. The source
`.function-name` uses uppercase Roboto 14px/16px; the disclosure icon uses a
10px square slot and 10px right gap, rotated by -90 degrees when closed. Header
switches/editors/reset buttons use the source 10px left gap. The reset glyph is
20px and has 0.7 hover opacity.

34 header help entries are now visible, including watermark, HDR, autofocus,
exposure, zoom, image presets and processing groups. The current tooltip
function is the same `.drop-tips` portal contract already implemented by
`SourceTooltip`: 210px hidden measurement (280px for resolution warnings),
client-size copying, independent right/bottom viewport fallback, 14px/16px text,
8px/10px padding, `#000` background, `#5d5d5d` border and `#ccc` foreground.
Opacity transitions for 300ms linearly, visibility for 200ms, and top/left for
100ms. The question-mark glyph has a 14px slot, 7px radius, `#6c6c6c` background,
and a 300ms transition to `#ffffff4d`; warnings use the source 20px SVG.

[camera-section-assets-current-evidence.json](camera-section-assets-current-evidence.json)
records seven SVGs from all four current manifests. Prepared files are compared
byte for byte across the roots. `tools/prepare-camera-section-assets.py`
prepares their bytes; `--register` updates the shared asset tables explicitly,
and `--check-registration` only verifies the tables and hashes.

## Verification and limits

`python -X utf8 tools/validate-camera-presentation.py` validates all source
excerpts and hashes, source-to-group equality, complete single ownership of
every control, header control kinds, English/Chinese localization keys, the
three corrected processing mount branches, and SVG/manifests. It reports
64 groups, 51 collapsible groups, 34 help entries and 3 conditional warnings.
The camera descriptors also pass `validate_controls` from the existing native
product validator. Rust formatting was run; the parent task owns the combined
`cargo check --locked --all-targets` result.

No application, build, test, installer, downloaded JavaScript or DLL was run.
There is no live-window pixel, focus or pointer acceptance claim. Camera stream
transport, enumeration, third-party application detection, USB capability
warnings, firmware-dependent focus modes, watermark processing output and some
subcontrol-specific geometry/tooltips remain incomplete. In particular, this
work does not pretend a camera, HDR transport, or a third-party service exists.
The camera help and warning callers now opt into deferred priority 9999 while
hovered and return to 200 immediately on mouse leave, matching `.drop-tips.on`
and `.drop-tips`. Other `SourceTooltip` consumers keep their existing layers.
The source, parent-overlay and framework ordering proof is recorded in
[camera-tooltip-layer-current-audit.md](camera-tooltip-layer-current-audit.md).
