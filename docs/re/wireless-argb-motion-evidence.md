# Wireless ARGB overlay and motion evidence

Audit date: 2026-10-05 (Asia/Shanghai). The source is limited to the current
`.ref/devices/3884/` and `.ref/devices/3886/` bundles; vendor JavaScript was
read as static evidence and never executed.

The mounted top view in both bundles owns the auto-detection click transition.
The inactive SVG receives `icon-detection--animation`; its `detect-b` glyph
uses `zoomout` for 50ms and its `detect-c`/`detect-f` rings use `zoomoutc` and
`zoomoutf` for 700ms. The enabled SVG uses `zoomin` for 100ms. The source
also gives the detection, refresh, warning and close-icon paths a 300ms
`transition: all`; the source tooltip component is mounted on mouse enter and
removed on mouse leave.

Receipts are in [wireless-argb-current-evidence.json](wireless-argb-current-evidence.json):
the 3884 top view and `rU`/`TU` SVG components are recorded at the exact source
offsets, and the extracted CSS rules include the animation and overlay
selectors. The relevant source classes are `icon-detection--animation`,
`icon-detection--active`, `warning-container--overlay`, and
`customize-container--wireless`.

`WirelessArgb` now assigns a generation to each auto-detection request. The
local `AutoDetectionIcon` plays the audited 100ms active or 700ms inactive
phase, with the 50ms glyph portion preserved in the inactive interpolation,
and uses a keyed 300ms hover motion for the icon wrapper. The generation is
visual-only: a missing service response never becomes a fabricated hardware
state. The initial icon remains static, matching the source's `isFirstRender`
branch.

The extracted CSS declares `.warning-container--overlay` for a shared warning
component, but the mounted 3884/3886 top views call that component without the
overlay prop in their no-power branches. The local page therefore keeps these
cards in normal flow instead of inventing an overlay mount. Product and port
observations remain service-owned and are not serialized.

The path-level recoloring is now implemented as well. `.icon-detection:hover
.detect-b{fill:#44d62c}`, `:active` `#39a029`, `.icon-detection--active:hover
.detect-a{stroke:#96ef89}` and the refreshing/power/warning/close-glitter
recolors each target one element, so `prepare-wireless-argb.py` splits that
element into a `currentColor` layer and the native page supplies the audited
colors on the icon wrapper. The shared
`transition:all .3s` selector list names the icon roots (and the `:hover` state
of the refreshing/warning paths), not the recolored glyph, so those recolors are
intermediate-free: the earlier local 300ms hover opacity fade was an
approximation and is gone. `@keyframes zoomout/zoomoutc/zoomoutf/zoomin` are now
played layer by layer — scale .8->1 for 50ms on the glyph, .4->2 with a fading
`#44d62c` square and .2->1.4 with a fading `#666` square for 700ms, and
opacity 0->1 with scale 1.8->1 for 100ms on the enabled glyph — using
`Easing::EaseOut` for the source's `ease-out`. No hardware or service result is
inferred by this local motion layer; the verification remains static.

Validation is static only: `cargo check --locked --all-targets` and source/hash
audits. The application, downloaded JavaScript, DLLs and installers were not
run.
