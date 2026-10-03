# Current audio, broadcast and haptic native controls

This is a **partial native implementation**, not a declaration that the product interfaces are complete. Product registration, an available control, and full UI fidelity are separate claims.

## Current source and reproducibility

The implementation uses each product's current files under `.ref/devices/<pid>/`. The current Dashboard and official host provenance are documented in `20-current-source-version.md` and `current-host-version-audit.md`. No obsolete frontend/ASAR/host source or historical extraction scripts are used here.

`tools/extract-audio-evidence.cjs` parses JavaScript with Acorn without importing or evaluating downloaded code. It resolves module exports and lexical bindings, follows mounted JSX references, and records source file SHA-256 hashes and exact component slices. Function parameter shadowing is respected; unresolved webpack module tables cannot become page components. Conditional component references retain both static branches without assuming their runtime condition.

`tools/extract-audio-additional.cjs` applies the same parser to the Wireless Control Pod, Stream Controllers, Freyja and the two chair products. The primary and additional evidence are in `audio-product-evidence.json` and `audio-additional-evidence.json`. `audio-product-configs.json` anchors the primary config and navigation locations; the early `audio-product-pages.json` inventory is not sufficient mounted-component evidence.

`tools/prepare-audio-products.cjs` creates `src/features/audio_products_data.json` and `audio-product-native-coverage.json`. Controls require a matching product state and a mounted field. Numeric bounds come from JSX props, component defaults, product config or explicit source state bounds. Source sentinels outside those bounds are normalized for the local draft and recorded in `default_normalizations`. EQ preset data is selected from the actual mounted EQ variant. The Control Pod's lazy mapping literals are separately recorded with source slices in `supplementary_source`.

`tools/validate-audio-products.cjs` validates scanner/generator hashes, all recorded source hashes, component and supplementary slices, navigation, control pointers, enable dependencies, range/selection defaults and EQ data. The authoritative latest counts and empty-page list are in `audio-product-validation.json`.

## Implemented behavior

The native module currently covers the 70 audio products plus PID 1382, 3334, 3337, 3909, 3942 and 3949. It includes product-specific volume, enhancements, microphone, lighting and power controls; audio and microphone EQ presets, band edits and resets; supported enhancement exclusivity; stream/playback mixer channels and linked faders; Seiren DSP and parametric EQ; Audio Mixer channel assignment, mixer and some effects; spatial calibration angle drafts; haptic master/region intensity and apply-all/reset coupling.

PID 1383 has OLED brightness, idle return time, dim time and language draft controls. PID 1465 has the source ANC/ambient selector. PID 3867's lighting page is an introduction to the Streamer Companion app and links to its official site. PID 1382 supports multimedia, default playback-device cycling and disabled assignments, with the source's reserved button enable states retained. This is not its complete assignment editor.

`AudioProductWorkspace` owns a local JSON draft and retains Slider/Select entities. Checkboxes are controlled by that draft. The parent owns the selected product/page, persistence and navigation. `snapshot`, `restore`, `set_page`, `source_product` and `supports_page` form the integration boundary; `AudioProductChanged` reports local edits. Restore only merges known shapes and revalidates source limits. `supports_page` reports a nonempty page so shared fallback controls do not replace a more complete product-specific page. Unknown or empty pages cannot silently retain the previous page.

No controls in this module issue device-service commands or claim hardware acknowledgement. Stream Controller data does not invent physical microphone/headphone inputs from unused shared defaults. Media playback, meters and device-dependent launch/update operations are not simulated. External links open only after the user's click.

## Remaining work and verification limits

The three remaining empty pages in this module are the demo pages for 1392, 1442 and 3942. Nonempty pages also remain incomplete: exact product artwork/layout, button/dial editors and advanced assignment categories, OLED home-screen editors/screensaver previews, spatial drag geometry and preview audio, complete lighting effects, mixer routing/application lists, media playback and some source-dependent device flows.

The available UI uses native Kit form controls with semantic theme tokens, retained input state and stable IDs. It has not been verified for pixel fidelity or interaction behavior in a running window.

Allowed verification performed for this work consists of formatting, static source parsing/resource validation, and `cargo check --locked --all-targets`. No application, build, tests, installers, downloaded JavaScript or DLLs were executed. Compilation alone does not prove visual or interaction parity.
