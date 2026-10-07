# 2636 calibration review — 2026-10-07

Current source only: `.ref/devices/2636/static/js/main.82d8a835.js`, SHA-256 `a5b49ece13b13202b65e97d565b77ca61d22c8c6e14f9ec560dcee68d1155f9a`, and `.ref/middleware/2636/main.f0ca95429464cd09cbdb.js`. Exact AST ranges, CSS and source asset digests are in `gamepad-2636-calibration-current-evidence.json`; regenerate/validate with `node tools/prepare-gamepad-2636-calibration.cjs [--check]`. The tool parses vendor JavaScript as data and converts original AVIF resources to PNG; it never runs vendor modules.

## Verified branches

- Current page mount is `qM → ZM → XM`, with `zM` stepper, `aM` idle tester, and `xM` direction/rotation canvas. This product has no separate trigger-calibration popup. Other products' trigger or thumbstick popup branches were not transplanted.
- Step 0 selects left/right. `ON_SET_CONTROLLER_CALIBRATION` immediately sets the UI's step to 1 for Start and 0 for Stop. This local transition is separate from device acceptance.
- Steps 1–4 direct right/down/left/up. The middleware's actual module 66118 enum and canvas transforms establish this order; a different shared enum's historical names must not override it.
- A valid directional step replaces its direction canvas with the original LB overlay. Step 5 retains the rotation canvas. Done/success appears only at observed step 6, or observed step 5 with `isStepValid` and three rotations from real axis samples. Rotation uses the source's radius threshold 62.05, signed angle accumulation and wraparound; no time-based progress is supplied.
- Step -1 is an observed calibration error, not a pre-start confirmation. The source dialog has Cancel/Recalibrate and no backdrop/keyboard submission. Base Dialog owns focus trapping; explicit choices restore focus. No invented error is displayed when no service data exists.
- Edition 0/128/129 images follow the current context map; unknown editions use the source-authorized edition-0 fallback. The original 1116 × 753 images render at 372 × 251 CSS units. `surface::css` preserves the project's rem scale. The artwork/tester region follows the current 850/650/400 media-query thresholds and .8/.6/.5 scales while preserving its layout box. Source SVG shapes and canvas background geometry are prepared as inert assets; only genuine observation positions draw a moving dot.

## Native changes and state boundary

`gamepad_products.rs` owns retained calibration state and invokes `gamepad_calibration.rs`. Start/Cancel/Done/Retry emit `CalibrationIntent` and retain the latest local intention. These intentions do not mutate the profile draft or claim a device save. The visible status identifies the local scope. There is no backend mutation subscriber in this implementation.

`CalibrationObservation` validates part IDs, step range and finite axes. Generation/sequence guards reject earlier sessions, duplicate/out-of-order observations and the wrong active stick. Cancel, Retry, page exit/profile restore and unavailable observations invalidate earlier state. Parent integration also invalidates calibration when the product workspace becomes inactive and forwards generation/intent/observations through ProductWorkspace → SourceWorkspace → Gamepad. There is still no live publisher or device mutation subscriber. Positions remain unknown until supplied; neither centered samples nor successful service states are seeded. Source axis animations are observation-driven; no decorative progress timer was introduced.

Middleware module 66118 reads raw axes via `rzDevice.getAnalogInputRawDataAll()`. Version-2 handler I derives movement validity, updates `CalibrationUserMovement` window storage on a 33 ms throttle and includes device mutation during `complete → processCalibrationData`. The native observation seam is ready to receive verified readings, but the complete vendor calibration handler must not be activated as a supposedly readonly adapter. No DLL, HID transport or service was executed here.

## Verification and remaining acceptance

- Static preparation/check covers current UI/MW AST receipts, SVG resources and all three original image editions. The group queue now records 2636 as partial UI implementation instead of the former disabled-button stub.
- Production controls and meaningful state regions expose GPUI `test-support`. Two compile-only tests exercise native button paths through local start/cancel/error/retry, stale-result rejection, valid-without-rotation gating and disconnect invalidation. Tests were not run.
- Parent agent runs the permitted `cargo check --locked --all-targets`; this report does not assert a result before that check.
- Live read/query wiring remains incomplete. The implemented viewport media scaling and product-contained error placement still require native acceptance, along with visual, focus, zoom and input verification. The repository prohibits application/test execution, so none is claimed. This page and product remain **not completed** in the acceptance inventory.
