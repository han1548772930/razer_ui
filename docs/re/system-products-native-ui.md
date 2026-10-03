# Current system product native settings

Source date: 2026-10-02. Implementation work: 2026-10-03.

This is a new audit of `.ref/devices/<product_id>/static/js/`, not a relabeling
of an older host or frontend snapshot. `tools/extract-system-products.cjs`
parses the current bundles with Acorn and follows lexical bindings and webpack
exports. It interprets literals, primitive expressions, string concatenation,
and the inspected Babel object-spread helper over plain data. It never loads,
imports, evaluates or calls downloaded code. `tools/prepare-system-products.py`
checks every input SHA-256 again before generating native settings.

The complete receipts are `system-products-resolved.json` and
`system-products-native-audit.json`. Native data is
`src/features/system_products_data.json`; its owner is
`SystemProductWorkspace` in `src/features/system_products.rs`.

## Product coverage

All 41 current products with the inventory family `system` have explicit data
(199 mounted navigation entries: 158 non-Help entries and 41 shell-owned Help
entries). Every non-Help entry has a native body; unknown navigation keys show
an explicit incomplete-page state instead of retaining the previous page:

563, 564, 569, 570, 571, 576, 581, 582, 586, 587, 588, 589, 594, 595, 597,
598, 601, 616, 618, 621, 622, 623, 624, 630, 633, 634, 650, 651, 652, 669,
670, 671, 672, 694, 695, 696, 709, 710, 711, 736, 737.

The product list is asserted against the current inventory on every resource
preparation. A matching product ID is not a claim that all of its UI behavior
is complete. The limitations below remain part of this audit.

## Implemented native state and interactions

- Performance mode names, availability on battery, mode-specific default state
  and fan ranges come from each product's exports. Advertised modes with no
  configured state remain disabled pending capability data. CPU/GPU baseline
  choices are independently matched against each product's literal arrays.
- The modern fan UI in 709, 710, 711, 736 and 737 has Auto, Fixed and Smart
  modes; Smart is unavailable on battery. CPU/GPU temperature curves retain
  their own source point arrays, independent fan values, and reset values.
  Fixed-speed limits follow the current fan component constants. The source's
  configured curve temperatures are immutable during profile restoration.
- The 15 products whose mounted navigation has a battery page expose the
  battery optimizer's 50–80 percent limit in 5 percent steps. Charging override
  is shown only when a mounted component passes its supported flag. Override
  disables limit editing; turning off the optimizer disables override editing.
- Display uses the actual reducer seed of 60 Hz, disabled because it is not a
  hardware enumeration result. Windows display settings has a native command.
  For 711 and 737 the mounted source supplies explicit native display modes:
  3840 × 2400 at 240 Hz and 1920 × 1200 at 440 Hz. These can be retained as local
  requested settings; no restart or hardware success is claimed.
- Lighting retains separate plugged, battery and linked settings, brightness,
  logo effect IDs, quick effect selection, display-off, idle and low-battery
  conditions. Slider steps come from the mounted brightness control (`1`),
  not from the firmware's different `DeviceInfo.backlightStep`.
- Sound pages use configured volume and speech-processing values, THX enable,
  separate speaker/headphone equalizer curves, source preset tabs and resets.
  The speaker surface follows `convertSpeakerPresets` and exposes seven bands
  starting at 250 Hz while headphones retain all ten bands.
  A feature absent from the product's initial profile is not fabricated.
- Customize exposes source default mapped keys, a local disable/reset override,
  gaming-mode state and disabled shortcuts, and Fn-primary selection. The
  actual source values are `func` and `multi`.

The entity retains SliderState entities and their subscriptions; restoration
does not emit a user-change event. A profile snapshot keeps the source profile,
performance, display, battery and equalizer state distinct. Unknown object
fields are discarded, fixed arrays retain their source length, monitor
enumeration is not restored from arbitrary profile data, and out-of-range
edited numeric values are clamped. Untouched source defaults are preserved,
including curve values that are between the drag control's step increments.

The current official configuration still contains historical field spellings
on some IDs (`switchOffLightingInfo`, `GamingMode`, and shortcut `Enabled`
fields). Preparation records each local schema alias in the receipt; this
does not claim that the old field spelling belongs to a different source
version. Local drafts use the current page/reducer schema.

## Remaining source parity work

- Original SVG keyboard geometry, regional/edition keyboards, every remappable
  key, full assignment editor and source-specific Fn/Hypershift restrictions.
  The current mapped-key list is an interim native surface.
- Quick-effect color/direction/speed parameters, per-region lighting, vapor
  chamber lighting, Chroma and Windows Dynamic Lighting interactions.
- Capability-gated Boost/Max CPU/GPU modes, CPU/GPU overclocking, voltage/power
  controls, smart fan chart geometry and all dynamic fan-limit branches.
- GPU switching, detected refresh rates, installed color profiles, local
  dimming/HDR and remaining display models whose choices are supplied by
  hardware replies rather than literal mounted properties.
- Full sound-page processing/routing when a reducer's device data supplies
  values that are absent from DEFAULTPROFILE.
- Hardware bridge commands, acknowledgements and system monitoring. These
  cannot be replaced by writing local JSON or synthetic success banners.

No application, build, test, installer, downloaded JavaScript or DLL was run.
Resource generation and Rust formatting are permitted static verification.
`cargo check --locked --all-targets` passed after module and workspace
registration on 2026-10-03; the system module has no compiler warnings.
Rendered layout and input behavior remain unverified under the user's no-run
requirement.
