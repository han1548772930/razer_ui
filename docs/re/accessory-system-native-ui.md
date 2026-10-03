# Current monitor and cooling accessory controls

Status: native local controls implemented for five products and nine non-lighting
pages; hardware integration and full rendered parity remain incomplete.

Only the current `.ref/devices/<pid>/` sources are implementation evidence.
`tools/extract-accessory-system-products.cjs` resolves webpack exports and mounted
components by static AST traversal. It does not execute the downloaded program.
`tools/prepare-accessory-system-products.py` verifies every source file hash and
prepares the native data. Source paths, hashes, offsets and extracted JSX are in
`accessory-system-source.json`; the compact receipt is
`accessory-system-native-audit.json`.

| Product | Native pages | Implemented local settings |
| --- | --- | --- |
| 3858 Raptor 27 | Gaming, Color, Display | Six gaming presets, brightness/contrast/overdrive, Gamma 1.4/1.8/2.2, color temperature and custom RGB, input source, PIP/PBP, adaptive sync, HDR and refresh-rate-counter position |
| 3880 Raptor 27 165Hz | Gaming, Color, Display | Above plus Gamma 2.4, Native/Rec.709/DCI-P3 gamut and THX Cinema; installed ICC profiles and supported refresh rates show unavailable data |
| 3893 Hanbo | Performance | Separate fan and pump mode requests; actual modes, RPM and curves remain unknown until hardware supplies them |
| 3900 PWM controller | Performance | Eight profile port settings, linked fan mode, Quiet/Normal/Performance/Manual/Advanced, manual speed and monotonic nine-point curve |
| 3907 Laptop Cooling Pad | Performance | Enable, Fixed/Smart modes, separate fixed preset values and limits, independent CPU/GPU smart presets, Celsius/Fahrenheit, percent/RPM display and selected preset reset |

Lighting remains in the supplemental source-controls workspace. The generator
now supplies 3893/3907 lighting values from their own reducer seeds because their
`DEFAULTPROFILE` objects contain only profile identity. The provenance is recorded
in `accessory-controls-audit.json.initial_sources`. Hanbo's current `hideLightingIdle`
is respected. Product 3929 mounts its lighting within its sole Customize page and
does not gain a fabricated Lighting tab.

## Behavior verified against current source

- Monitor Gaming displays the selected preset's data. The first manual change
  copies that preset into `customData` and selects Custom, matching the reducer;
  the unused `DEFAULTPROFILE` custom values are not shown as an active preset.
- Overdrive values are Off 0, Weak 1, Strong 2. Gamma 3 is exposed only on 3880.
  Only 3880 mounts the color-gamut control. Non-Native gamut disables contrast
  and gamma; a selected Custom non-Native gamut also disables color controls.
- PIP sizes are Small 1, Medium 2, Large 3, with corner positions 0–3. The refresh
  rate counter uses its distinct position values 1–4. Secondary input choices
  are DP 15, HDMI 17 and USB-C 19; Auto 0 belongs only to the primary source.
- PIP disables HDR and Adaptive Sync. On 3880, enabled THX Cinema disables
  Gaming/Color; sRGB color temperature disables THX Cinema. Windows HDR and
  duplicate-display state cannot be inferred from local requested values.
- PWM manual fan speed is 33–100% step 1. Its advanced chart clamps to 25–100%
  and between adjacent points. Enabling linking copies the selected mode;
  Manual copies its speed, Advanced its curve. This is local profile intent,
  with no fabricated active-port detection or 1250-RPM demonstration values.
- Cooling Pad fixed modes use Low 500–2000, Medium 1500–2500 and High
  1900–3200 RPM, step 50. Smart uses the source's independent CPU/GPU presets,
  500–3200 RPM and monotonic adjacent-point constraints. Display percentage is
  RPM/3200, matching the source's 15.625% lower endpoint. The seeded `cpu_gpu`
  data is retained but no extra sensor tab is invented.
- Slider entities and subscriptions are retained outside rendering. Restoring
  a snapshot resets against the current source schema, rejects unknown fields,
  validates choice values, fixes port IDs and temperature coordinates, preserves
  fixed array lengths, clamps numbers and does not emit a user-change event.
- Page selection always accepts the shell's requested source key. Unsupported
  pages show an explicit incomplete state, never the previous page's controls.

## Remaining work

- Monitor source product images, input-source confirmation overlay, source PIP
  placement diagram, ICC install/select commands, detected refresh rates,
  Windows HDR integration and complete backend-supplied UI constraints.
- Hanbo detected fan/pump curves, hardware limits and update-required states.
  The current source starts with `ports: []`; no usable curve default exists.
- PWM port artwork, renaming, detection and active-port-only hardware routing.
- Cooling Pad smart curve add/remove node interactions (source supports 2–20
  nodes), sensor/system status panels, compatible Blade performance navigation,
  hardware capability gates, HyperBoost and mapped-button controls.
- Interactive dragging on the source chart is currently represented by native
  retained sliders and a line plot. Exact source geometry and measured pixel
  alignment are not verified.
- These local drafts do not invoke hardware commands or present success as a
  device acknowledgement. Persisted local settings are separate from observed
  device state.

`cargo check --locked --all-targets` passed after workspace integration on
2026-10-03. No application, build, test, installer, downloaded JavaScript or DLL
was executed. Rendered appearance and input behavior remain unverified under
the user's no-run restriction.
