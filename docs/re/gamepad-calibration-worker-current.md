# Current V3 trigger worker implementation

Source behavior is established by the independently parsed current 2676 and 2684 middleware, device chunks and current host receipts in `gamepad-calibration-middleware-current.md`. `gamepad-2676-calibration-device-live-source.json` and its 2684 counterpart retain exact module bytes, source hashes and UTF-16 boundaries. Their `transaction_receipt` additionally verifies the original getter uses **post-increment**: initial counter 0, returned sequence **0 through 30 repeatedly**. The previous prose interpretation as 1 through 31 was incorrect and has been corrected; the original recorded source expression was always `_getTransactionId(){return 31===this.transactionId&&(this.transactionId=0),this.transactionId++}`.

`crates/razer-service/src/runtime/controller_calibration.rs` owns a background operation that calls the shared calibration arithmetic and the actual Feature-report protocol. It is not a successful-write wrapper: it opens a uniquely observed `HidNode`, checks source vendor/product/interface and the actual descriptor's Feature Report 10 length 91, retains the same handle for ADC polling, five-sample acquisition, calibration set and exact readback, and emits actual progress/movement observations. `razer-hid` already holds an OS file lock for that handle's whole lifetime across workers. The existing transport verifies exact Report ID/length against the descriptor before I/O.

The current worker accepts source-proven wired physical products 2676/2677 for logical product 2676 and 2684/2685 for logical product 2684, interface 1 and vendor 5426. An unknown Windows interface requires the actual SetupAPI proof described below; other unresolved interfaces are rejected. Device identity and descriptor failures cannot become successful observations. Every Feature send/get is bracketed by cancellation and unique-current-node checks. The wait callback also observes cancellation and identity. The worker has **no invented overall calibration deadline**: current source allows calibration to remain active until canceled, completed or failed. OS/device operation blocking and each IPC request timeout remain distinct from calibration duration.

The operation loop schedules the next poll after the source 10 ms delay. When a hold is due it starts the source five-sample sequence before issuing another raw poll. It does not create successful steps from the UI timer. Press/release hold lengths come from the core's actual source 2,000/3,000 ms. Progress comes from the core's retained broadcast snapshot, separate from its post-stop reset, so step 12 retains the original validity at publication. Terminal success and calibration failure queue removal of `CalibrationUserMovement` **before** their terminal progress event. Cancel queues movement removal without fabricating a firmware error step.

The service response contract is:

```json
{
  "start": {
    "operation_id": "caller-owned-operation-id",
    "state": "running",
    "terminal": false,
    "part_id": 3
  },
  "running_poll": {
    "operation_id": "caller-owned-operation-id",
    "state": "running",
    "terminal": false,
    "observations": [
      {
        "kind": "progress",
        "payload": {
          "step": 10,
          "stepDesc": "Step_1_Trigger_Press",
          "isStepValid": false,
          "error": ""
        }
      }
    ]
  },
  "cancel_ack": {
    "operation_id": "caller-owned-operation-id",
    "state": "canceling",
    "terminal": false
  }
}
```

The above is contract notation, not device observations. `start` accepts an operation without implying its first ADC reads already succeeded. Its first progress event is queued after the two actual start queries. A nonterminal poll's state is `running` or `canceling`. Every terminal poll has `terminal:true` and exactly one state `completed`, `failed` or `canceled`; callers must apply its queued observations in order before consuming the terminal outcome. For an ordinary completed worker, `result` includes the actual node, logical product, part, state, transport metadata, `device_write_attempted`, `device_write_verified`, and `error`. Attempted means a SetRange send was attempted; verified is true only after complete source range validation, a physical set and an exact selected-range readback. Failure before adapter setup or a worker panic returns a root error and `device_write_verified:false`, without inventing a result object or device observation.

Other queued observation forms are `{"kind":"movement","payload":{"x":0,"y":0,"t":actual_source_percentage}}` and `{"kind":"remove_movement","key":"CalibrationUserMovement"}`. The top-level calibration error string matches source; detailed transport context remains in `result.error`. The observation queue is bounded and discards old movement values first when a consumer fails to drain it, consistent with movement being current window-storage state. Progress/error events are retained. A later retry requires finishing cleanup and collecting the previous terminal result first. Controller drop cancels and joins its worker so the HID handle and lock finish releasing before shutdown completion.

`runtime/windows/controller_calibration_identity.rs` supplies the Windows unknown-interface adapter. It leaves the original `HidNode.interface_number=-1` intact, locates one current SetupAPI interface by the same exact path, requires observed vendor/product, claim interface 1, Feature length 91, a valid actual ContainerId and a nonempty device instance ID, and captures those identity fields for repeated comparisons during the operation. Report 10 alone does not establish claim interface. The actual descriptor check remains a separate prerequisite after opening the node. Native runtime registers the helper through `controller_calibration_node_proof`; discovery must likewise use actual observed paths when selecting an unknown-interface node.

Current popup storage-removal semantics are independently literal-proven: PID2676 `tp` is at UTF-16 offsets 6802022..6806844 in its recorded current UI bundle; PID2684 `Rc` is at 529685..534458 in its current chunk. Their storage callback only updates when both the key matches and `e.newValue` is truthy. The 2676 literal is `if(e.key===Dt.CALIBRATION_USER_MOVEMENT&&e.newValue)try{const{t:t}=JSON.parse(e.newValue);if(!Number.isFinite(t))return;_(Math.min(100,Math.max(0,t)))}catch{}`; 2684 uses the equivalent `Br` key namespace and `c(...)` setter. A remove/null newValue is therefore ignored by the popup and preserves its previous marker value. It does not generate unavailable state, change UI generation, or reset t to100. Only the open/selected-trigger effect resets to100. The source Recalibrate action dispatches start without resetting the movement marker. Native host-key removal and popup marker preservation must remain separate.

Discovery/IPC/shell are now connected by `razer-discovery/src/controller_calibration.rs`, typed `ControllerTriggerCalibrationStart/Poll/Cancel`, the portable runtime field/dispatch and `razer-shell/src/shell/controller_calibration.rs`. The shell retains operation, stop-cleanup and failed-thread-start predecessors until teardown can be awaited on application quit. Ordinary discovery refreshes do not cancel an unchanged actual controller. Progress and movement flow into the mounted popup; acceptance and device completion remain distinct. Hardware/device/service interaction has not been executed for verification.

Focus is independently audited in `gamepad-calibration-focus-current-source.json`; host event producers are retained in `gamepad-calibration-focus-host-producers-current.json`. Modules21107/1107 query host visibility/minimization and selected frame identity. `razer-model::host_window_focus` preserves strict missing-field comparisons: initial and focus(false) query status plus selected tab, while focus(true) checks only selected tab. The shell connects the once-per-popup initial observation and BrowserWindow-equivalent activation events to retained HostTab ownership; Windows status uses the borrowed owning HWND with IsWindowVisible/IsIconic. GPUI presentation visibility and foreground-window identity are not used. Retry does not remount or reinitialize the helper. Local HostTab IDs are the internal frame identity, not a claim of querying a vendor BrowserView. Other platform status adapters, the thumbstick producer and runtime window/tab acceptance remain gaps.

This is implementation evidence only. No application, vendor JavaScript/native binding or real device operation was executed. Wireless products 2680/2681 still require their actual live peer identity and online association. Cross-owner host-storage publication, complete source window lifecycle and runtime acceptance remain unverified. Native internals beyond the existing portable HID implementation require installed IDA Pro/Hex-Rays evidence for new analysis.

2026-10-11 verification: `cargo check --locked --all-targets --offline` passed, with existing unused-item warnings. `cargo test --locked --offline -p razer-device controller_calibration --lib` passed all ten pure Rust/simulated transport tests, including initial-zero/wrap transaction behavior, busy/stale responses, truncated/rejected replies, product-specific range limits, stable holds, sample failures without write and strict readback/reset snapshots. The four focus receipt source spans in three current files passed independent SHA-256 and UTF-16 range comparison. These checks establish compilation and simulated source semantics, not device or visual runtime acceptance.

The subsequent host-focus integration passed the consolidated all-targets check. `cargo test --locked --offline -p razer-model host_window_focus --lib` passed four pure tests: missing-field strict comparisons, ordinary OS blur retaining selected-frame focus, hidden/minimized status-query branches, and the mounted popup's initial-false/true-then-false/remount lifecycle. Independently verified nine exact UTF-16 focus-consumer/host-producer spans across five current files against original SHA-256. No GUI, vendor JavaScript, DLL or device execution was performed.
