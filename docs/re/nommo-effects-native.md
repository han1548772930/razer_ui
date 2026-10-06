# Nommo 1303 / 1304 effects — current source audit

This audit uses only the JavaScript and CSS named by each current
`.ref/devices/{1303,1304}/asset-manifest.json`. It does not use an earlier host
or historical frontend snapshot. `tools/audit-nommo-effects.cjs` parses the
JavaScript with Acorn and records UTF-16 code-unit offsets and SHA-256 in
`nommo-effects-source.json`; it never evaluates reference JavaScript.

| Product | Lighting page | Effects panel | Quick effects | Advanced effects |
| --- | --- | --- | --- | --- |
| 1303 | `wm` | `ym → bm` | `xp → Kp` | `Um → Mm` |
| 1304 | `kM` | `zM → wM` | `Qm → qm` | `HM → fM` |

Both mounted pages have a left brightness/display-off column and a right
effects column. Both configurations export the same six choices, in this
order: Audio Meter (12), Breathing (2), Spectrum Cycling (3), Static (1),
Tidal (19), Wave (4). Module 8193 supplies the product configuration and
module 3254 export `U2A` supplies parameter defaults. Module 4693 resolves
the rendered translation keys, including `TEXT_DIRECTION`.

`audio_nommo_effects.rs` now replaces the descriptor-only Effects section in
both the main lighting page and the standalone `chroma-lighting` body:

- Quick/Advanced pills retain the source `iP/Tp` behavior: clicking either pill,
  including the active pill, toggles the selected mode.
- Audio Meter has the source 0.25–4 color boost, increment 0.25, with typed
  values rounded upward to the next increment. The shared source `Stepper`
  retains 300 ms press repeat, focused wheel
  stepping and Enter/Escape blur commit. 1303 `Lp` and 1304 `Um` provide the
  independent current behavior receipts. No unmounted speed slider is added.
- Breathing and Tidal expose both source colors and random-color disabling.
  Static hides the no-color choice. Spectrum has no parameter body.
- Wave uses the product's `CWCCW` 11/12 direction branch, with the initial
  value 12 extracted from `Kp/qm.handleDefaultWaveDirection`'s direction map.
  Tidal uses its
  default outward/inward 1/0 branch. Direction pills follow the same source
  toggle behavior even when the active side is clicked.
- Advanced effects uses the source installed-branch layout, its empty-profile
  presentation and the 240×50 Studio button. Studio is a distinct target from
  the locally implemented Chroma Dashboard, so its unavailable launch action
  is disabled. No download or install prompt is inserted.
- Local profile snapshots retain each effect's parameter cache and Advanced
  selection. Restore constrains colors, boost and directions to the current
  source domains; control synchronization suppresses change callbacks.

The source glyphs for CW/CCW, inward/outward, Chroma sync and Chroma Studio
were fetched separately for both products. `tools/prepare-nommo-effects.py`
restricts requests to manifest-declared SVG resources and verifies exact
equality with the existing prepared assets. `nommo-effects-assets.json`
contains 24 URL/SHA-256/equality receipts including stepper arrows; no duplicate asset registration
was necessary.

The source palette `sP` / `Ap` contains the same 40 color values plus
`no-color` as the shared `LightingColorPicker`; this equality is checked by
the maintained audit tool. `CP` / `pp` records its 16 custom slots, context
menu, custom-color Save/Cancel and upward-opening branch. The shared picker
retains these interactions and the source 100 ms palette opacity transition.

Geometry and appearance are taken from the current CSS: `.twoway-lighting`
36 px, 18 px radius, 5 px inset; tabs 26 px, 13 px radius; `.toggle-btn`
42 px with 40×30 px direction tabs and 20 px SVGs; effect dropdown 150 px
with 20 px trailing margin; sync SVG 26 px and 10 px trailing margin;
parameter top spacing 20 px and Tidal direction spacing 10 px. The explicitly
declared quick-effect description, advanced body text and tabs use Roboto
14/17 px; advanced detail text overrides its line height to 18 px. Remaining
labels inherit line height, preserving the source `normal` boundary instead
of treating an arbitrary numeric GPUI line height as equivalent. The existing
panel supplies RazerF5 16 px title,
40/30 px insets, 5 px corners and the source title/help arrangement.

`audio_nommo_effects_theme.rs` owns the source colors (`#111`, `#ccc`,
`#5d5d5d`, `#44d62c`). Pill border changes use 200 ms ease, pill background
500 ms ease, and tab foreground/background 200 ms ease with sRGB channel
interpolation. Nommo brightness now shares the current 300 ms thumb
background transition while preserving dragging when brightness is off.

Validation performed: Acorn extraction, literal/default and translation
resolution, source palette equality, 24 SVG byte comparisons and rustfmt.
`node tools/audit-nommo-effects.cjs --check` compares the re-extracted receipts
and defaults without writing files; missing/duplicate required declarations,
missing palettes and unknown command flags fail rather than silently passing.
The parent task performs the permitted `cargo check --locked --all-targets`.
No application, build, test, installer, downloaded JavaScript or DLL was run.

The current native workspace has no live Chroma device inventory or service
profile feed. Consequently the source no-sync-peer and empty-Studio-profile
branches are shown. The source connected-peer 1200 ms sync action and active
Chroma-app takeover status are recorded but not fabricated from catalog
entries. Real service data, Chroma profile synchronization and device writes
remain outside this local draft implementation. No runtime visual equivalence
is claimed by static checking.

Both Audio Meter components mount `active=true, maxLength=4, maxValue=4,
minValue=.25, allowDecimals=true, roundUpDecimals=true, stepValue=.25`.
The source default `colorBoost` is 1. These parameters were checked against
the native `Stepper::new` call after the initial integration. Its 14×12 px
spinner and 8×4 px SVG agree with the final CSS cascade; the shared stepper's
`wired-argb-3871-stepper_{up,down}.svg` assets are byte-identical to the
Nommo resources. The opt-in initial-hidden/hover-focus spinner visibility is
now implemented and audited in [stepper-visibility-current-audit.md](stepper-visibility-current-audit.md).
That audit separately records the remaining bounded-spinner pointer behavior.

The Studio launch chain was checked separately: 1303 `um → Cm` and 1304
`mM → pM` first activate/open `/chroma-app/dashboard/index.html?action=focus-module-installer`,
send the Studio subtarget to an existing Chroma host, and additionally focus
an existing Studio window. Merely emitting the local `OpenChroma` event would
drop that Studio target, so this incomplete bridge was removed. A real Studio
implementation/host integration remains necessary before enabling this button.
