# Current camera tooltip layer (2026-10-06)

The camera help and conditional warning portals now use deferred priority 9999
while their trigger is hovered. On mouse leave the priority immediately returns
to 200, while the existing visibility/opacity transitions continue. This fixes
the camera-specific difference identified by the presentation audit.

All four current manifests (3592/3594/3595/3596) name CSS with the same effective
rules: `.drop-tips{z-index:200}` and `.drop-tips.on{z-index:9999}`. Their current
tooltip function portals the visible element directly into `document.body` and
chooses `"drop-tips on"` versus `"drop-tips"` using the immediate hover state.
The high layer is therefore controlled by hover, rather than the longer-lived
fade visibility state.

`SourceTooltip::hovered_priority` is an optional override. Its default is
`None`. Only `source_controls/camera_sections.rs` opts in, at 9999; the existing
200 default and the widget/receiver portal priority 10001 remain unchanged.
The parameter changes neither tooltip placement nor animation timing.

The native parent and GPUI sources were read to verify the actual paint path:

| Surface | Native ordering |
| --- | --- |
| Camera content | Normal flow inside the source workspace's scroll container |
| Base popup | Deferred priority 100 |
| Base dialog | Deferred priority `10 + layer`; the sibling profile transfer dialog requests layer 3 |
| Existing default source tooltip | Deferred priority 200 |
| Hovered camera help/warning | Deferred priority 9999 |
| Existing widget/receiver tooltip | Deferred priority 10001 |

The camera tooltip's `TipOverlay` is the only deferred wrapper on its path.
Base `Tooltip` itself is a presentation surface and does not create another
deferred layer. In the locked GPUI 0.3.8 implementation, `Deferred::prepaint`
calls `defer_draw(..., priority, None)`, so the parent's scroll clip is not
copied into the portal. The window sorts all deferred draw indices by ascending
priority and uses that order to paint, including nested deferred elements.
Thus the camera override reaches the real paint queue instead of only changing
an unused style value. Component root sheets, dialogs and notifications remain
owned by the existing root layer renderer.

[camera-tooltip-layer-current-evidence.json](camera-tooltip-layer-current-evidence.json)
contains current manifest/CSS hashes, the exact portal JS, native caller and
policy excerpts, parent camera/workspace/profile-overlay excerpts, and locked
framework source excerpts. `tools/audit-camera-tooltip-layer.py` generates this
receipt. Its `--check` mode recomputes and compares without writing, including
the assertion that the camera helper is the only caller of the new override.

Formatting, the dedicated static audit, the existing camera presentation
validator and `git diff --check` pass. The parent task owns the combined
`cargo check --locked --all-targets` result. No application, build, test,
installer, downloaded JavaScript or DLL was run; visual stacking and pointer
dispatch in a real window have not been runtime-verified.
