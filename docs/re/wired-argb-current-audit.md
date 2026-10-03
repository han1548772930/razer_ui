# Wired ARGB Customize pages: current source audit

Audit date: 2026-10-03 (Asia/Shanghai). Scope: product 778 ASRock B550 Taichi Razer Edition and product 3871 Razer Chroma Addressable RGB Controller, `TAB_CUSTOMIZE`.

Only the current `.ref/devices/778/` and `.ref/devices/3871/` JavaScript/CSS and their declared bitmap/SVG assets were used. Vendor JavaScript was parsed with Acorn; it was never executed. `wired-argb-current-evidence.json` records source SHA-256, exact UTF-16 offsets, complete mounted components, LED limits, reducers, resource declarations and CSS. This does not treat older host/frontend references as evidence.

## Source behavior and native ownership

`WiredArgbWorkspace` owns observed ports and transient service state; each `PortEditor` owns the retained framework controls and its local layout edits. Device drafts contain names, strip/fan mode, separate strip/fan segments, Chroma reminder dismissal and an optional requested auto-detection setting. Observed activation, detected LED counts, maximum supported counts, power, refresh results, protection state and installation state never enter the draft. With no ARGB transport, the actual page reports that observations are unavailable. It does not manufacture detection or successful device writes.

The edit scope is **device/localstorage**, not a lighting profile. In source, the port layout belongs to `portsReducer`; `pH`/its 778 counterpart forwards `ON_SET_PORT_VALUES`. Auto-detection belongs to `argbControllerReducer` and requests `ON_SET_AUTO_DETECTION_ENABLE`. Current `DEFAULTPROFILE` contains brightness, quick effects, switch-off lighting and mappings, with no port layout. The host integration stores `snapshot()` in the device's `source_device_settings`; changing lighting profiles must not reset the physical layout. Exact source receipts are under `state_scope` and `editReducers`.

Source distinctions retained:

- 3871 places ports 4–6 to the left and 1–3 to the right of the 130×270 controller image. The current 1024px rule moves the image above the columns. 778 uses a centered product image and the filtered/sorted port IDs 2147483651, 2147483652 and 2147483656, with 600px cards. Art is extracted separately for editions 0, 128 and 129.
- Port name commits on Enter or loss of focus, rejects empty names and limits UTF-16 length to 32. Escape cancels inline editing. Local segment IDs remain stable when another segment is removed.
- Fan mode is available only when the observed per-port maximum is 80. The dropdown contains exactly 6, 8, 9, 10, 12, 15, 16, 18, 20, 22, 24, 25, 32 and 40 LEDs. Source minimum LED count is 1 for both products.
- Adding a 90° bend uses the source's recursive ceiling distribution, including its minimum behavior. A strip has at most four sides. The fourth side cannot exceed the second; reducing equal second/fourth sides updates both. Switching modes retains the separate strip/fan values. Adding a fan preserves existing values and appends 20, and is disabled when the active total reaches the observed maximum. Removing a segment does not redistribute the remainder.
- Per-port maximum warnings and the controller's 240 LED warning are distinct. No-power and protection states suppress port editors; no-device and refreshing states have separate original presentations. Dismissing the total-limit warning does not change the counts or hardware state. Changing layout clears the Chroma reminder's dismissal. Chroma Studio commands report unavailable service rather than an invented launch.

## Profile bar: important source distinction

Both product roots pass `isEnableProfileBar: active_view === TAB_LIGHTING`. Lexical export resolution confirms that 778 `FO.lgc` and 3871 `bO.lgc` mean `TAB_LIGHTING`.

This flag disables **only the sync icon**. The consumer's `renderProfileBarIcon` sets `className:"loader disable"` when it is false. The surrounding `.profile-bar` is disabled separately by `enableSwitchProfile()`, which reads `profileReducer.enableSwitchProfile` and a dynamic Loupedeck condition. `enableSwitchProfile` defaults to true. Consequently Customize retains the profile dropdown/actions; its sync indicator is disabled. It is incorrect to hide the bar or disable all its controls based on `isEnableProfileBar`. `profile_bar` in the evidence contains producer, export, consumer and initial-state receipts.

## Geometry, assets, color and animation

Source geometry uses `surface::css`, preserving the original 16px CSS-root ratios through GPUI zoom. Port cards use 460/600px widths, 26px top/30px bottom/40px side insets, 199px minimum height, 5px radius, 150px device selectors, 62×27px LED controls and 100×100px strip/fan images. Names use RazerF5 16px; other port text uses Roboto 14px. Help sits at top/right 10px, with the original 14px questionmark artwork. Source resource modules supply the rename, remove, refresh, warning, detected-count, auto-detection and stepper artwork.

The current application theme exactly supplies source background `#222`, card `#111`, text `#ccc`, border `#5d5d5d`, primary `#44d62c` and warning `#fd8611`. ARGB-specific `#707070`, help `#4a4a4a`, help hover `#ffffff4d`, stepper hover `#ffffff1a` and stepper active `#0000001a` are centralized in `wired_argb/theme.rs`. The dots and radial dim layer are a static SVG translation of the source's 22px CSS grid and radial gradient. The warning panel occupies the source overlay position, 20px from the top, rather than adding a new section below the layout.

`detecting.f2110e0d.svg` is retained and statically decomposed because GPUI's image decoder does not execute SMIL. Its original paths, masks and embedded ring bitmap are preserved. Native animation uses a 2-second linear ring rotation (120 prepared 3° frames), and the original 18 LED opacity tracks: 0.5-second fade in, 0.5-second fade out, 0.1-second stagger, 1.8-second cycle. It does not change service results. Reduced motion displays a static frame. `prepare-wired-argb.py` verifies timing attributes against the source and records all derived assets.

## Reviewable limits

The state-sample dialog reuses the complete `WiredArgbWorkspace` body, with a clearly separate developer fixture toolbar. The main application also provides the normal product-tab entry through the shared source workspace; the sample dialog is not presented as a substitute for the product shell. The toolbar explicitly supplies power, detected-count, protection and refresh-result samples. Examples do not enter the live device list.

This audit does **not** claim rendered pixel parity: the user prohibited launching the application, builds and tests. Native focus, scaling, rendering and layout still require a permitted interactive review. Source hover transitions (notably 300ms icon/help transitions), the auto-detection 50/100/700ms click animation and continuous press-and-hold stepper repetition are not yet reproduced. Pointer/keyboard activation and numeric input work; no synthetic service outcomes are attached to these actions. The ring preserves source geometry and timing at a discrete 60Hz frame resolution. Generic Kit popup focus/keyboard behavior remains in the source-styled Select and Input controls.

## Verification and maintenance

`node tools/extract-wired-argb.cjs --check` compares maintained generated data/evidence with current sources. `.work/resource-env/Scripts/python.exe tools/prepare-wired-argb.py` prepares only declared resources and static animation layers. `python tools/validate-wired-argb.py` validates current source hashes, exact component receipts, locale keys, physical-state separation evidence and every prepared asset. `rustfmt --edition 2024` formats the Rust modules. The parent agent performs the consolidated permitted `cargo check --locked --all-targets`; no application, build, test binary, installer, vendor JavaScript or vendor DLL was executed for this audit.

Integration seam: `supports_page(pid, key)`, `WiredArgbWorkspace::new(&Device, window, cx)`, `snapshot() -> Value`, `restore(&Value, window, cx)`, `dismiss(window, cx)`, `WiredArgbChanged`, and `open_preview(window, cx)`.
