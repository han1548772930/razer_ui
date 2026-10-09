# Core X V2 fan controls — current source

Product 3921 now routes TAB_CUSTOMIZE to the accessory workspace. Its native
fan editor uses `main.77ab7933.js` and `684.ebbca785.chunk.js`, with hashes and
reducer-slice provenance in `crates/razer-pages/src/features/corex_fan_data.json`.

The current product CONFIG supplies three presets and separate GPU/chassis
curves (47 points in six curves). The reducer's `try` branch reads module 8193's
DEFAULT_CURVES; its different generic fallback is not used. `Smart` serializes
as `Manual`. Initial mode is Smart/Manual, preset quiet, target GPU.

Implemented local interactions:

- Auto/Manual radio selection; source preset and temperature-source dropdowns.
- 820×410 curve geometry, original axis arrays, 75-pixel padding and integer
  grid-step rounding. GPU axis ticks are not uniformly spaced temperatures;
  their grid spacing deliberately follows the original renderer.
- Vertical node dragging, RPM clamped between adjacent points, adding near
  the interpolated line, and deleting selected points (2–20 points).
- Celsius/Fahrenheit and RPM/percentage display choices. Plot coordinates map
  1000–2800 RPM to 0–100; displayed percent is RPM / 2800, as in source.
- Reset restores both curves in the active preset. Its enabled state compares
  active-target point counts and fan speeds, matching the source predicate.
- Saved editable coordinates are validated for count, ranges, increasing
  temperatures and nondecreasing speeds; invalid curves restore source defaults.
- Keyboard focus: left/right select nodes, up/down adjust RPM, Delete removes
  a selected node while preserving the minimum count.

Boundaries: hardware transport, live monitoring, product illustration, exact
tooltip/hover styling and platform visual acceptance remain incomplete. No
current-temperature dot, device temperature or measured RPM is fabricated.
The native editor remains partial; this page must not count as fully reproduced.

Reproduction: run maintained `extract-accessory-system-products.cjs`, then
`extract-corex-fan.cjs`, then `prepare-accessory-system-products.py`. Static
validation is in `validate-native-product-data.py`. Only `cargo check` is used
for Rust verification; no application, build command or test program is run.
