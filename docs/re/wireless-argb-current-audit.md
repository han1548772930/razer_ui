# Current wireless ARGB customize pages (3884 / 3886)

Audited on 2026-10-03 using only the current product bundles recorded by
`pending-product-3884-source.json` and `pending-product-3886-source.json`.
`tools/extract-wireless-argb.cjs` parses these bundles with Acorn. Vendor
JavaScript is never evaluated. Component offsets, product metadata, label
exports, ten locale tables, CSS, reducer excerpts and resource declarations
are retained in `wireless-argb-current-evidence.json`.

The native module renders the 260 × 260 product artwork, power/automatic
detection/refresh controls, standby, Bluetooth/mobile sync, detection, missing
devices, overcurrent protection and LED-limit messages. The 3886 source also
has a DC-power-required message. Requests never fabricate hardware responses.
Without a device service the live page shows status unavailable; development
fixtures are confined to the explicit preview dialog.

The port editor preserves strip/fan choice, editable port names, LED steppers,
fan LED choices, up to four strip segments, bend redistribution, second/fourth
side constraint, removable additional segments, totals, Chroma-layout notices
and warning copy. The 3884 editor additionally permits multiple fans, adding
20 LEDs per fan, and sums all active ports against the 240-LED device limit.
It uses the source `minimumLedValue: 1`; its per-port maximum comes from a
device capability (the isolated fixture uses the source's 80-LED fan-capable
case). The 3886 source minimum is 4, its numeric editor maximum is
`max(detectedLedCount, 40)`, and its per-port warning threshold is 120 despite
the warning string naming 80. That disagreement is preserved, not normalized.

## Actual mounted-source limitations

3886 `TopViewComponent` includes
`!r && i && !u.type === k_.BLE_MOBIL && this.renderPorts(a)`.
`k_.BLE_MOBIL` is the number 3; strict comparison of a Boolean with 3 is always
false. The normal 3886 page therefore does not mount its port cards. Native
rendering preserves this. The preview contains an explicitly labeled switch
to inspect the defined but unreachable editor; that switch is never counted
as live-page coverage or evidence that the official page mounts it.

There is no discovery/power/connection/Chroma service adapter in this module.
The preview records requests and requires explicit state selection for their
results. It neither emits profile updates nor saves fixture observations.
`snapshot`/`restore` contain only port layout preferences. Detection counts,
active ports, power, warning, connection and the per-port notice dismissal
remain transient. Restore validates known ports and values, limits input size,
and assigns fresh stable local segment identities.

The parent stores the port-layout snapshot in `source_device_settings`, outside
lighting profiles. Both current roots' `loadActiveProfileSettings` load
`isPowerOn`, `isAutoDetectionEnable`, brightness, quick effects and lighting
timeouts from the selected profile; neither loads a ports field. Ports instead
flow through `portsReducer` and `ON_SET_PORT_VALUES` (3884 also sends
`portsConfig`). These exact paths are retained in evidence `persistence`.
The downloaded UI does not establish the service's on-disk port schema; this
audit makes no claim about that absent middleware storage implementation.

Native interaction uses retained Kit input/select state, semantic buttons,
and the framework dialog for Escape/focus restoration. The source SVG shapes
and source CSS artwork colors are statically extracted; rem-relative geometry
follows the source 16px reference scale. Runtime geometry, focus behavior and
pixel equality have not been measured because running the application/tests
is prohibited. The current native editor uses integer LEDs; 3886's historical
numeric widget technically accepts decimals despite LED-count semantics.

The latest source review moved distinct card/border/warning colors into
`wireless_argb/theme.rs`, and explicitly set the Roboto body and RazerF5 port
heading fonts. Exact original detection animations (50ms / 700ms) and hover
transitions (300ms), native tooltip timing, numeric decimal entry for 3886 and
rendered pixel geometry remain unverified/unimplemented fidelity gaps. The
static SVGs preserve paths, fills and viewports but do not prove animations.

Profile header behavior is separately traced in evidence `profile_bar`:
3884 root dispatches `setProfileDropdownState(active_view===TAB_LIGHTING)` on
mount and update, so its non-Lighting dropdown is disabled. 3886 has no such
root dispatch and starts `enableSwitchProfile:true`; its `isEnableProfileBar`
prop only disables the synchronization icon outside Lighting. Both keep the
profile bar mounted. Parent SourceWorkspace owns these conditions.

## Assets and integration

`assets/synapse/wireless-argb-manifest.json` records 30 source/output SHA-256
receipts: product art, fan/strip diagrams, detection art and source inline SVG
icons for both products. `prepare-wireless-argb.py` fetches only exact current
manifest URLs when missing, converts bitmap data with Pillow, and copies safe
SVG data. Main resource embedding is integrated by the root task.

Every resource receipt uses the shared `source`, `source_sha256`, and
`output_sha256` schema. For inline artwork, `source` is its current JavaScript
bundle and `source_sha256` hashes that bundle; `embedded_sha256` separately
hashes the statically decoded image data. The validator checks both hashes,
the extraction declaration and the output, so the global resource preparation
can validate inline assets without skipping their source provenance.

Public seam: `WirelessArgb::new(&Device, window, cx)`, `supports_page(pid,key)`,
`snapshot()`, `restore(saved,window,cx)`, `dismiss(window,cx)`,
`WirelessArgbChanged`, and `open_preview(window,cx)`.

Permitted verification:

```text
node tools/extract-wireless-argb.cjs --check
.work/resource-env/Scripts/python.exe tools/prepare-wireless-argb.py
python tools/validate-wireless-argb.py
rustfmt --edition 2024 src/features/wireless_argb.rs src/features/wireless_argb/state.rs src/features/wireless_argb/preview.rs
cargo check --locked --all-targets
```

No application, builds, tests, downloaded JavaScript, installers or DLLs ran.
