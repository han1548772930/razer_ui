# Aether 784 Lighting Preview Audit

The current product source is `.ref/devices/784/static/js/main.3094caa4.js`, SHA-256 `3751063a619bd163a7ac9d8bf238a5452154ecf20fbaf04fcaaee4275b65a1f2`. The product navigation registers `TAB_LIGHTING` at offset `4601197`; the page renderer at the nearby `aN` definition composes the `kd` Synapse Override widget, `Wd` brightness widget, and `Bd` effects widget.

Static source inspection confirms these controls and branches:

- Synapse Override (`kd`) is a title-row switch plus explanatory content.
- Brightness (`Wd`) is a title-row switch and a slider with min 0, max 100, step 1, and endpoint labels.
- Quick Effects (`Bd`) normally exposes Quick/Advanced mode tabs. Quick mode contains a product-supplied effect selector, a Chroma sync row and effect-specific controls. The effect-specific control depends on the selected effect and includes color/direction or other parameters.
- Advanced mode contains Chroma Studio profile availability, a profile selector, launch action, or an installation branch.
- The outer `aN` renderer disables the widgets when locked/offline, when Synapse Override is off, or when power is off. Locked/offline state also shows the source status overlay. Device selection is initialized from the connected IoT devices.

The local source-workspace page keeps this content in the registered `TAB_LIGHTING` body. Its test-support anchors cover the Override switch, brightness control, mode tabs, quick effect selector/sync row, and advanced profile/Studio controls. Quick and Advanced render as mutually exclusive tab bodies, matching the source mode switch. Since no service observation is connected, device-dependent switches/actions are disabled, selector catalogs have no entries, and values/status are explicitly unknown. No selected effect, profile, power state, online state, lock state, or successful device action is fabricated.

Remaining differences are the read-only service/device observations, the product effect and Chroma Studio profile catalogs and current selections, the active IoT device carousel, effect-specific parameter controls once an effect is observed, the real lock/offline overlay branch, and eventual DLL-backed writes. Those operations remain outside this read-only preview phase.

Validation: `cargo check --locked --all-targets` passed. The test-support anchor test is compile-only; it was not run. No application, downloaded JavaScript, test binary, or DLL was executed.
