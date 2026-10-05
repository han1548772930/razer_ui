# `multiDevicePairing` opener integration (2026-10-05)

The current Dashboard source is `.ref/applications/synapse/dashboard/static/js/7861.1b0e99a4.chunk.js` (SHA-256 `9fc4c16bfd646066882263192641c0ad7be5a0d3b38f4a03293deab94b7e43a6`). Static text windows at offsets `72757` and `116069` show the source URL builder and its device-box opener. The builder forwards these fields when present:

`containerId`, `displayMode="multiDevicePairing"`, `productId` and `pid`, `category`, `canPairTwoDevices`, `isProductivity`, `deviceName`, `serialNumber`, `lang`, and JSON `allMasters`.

The root branch is present only for the 30 products listed under `multiDevicePairing` in [display-mode-audit.json](display-mode-audit.json). The native opener now follows that membership table before creating a window. `PairingDevice::open_window_payload` preserves the audited metadata, and `Shell::open_product_pairing_window` accepts the source `containerId` alias and parses either an array or the source's JSON encoded `allMasters` before handing it to the product-root page. Missing optional fields remain absent; no values are inferred from a product id.

Evidence and implementation:

- [display-mode-audit.json](display-mode-audit.json) — generated product root membership and source offsets.
- [display-window-contract.md](display-window-contract.md) — URL and window policy contract.
- [src/shell/pairing_state.rs](../../src/shell/pairing_state.rs) — source opener payload projection.
- [src/shell.rs](../../src/shell.rs) — root membership guard and `allMasters` hand-off.

## Non-root HyperPolling control

The `179:hyperpolling-pairing` control listed for product 126 is a separate
case. The current product-179 bundle
`.ref/devices/179/static/js/main.4849f7ca.js` (SHA-256
`f5d6efe15f34167d61020469f151c3a0827f695cb35bc49cbdd32c23957fb874`, static
evidence window `4103119..4108441`) opens its `G` pairing component in an
in-page modal. When an existing binding is selected, its `x` helper opens the
service-returned `/synapse/...` device URL with `policy=3,tab_visible=1`.
Neither path reads `displayMode=multiDevicePairing`; product 126 therefore
must not be added to the audited root table or bypass the root guard.
