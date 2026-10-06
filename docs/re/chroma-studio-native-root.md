# Current independent Chroma Studio native root

Source is the current `/synapse/chroma-studio/` application, not the Chroma
Dashboard and not a product's `displayMode=chromaApp` branch. Manifest and all
source hashes/ranges are in `chroma-studio-source.json`. `xt` selects the
independent editor module 4264 (EditorCanvas.c3ee330f.chunk.js); `nt` owns the
38 px host toolbar and 48 px Studio navigation. The local root renders the
editor navigation and three editor regions; the shell owns window chrome.
The current host `PC.init` places Studio inside `chroma-app` as a policy 3
child tab. Its parent is 1280 x 720 with a 600 x 500 minimum, and its tab strip
uses the shared current TabUI 42 px geometry. The implementation keeps
Dashboard and Studio as distinct roots in that Chroma host window. The
machine receipt `chroma-studio-window-source.json` includes the current host
ownership class, title table, parent opener flags and 62 tab CSS rules.

Implementation is `src/features/chroma_studio.rs`, with separate palette and
canvas modules. The source-owned geometry is expressed through `surface::css`
except for painting measured canvas bounds. GPUI Base Button owns focus,
keyboard activation and disabled state; GPUI DropdownMenu owns popup keyboard
navigation, Escape and focus restoration.

## Implemented scope

- J: 250 px layer panel, effect library in source order, 76 x 72 px tiles,
  current layer selection, source default effect values/parameters, empty
  groups, duplicate/delete/visibility, inline rename and local history. Rename
  follows current EditorCanvas.Z: Enter/blur commits trimmed nonempty text,
  Escape cancels, same-kind duplicate names do not commit, and input is limited
  to 32 UTF-16 units. Its function and production title-limit constant are
  recorded in `chroma-studio-window-source.json`. Reactive and Ripple
  require a real keyboard/mouse/keypad/system item, so they remain disabled
  when no such item has been enumerated.
- vt: empty device canvas. No product catalogue entries are turned into
  connected devices. The Move tool renders the source's 480 x 480 SVG tile,
  all 145 source rectangles, minor-grid cutoff at zoom <= 50%, stroke scaling
  and center axes. Default tool, zoom and preview values match module 1638.
- me/re/De/Se/we: tool selection, source keyboard shortcuts S/P/B/M,
  Ctrl+Z/Shift+Ctrl+Z, Ctrl+S, Ctrl+L, Ctrl+D/Ctrl+E, source zoom levels and step
  boundaries, preview enabled/speed preference and device-name preference.
  The preview preferences are local editor state, not a native engine reply.
- Local save emits a strictly validated serializable snapshot. The shell's
  persistence owner acknowledges the exact submitted snapshot, so edits made
  during disk I/O stay dirty. Saving remains enabled while a write is pending,
  including when Undo restores the previously saved snapshot. Retained
  documents discard transient rename inputs before a new host attachment
  focuses them. The format does not claim vendor compatibility.
  Unknown fields/effects, duplicate IDs and parameters this version cannot
  represent are rejected rather than silently discarded. Nommo's source
  Studio launcher emits `AudioStudioRequested` through the workspace's
  `OpenStudio` event; it does not open Chroma Dashboard in place of Studio.

## Remaining scope (not completed by this root)

The Ambient region presets, region preview and blur control are now mounted,
with the original 1..9 range, 30px/4px edge strips, disabled select/move state,
and transient paint/bucket parameter ownership. Native region selection and
screen highlighting still require a real service. Static now mounts the separate source color editor (HSV field, brightness,
HEX/RGB, fixed presets and session custom colors); see
`chroma-studio-color-native.md`. The other eleven lazy effect
property editors remain unported; their current roots and shared components
are recorded in `chroma-studio-properties-source.json`. Parameter edits alone
do not mark a device-free document dirty: source UA updates the working buffer,
while applying to device regions is the separate persistence boundary.

Layer change-effect and duplicate-title rules are implemented; see
`chroma-studio-layers-native.md` for the native submenu difference. Other
remaining work includes drag order, populated/nested groups, group duplication;
profile selector/add/import/export/link
games; help/tutorial and service-backed modals; resizable panels and compact
breakpoints; quick LED selections, device geometry, panning/rotation/painting,
center-point editing, clipboard and native engine/frame transport. Ctrl+F
currently restores 100%; fitting a device selection needs real layout data.

No synthetic device, login, installation state or success response is created.
The source's `device.hasItems && items.length === 0` no-device modal is not
displayed merely because the local editor has not enumerated devices.

## Static verification

`python tools/validate-chroma-studio.py --check` checks all 44 existing source
receipts against their current file hashes and UTF-16 ranges (locale export
inventories are file-hash checked), verifies source/output asset hashes, and
re-extracts the 145 SVG grid rectangles. Grid-specific source range and hash
are in `chroma-studio-grid-source.json`. This tool only reads source text; it
does not import or evaluate downloaded JavaScript.
`python tools/audit-chroma-studio-window.py --check` verifies the separate
8-receipt host/rename artifact and current favicon. Studio's HTML names
`./favicon.svg`; its bytes match the current Chroma Dashboard `icon.svg`
and embedded `host-app-chroma.svg` (SHA-256
`ceb14ca48686034903a07d0780d0060a1d3ac818a3a33eed1cbbf1d1a0b80293`).
`rustfmt` was applied.
Application execution, builds and tests were not performed; the main agent
owns the permitted unified `cargo check --locked --all-targets` verification.

This is a source-grounded, independently reachable local editor root, not a
claim that all Studio interactions or visual states are complete.
