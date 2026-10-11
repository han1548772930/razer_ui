# 679 current floating Controller

Audit date: 2026-10-11. This page is independent current-source evidence and an implementation checkpoint; runtime acceptance remains absent. Static AST/text/resource preparation only. The webpack inline entry has no numeric module ID.

Main: `local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/js/main.194d8a7b.js`

SHA-256: `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20`

## Conditions and interaction

The `.toggle-controller` listener toggles only local open state and CSS `open`. Close uses the same toggle. It does not clear the selected keyboard key. The controller uses the original skeleton and 24 exact path/circle hit regions. Hypershift alone selects green/orange hover, dragging and preview colors; this palette has no live device input observation.

The drag area is fixed, horizontally centered with 30px side margins, and follows `.config-block` top/height. Panel width360, padding20, border1, radius5 and universal border-box produce a318px image width at the320×220 viewBox ratio. Height is measured after localized description layout. First opening places it left of `.keyboard-svg` and vertically centers it within the area. Hide/show retains its origin. Source clamp applies max(minX) then min(maxX), including negative origins when the area is smaller than the panel. Drawer opening shifts origin+200 only at x<=200; closing reverses only a recorded shift.

Panel mousedown accepts every browser mouse button; document motion updates/clamps its position, document exit suppresses panel motion, mouseup commits/clamps and removes drag classes. Dragging tints the area #0000001a. A shape mousedown stops propagation and stores `{data: structuredClone(button.mapping)}` under controller-button, shows preview and global grabbing cursor. No dragCustomize/originalButton is present for palette drags. Generator refreshes every slot actuationPoint with31867.fH.createDefaultActuation(), including current profile sensitivity rules.

Shape enter/leave controls the body tooltip. It is centered10px above the shape, moves below if above panel top, and clamps horizontally10px within the panel. Drag preview follows page pointer+10,+10 with original icon,40px height,padding10,radius5,14px bold #212121. Pointer release consumes an exact keyboard geometry hit before clearing drag state.

Keyboard layout padding: `{"kind": "fixed", "px": 25, "rule": {"path": "local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css", "sha256": "389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8", "char_range": [391828, 391876], "byte_range": [391888, 391936], "slice_sha256": "f42c81913a01ef6968ea945ad2068d00bdb065d6f9216b3c8595ba3682a0344d", "source": ".padding-for-wrist-rest-device{padding-top:25px}"}}`.

## Rust and submission connection

`keyboard_floating_controller.rs` owns independent retained open/position/drag state, deferred absolute surfaces, source hit geometry, tooltip and preview. `keyboard_products.rs` mounts it in Customize and registers actual native keyboard regions; toggle does not mutate selected_key. Current layout geometry is selected from observed artwork/layout data when available. Page unmount clears floating state; changing a profile keeps the mounted floating origin while cancelling pending pointer interaction.

The palette-to-key path uses `keyboard_controller_drop.rs`: source notRemapped and3342.z exclusions, both asymmetric duplicate checks (current layer and omitted layer), complete-list replacement keyed by inputID/inputType/isHyperShift. Success writes the local full mapping list, emits KeyboardProductChanged, and calls request_actuation_mapping→ON_SET_KEYMAPPING. This immediate source action has no Save confirmation. Submission responses use the existing generation/error state; no successful device operation is invented. Full drop caller/reducer spans are independently retained in `keyboard-controller-drop-current-source.json`.

## Explicit gaps and verification limits

The real service/device mapping adapter still reports its implementation gap; this page does not establish native writeback or successful runtime acceptance. Source MapKeyboard assignmentValue depends on the complete button-state producer and real systemKeyboardLayout; primary hyperShiftGroup is reconstructed for its specific gate, but the remaining dynamic assignment display/Windows remap branch is not claimed complete. Keyboard-to-key moving drags are not mounted by this palette work. 679 RM/DM special dial/media popovers remain an explicit gap: their source hover uses disabled DIAL_CLICK and DKM_KBMK_02 shapes plus assignmentGroup projection and independent positioning; no static card or fabricated active state is substituted. The renderer does not reproduce the source stale mouse-pressed ref after release or mismatched scroll cleanup target; native per-frame callbacks are reclaimed with their element. Tooltip300ms opacity transition, exact resize/layout300ms measurement scheduling and all host focus/cancellation paths need separate runtime/static lifecycle acceptance.

Pure Rust tests cover clamp order/narrow areas, conditional drawer shift reversal, and close/reopen retained origin. Parent task performs cargo check and selected pure tests; application, vendor JavaScript, DLLs/helpers and real device operations were not executed.

## Conditional curve selection evidence

Selected `alternate` using `{"isLowProfile": null, "defaultActuation": null}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960332, 7960482]` SHA-256`4104fc62525303b85dd9dd98a67ad2e85409996f0ac4c3227132ef87684bcdda`

```javascript
null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.isLowProfile&&null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.defaultActuation
```

consequent: UTF-8`[7960483, 7960513]` SHA-256`472ca95f15ad21facb104fc0fc489f85ae6a3c26bb4c79d0cbb46bc8ea9b83ef`

```javascript
_e.DeviceInfo.defaultActuation
```

alternate: UTF-8`[7960514, 7960625]` SHA-256`e575fd05af605c18c5ef768f16814138cd975d7d7f98af02a40df5957c5e4577`

```javascript
tn.r?[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]:[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"AnalogGenVersion": "analogV2"}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960514, 7960518]` SHA-256`07a9a9bcf321638f7eaf51eea9c797374b5ed4754829cbb89bfa0b317808417e`

```javascript
tn.r
```

consequent: UTF-8`[7960519, 7960573]` SHA-256`310272417c9ea3c1477f9dd60bd6994a725571a5143972672cf70952c8543a98`

```javascript
[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]
```

alternate: UTF-8`[7960574, 7960625]` SHA-256`6039e93ac5a97f9d59e1f0add4f5c8f88b5e215f91eb18e177b6309071ec1958`

```javascript
[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"isLowProfile": null, "defaultActuation": null}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960332, 7960482]` SHA-256`4104fc62525303b85dd9dd98a67ad2e85409996f0ac4c3227132ef87684bcdda`

```javascript
null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.isLowProfile&&null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.defaultActuation
```

consequent: UTF-8`[7960483, 7960513]` SHA-256`472ca95f15ad21facb104fc0fc489f85ae6a3c26bb4c79d0cbb46bc8ea9b83ef`

```javascript
_e.DeviceInfo.defaultActuation
```

alternate: UTF-8`[7960514, 7960625]` SHA-256`e575fd05af605c18c5ef768f16814138cd975d7d7f98af02a40df5957c5e4577`

```javascript
tn.r?[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]:[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"AnalogGenVersion": "analogV2"}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960514, 7960518]` SHA-256`07a9a9bcf321638f7eaf51eea9c797374b5ed4754829cbb89bfa0b317808417e`

```javascript
tn.r
```

consequent: UTF-8`[7960519, 7960573]` SHA-256`310272417c9ea3c1477f9dd60bd6994a725571a5143972672cf70952c8543a98`

```javascript
[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]
```

alternate: UTF-8`[7960574, 7960625]` SHA-256`6039e93ac5a97f9d59e1f0add4f5c8f88b5e215f91eb18e177b6309071ec1958`

```javascript
[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"isLowProfile": null, "defaultActuation": null}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960332, 7960482]` SHA-256`4104fc62525303b85dd9dd98a67ad2e85409996f0ac4c3227132ef87684bcdda`

```javascript
null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.isLowProfile&&null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.defaultActuation
```

consequent: UTF-8`[7960483, 7960513]` SHA-256`472ca95f15ad21facb104fc0fc489f85ae6a3c26bb4c79d0cbb46bc8ea9b83ef`

```javascript
_e.DeviceInfo.defaultActuation
```

alternate: UTF-8`[7960514, 7960625]` SHA-256`e575fd05af605c18c5ef768f16814138cd975d7d7f98af02a40df5957c5e4577`

```javascript
tn.r?[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]:[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"AnalogGenVersion": "analogV2"}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960514, 7960518]` SHA-256`07a9a9bcf321638f7eaf51eea9c797374b5ed4754829cbb89bfa0b317808417e`

```javascript
tn.r
```

consequent: UTF-8`[7960519, 7960573]` SHA-256`310272417c9ea3c1477f9dd60bd6994a725571a5143972672cf70952c8543a98`

```javascript
[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]
```

alternate: UTF-8`[7960574, 7960625]` SHA-256`6039e93ac5a97f9d59e1f0add4f5c8f88b5e215f91eb18e177b6309071ec1958`

```javascript
[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"isLowProfile": null, "defaultActuation": null}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960332, 7960482]` SHA-256`4104fc62525303b85dd9dd98a67ad2e85409996f0ac4c3227132ef87684bcdda`

```javascript
null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.isLowProfile&&null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.defaultActuation
```

consequent: UTF-8`[7960483, 7960513]` SHA-256`472ca95f15ad21facb104fc0fc489f85ae6a3c26bb4c79d0cbb46bc8ea9b83ef`

```javascript
_e.DeviceInfo.defaultActuation
```

alternate: UTF-8`[7960514, 7960625]` SHA-256`e575fd05af605c18c5ef768f16814138cd975d7d7f98af02a40df5957c5e4577`

```javascript
tn.r?[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]:[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"AnalogGenVersion": "analogV2"}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960514, 7960518]` SHA-256`07a9a9bcf321638f7eaf51eea9c797374b5ed4754829cbb89bfa0b317808417e`

```javascript
tn.r
```

consequent: UTF-8`[7960519, 7960573]` SHA-256`310272417c9ea3c1477f9dd60bd6994a725571a5143972672cf70952c8543a98`

```javascript
[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]
```

alternate: UTF-8`[7960574, 7960625]` SHA-256`6039e93ac5a97f9d59e1f0add4f5c8f88b5e215f91eb18e177b6309071ec1958`

```javascript
[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"isLowProfile": null, "defaultActuation": null}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960332, 7960482]` SHA-256`4104fc62525303b85dd9dd98a67ad2e85409996f0ac4c3227132ef87684bcdda`

```javascript
null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.isLowProfile&&null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.defaultActuation
```

consequent: UTF-8`[7960483, 7960513]` SHA-256`472ca95f15ad21facb104fc0fc489f85ae6a3c26bb4c79d0cbb46bc8ea9b83ef`

```javascript
_e.DeviceInfo.defaultActuation
```

alternate: UTF-8`[7960514, 7960625]` SHA-256`e575fd05af605c18c5ef768f16814138cd975d7d7f98af02a40df5957c5e4577`

```javascript
tn.r?[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]:[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"AnalogGenVersion": "analogV2"}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960514, 7960518]` SHA-256`07a9a9bcf321638f7eaf51eea9c797374b5ed4754829cbb89bfa0b317808417e`

```javascript
tn.r
```

consequent: UTF-8`[7960519, 7960573]` SHA-256`310272417c9ea3c1477f9dd60bd6994a725571a5143972672cf70952c8543a98`

```javascript
[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]
```

alternate: UTF-8`[7960574, 7960625]` SHA-256`6039e93ac5a97f9d59e1f0add4f5c8f88b5e215f91eb18e177b6309071ec1958`

```javascript
[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"isLowProfile": null, "defaultActuation": null}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960332, 7960482]` SHA-256`4104fc62525303b85dd9dd98a67ad2e85409996f0ac4c3227132ef87684bcdda`

```javascript
null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.isLowProfile&&null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.defaultActuation
```

consequent: UTF-8`[7960483, 7960513]` SHA-256`472ca95f15ad21facb104fc0fc489f85ae6a3c26bb4c79d0cbb46bc8ea9b83ef`

```javascript
_e.DeviceInfo.defaultActuation
```

alternate: UTF-8`[7960514, 7960625]` SHA-256`e575fd05af605c18c5ef768f16814138cd975d7d7f98af02a40df5957c5e4577`

```javascript
tn.r?[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]:[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"AnalogGenVersion": "analogV2"}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960514, 7960518]` SHA-256`07a9a9bcf321638f7eaf51eea9c797374b5ed4754829cbb89bfa0b317808417e`

```javascript
tn.r
```

consequent: UTF-8`[7960519, 7960573]` SHA-256`310272417c9ea3c1477f9dd60bd6994a725571a5143972672cf70952c8543a98`

```javascript
[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]
```

alternate: UTF-8`[7960574, 7960625]` SHA-256`6039e93ac5a97f9d59e1f0add4f5c8f88b5e215f91eb18e177b6309071ec1958`

```javascript
[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"isLowProfile": null, "defaultActuation": null}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960332, 7960482]` SHA-256`4104fc62525303b85dd9dd98a67ad2e85409996f0ac4c3227132ef87684bcdda`

```javascript
null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.isLowProfile&&null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.defaultActuation
```

consequent: UTF-8`[7960483, 7960513]` SHA-256`472ca95f15ad21facb104fc0fc489f85ae6a3c26bb4c79d0cbb46bc8ea9b83ef`

```javascript
_e.DeviceInfo.defaultActuation
```

alternate: UTF-8`[7960514, 7960625]` SHA-256`e575fd05af605c18c5ef768f16814138cd975d7d7f98af02a40df5957c5e4577`

```javascript
tn.r?[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]:[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"AnalogGenVersion": "analogV2"}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960514, 7960518]` SHA-256`07a9a9bcf321638f7eaf51eea9c797374b5ed4754829cbb89bfa0b317808417e`

```javascript
tn.r
```

consequent: UTF-8`[7960519, 7960573]` SHA-256`310272417c9ea3c1477f9dd60bd6994a725571a5143972672cf70952c8543a98`

```javascript
[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]
```

alternate: UTF-8`[7960574, 7960625]` SHA-256`6039e93ac5a97f9d59e1f0add4f5c8f88b5e215f91eb18e177b6309071ec1958`

```javascript
[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"isLowProfile": null, "defaultActuation": null}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960332, 7960482]` SHA-256`4104fc62525303b85dd9dd98a67ad2e85409996f0ac4c3227132ef87684bcdda`

```javascript
null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.isLowProfile&&null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.defaultActuation
```

consequent: UTF-8`[7960483, 7960513]` SHA-256`472ca95f15ad21facb104fc0fc489f85ae6a3c26bb4c79d0cbb46bc8ea9b83ef`

```javascript
_e.DeviceInfo.defaultActuation
```

alternate: UTF-8`[7960514, 7960625]` SHA-256`e575fd05af605c18c5ef768f16814138cd975d7d7f98af02a40df5957c5e4577`

```javascript
tn.r?[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]:[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"AnalogGenVersion": "analogV2"}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960514, 7960518]` SHA-256`07a9a9bcf321638f7eaf51eea9c797374b5ed4754829cbb89bfa0b317808417e`

```javascript
tn.r
```

consequent: UTF-8`[7960519, 7960573]` SHA-256`310272417c9ea3c1477f9dd60bd6994a725571a5143972672cf70952c8543a98`

```javascript
[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]
```

alternate: UTF-8`[7960574, 7960625]` SHA-256`6039e93ac5a97f9d59e1f0add4f5c8f88b5e215f91eb18e177b6309071ec1958`

```javascript
[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"isLowProfile": null, "defaultActuation": null}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960332, 7960482]` SHA-256`4104fc62525303b85dd9dd98a67ad2e85409996f0ac4c3227132ef87684bcdda`

```javascript
null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.isLowProfile&&null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.defaultActuation
```

consequent: UTF-8`[7960483, 7960513]` SHA-256`472ca95f15ad21facb104fc0fc489f85ae6a3c26bb4c79d0cbb46bc8ea9b83ef`

```javascript
_e.DeviceInfo.defaultActuation
```

alternate: UTF-8`[7960514, 7960625]` SHA-256`e575fd05af605c18c5ef768f16814138cd975d7d7f98af02a40df5957c5e4577`

```javascript
tn.r?[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]:[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"AnalogGenVersion": "analogV2"}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960514, 7960518]` SHA-256`07a9a9bcf321638f7eaf51eea9c797374b5ed4754829cbb89bfa0b317808417e`

```javascript
tn.r
```

consequent: UTF-8`[7960519, 7960573]` SHA-256`310272417c9ea3c1477f9dd60bd6994a725571a5143972672cf70952c8543a98`

```javascript
[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]
```

alternate: UTF-8`[7960574, 7960625]` SHA-256`6039e93ac5a97f9d59e1f0add4f5c8f88b5e215f91eb18e177b6309071ec1958`

```javascript
[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"isLowProfile": null, "defaultActuation": null}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960332, 7960482]` SHA-256`4104fc62525303b85dd9dd98a67ad2e85409996f0ac4c3227132ef87684bcdda`

```javascript
null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.isLowProfile&&null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.defaultActuation
```

consequent: UTF-8`[7960483, 7960513]` SHA-256`472ca95f15ad21facb104fc0fc489f85ae6a3c26bb4c79d0cbb46bc8ea9b83ef`

```javascript
_e.DeviceInfo.defaultActuation
```

alternate: UTF-8`[7960514, 7960625]` SHA-256`e575fd05af605c18c5ef768f16814138cd975d7d7f98af02a40df5957c5e4577`

```javascript
tn.r?[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]:[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

Selected `alternate` using `{"AnalogGenVersion": "analogV2"}` from `assets/synapse/keyboard-products/679.json` SHA-256`1f45b82bbcb35190d8b243631462fc1553ef0f3d25a1429f86a33c034b2b3a95`. Both original branches remain retained.

condition: UTF-8`[7960514, 7960518]` SHA-256`07a9a9bcf321638f7eaf51eea9c797374b5ed4754829cbb89bfa0b317808417e`

```javascript
tn.r
```

consequent: UTF-8`[7960519, 7960573]` SHA-256`310272417c9ea3c1477f9dd60bd6994a725571a5143972672cf70952c8543a98`

```javascript
[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]
```

alternate: UTF-8`[7960574, 7960625]` SHA-256`6039e93ac5a97f9d59e1f0add4f5c8f88b5e215f91eb18e177b6309071ec1958`

```javascript
[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

## Complete floating functions, table, aliases and root

### `bP`

UTF-8`[7920516, 7920629]`, UTF-16`[7119901, 7120014]`, SHA-256`22a8142c2b18a831df5485a10b772592df836aebcc6410f4e449536ca9cfea62`

```javascript
{isDragging:()=>FP,setIsDragging(e){FP=e},setData(e,t){BP.set(e,t)},getData:e=>BP.get(e),delete(e){BP.delete(e)}}
```

### `VM`

UTF-8`[7960629, 7972290]`, UTF-16`[7160014, 7171675]`, SHA-256`9098042da1b7b17cb45b5df813b0f2725f6ff881a86682b68a2b1098c96a5bf0`

```javascript
[{id:"trigger-right",name:"RIGHTTRIGGER",shape:{type:"path",data:{transform:"translate(-216.61 -0.5)",d:"M484.7,24.8c-7.1-2-15.2-3.1-22.6-4.1l0,0l0,0l0,0l-0.7-0.1c-6.9-1-15.6-2.1-23.8-2.1   c-1,0-2,0-3,0.1c3.4-2.5,6.4-4.5,9.2-6.2c6.4-3.8,11.7-5.6,16-5.6c0.5,0,0.8,0,1.2,0.1l0,0l0,0c4.5,0.4,6.2,2,10.6,6.4l0,0   C474.5,15.8,478.4,19.7,484.7,24.8z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!0},analogSensitivityList:{analogSensitivityAssignment:wM},analogSensitivityType:"STANDARD",controllerAssignment:"RIGHTTRIGGER"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"trigger-left",name:"LEFTTRIGGER",shape:{type:"path",data:{transform:"translate(-216.61 -0.5)",d:"M267.7,24.8c6.3-5.1,10.2-8.9,13-11.6l0,0c4.5-4.4,6.1-6,10.6-6.4l0,0l0,0   c0.4,0,0.7-0.1,1.2-0.1c4.3,0,9.6,1.8,16,5.6c2.8,1.7,5.8,3.7,9.2,6.2c-1,0-2-0.1-3-0.1c-8.2,0-16.8,1.2-23.8,2.1l-0.7,0.1l0,0l0,0   l0,0C283,21.7,274.8,22.8,267.7,24.8z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!0},analogSensitivityList:{analogSensitivityAssignment:wM},analogSensitivityType:"STANDARD",controllerAssignment:"LEFTTRIGGER"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"right-bumper",name:"RIGHTBUMPER",shape:{type:"path",data:{transform:"translate(0.209 9)",d:"M276.1,23.8l-67.5-8c1-0.4,1.9-1.1,2.5-2.1c1.6-2.1,3.9-3.5,6.5-4.1   c3.2-0.8,6.5-1.2,9.7-1.1c5.9,0.1,11.8,0.7,17.6,1.6l0.5,0.1c6.6,0.9,14.1,1.9,19.9,4c2.6,0.9,5.1,2.2,7.2,3.9   C274.3,19.6,275.5,21.6,276.1,23.8z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!1},analogSensitivityType:"STANDARD",controllerAssignment:"RIGHTBUMPER"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"left-bumper",name:"LEFTBUMPER",shape:{type:"path",data:{transform:"translate(38 17.534)",d:"M6,15.3l67.5-8c-1-0.4-1.9-1.1-2.5-2.1c-1.6-2.1-3.9-3.5-6.5-4.1c-3.2-0.8-6.5-1.2-9.7-1.1   c-5.9,0.1-11.8,0.7-17.6,1.6l-0.5,0.1c-6.6,0.9-14.1,1.9-19.9,4c-2.6,0.9-5.1,2.2-7.2,3.9C7.9,11.1,6.6,13,6,15.3z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!1},analogSensitivityType:"STANDARD",controllerAssignment:"LEFTBUMPER"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"xbox-view",name:"SELECT_BUTTON",shape:{type:"circle",data:{transform:"translate(-0.501 9)",cx:142,cy:67.4,r:10}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!1},analogSensitivityType:"STANDARD",controllerAssignment:"VIEWBUTTON"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"xbox-menu",name:"MENUBUTTON",shape:{type:"circle",data:{transform:"translate(0.503 9)",cx:178,cy:67,r:10}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!1},analogSensitivityType:"STANDARD",controllerAssignment:"MENUBUTTON"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"rightjoystick-up",name:"RIGHT_JS_UP",shape:{type:"path",data:{transform:"translate(-0.5 9.5)",d:"M219.1,100.4l-7.1,7.1c-7.5-7.7-19.7-7.9-27.5-0.5c-0.2,0.2-0.3,0.3-0.5,0.5l-7.1-7.1   c0,0,0.1-0.1,0.1-0.1l0.2-0.2c11.6-11.5,30.3-11.4,41.7,0.2C219,100.3,219,100.3,219.1,100.4z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!0},analogSensitivityList:{analogSensitivityAssignment:wM},analogSensitivityType:"STANDARD",controllerAssignment:"RIGHT_JS_UP"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"rightjoystick-right",name:"RIGHT_JS_RIGHT",shape:{type:"path",data:{transform:"translate(-0.5 9.5)",d:"M227.5,121c0,7.9-3.2,15.5-8.9,21.1l-7.1-7.1c7.7-7.4,7.9-19.6,0.6-27.4l7.1-7.1   C224.5,106,227.5,113.3,227.5,121z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!0},analogSensitivityList:{analogSensitivityAssignment:wM},analogSensitivityType:"STANDARD",controllerAssignment:"RIGHT_JS_RIGHT"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"rightjoystick-down",name:"RIGHT_JS_DOWN",shape:{type:"path",data:{transform:"translate(-0.5 9.5)",d:"M218.5,142.2c-11.4,11.1-29.6,11.1-41,0l7.1-7.1c7.5,7.1,19.3,7.1,26.8,0   L218.5,142.2z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!0},analogSensitivityList:{analogSensitivityAssignment:wM},analogSensitivityType:"STANDARD",controllerAssignment:"RIGHT_JS_DOWN"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"rightjoystick-left",name:"RIGHT_JS_LEFT",shape:{type:"path",data:{transform:"translate(-0.5 9.5)",d:"M184.5,135l-7.1,7.1c-11.6-11.3-11.8-29.9-0.6-41.6l7.1,7.1   C176.5,115.3,176.8,127.5,184.5,135z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!0},analogSensitivityList:{analogSensitivityAssignment:wM},analogSensitivityType:"STANDARD",controllerAssignment:"RIGHT_JS_LEFT"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"rightjoystick-press",name:"RIGHT_JS_PRESS",shape:{type:"circle",data:{transform:"translate(-0.5 9.5)",cx:198,cy:121.4,r:19.5}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!1},analogSensitivityType:"STANDARD",controllerAssignment:"RIGHT_JS_PRESS"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"xbox-a",name:"A_BUTTON",shape:{type:"circle",data:{transform:"translate(0 9)",cx:253,cy:87,r:10}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!1},analogSensitivityType:"STANDARD",controllerAssignment:"A_BUTTON"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"xbox-b",name:"B_BUTTON",shape:{type:"circle",data:{transform:"translate(-0.501 9)",cx:273,cy:67,r:10}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!1},analogSensitivityType:"STANDARD",controllerAssignment:"B_BUTTON"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"xbox-x",name:"X_BUTTON",shape:{type:"circle",data:{transform:"translate(-0.501 9)",cx:233,cy:67,r:10}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!1},analogSensitivityType:"STANDARD",controllerAssignment:"X_BUTTON"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"xbox-y",name:"Y_BUTTON",shape:{type:"circle",data:{transform:"translate(0 9)",cx:253,cy:47,r:10}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!1},analogSensitivityType:"STANDARD",controllerAssignment:"Y_BUTTON"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"leftjoystick-up",name:"LEFT_JS_UP",shape:{type:"path",data:{transform:"translate(-0.001 9.5)",d:"M88.1,46.4L81,53.5c-7.4-7.7-19.7-7.9-27.5-0.5c-0.2,0.2-0.3,0.3-0.5,0.5l-7.1-7.1   c0,0,0.1-0.1,0.1-0.1l0.2-0.2c11.6-11.5,30.2-11.4,41.7,0.2C88,46.3,88,46.3,88.1,46.4z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!0},analogSensitivityList:{analogSensitivityAssignment:wM},analogSensitivityType:"STANDARD",controllerAssignment:"LEFT_JS_UP"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"leftjoystick-right",name:"LEFT_JS_RIGHT",shape:{type:"path",data:{transform:"translate(-0.001 9.5)",d:"M96.5,67c0,7.9-3.2,15.5-8.9,21.1L80.5,81c7.7-7.4,7.9-19.6,0.6-27.4l7.1-7.1   C93.5,52,96.5,59.3,96.5,67z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!0},analogSensitivityList:{analogSensitivityAssignment:wM},analogSensitivityType:"STANDARD",controllerAssignment:"LEFT_JS_RIGHT"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"leftjoystick-down",name:"LEFT_JS_DOWN",shape:{type:"path",data:{transform:"translate(-0.001 9.5)",d:"M87.5,88.2c-11.4,11.1-29.6,11.1-41,0l7.1-7.1c7.5,7.1,19.3,7.1,26.8,0L87.5,88.2z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!0},analogSensitivityList:{analogSensitivityAssignment:wM},analogSensitivityType:"STANDARD",controllerAssignment:"LEFT_JS_DOWN"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"leftjoystick-left",name:"LEFT_JS_LEFT",shape:{type:"path",data:{transform:"translate(-0.001 9.5)",d:"M53.5,81l-7.1,7.1c-11.6-11.3-11.8-29.9-0.6-41.6l7.1,7.1   C45.5,61.3,45.8,73.5,53.5,81z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!0},analogSensitivityList:{analogSensitivityAssignment:wM},analogSensitivityType:"STANDARD",controllerAssignment:"LEFT_JS_LEFT"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"leftjoystick-press",name:"LEFT_JS_PRESS",shape:{type:"circle",data:{transform:"translate(-0.001 9.5)",cx:67,cy:67.4,r:19.5}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!1},analogSensitivityType:"STANDARD",controllerAssignment:"LEFT_JS_PRESS"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"dpad-up",name:"DPADUP",shape:{type:"path",data:{transform:"translate(85 100)",d:"M36.5,5c2.2,0,4.4,0.3,6.5,0.8V20H30V5.8C32.1,5.3,34.3,5,36.5,5z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!1},analogSensitivityType:"STANDARD",controllerAssignment:"DPADUP"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"dpad-right",name:"DPADRIGHT",shape:{type:"path",data:{transform:"translate(85 100)",d:"M62,30.5c0,2.2-0.3,4.4-0.8,6.5H47V24h14.2C61.7,26.1,62,28.3,62,30.5z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!1},analogSensitivityType:"STANDARD",controllerAssignment:"DPADRIGHT"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"dpad-down",name:"DPADDOWN",shape:{type:"path",data:{transform:"translate(85 100)",d:"M30,41h13v14.2c-4.3,1.1-8.7,1.1-13,0V41z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!1},analogSensitivityType:"STANDARD",controllerAssignment:"DPADDOWN"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]},{id:"dpad-left",name:"DPADLEFT",shape:{type:"path",data:{transform:"translate(85 100)",d:"M11.8,24H26v13H11.8C10.7,32.7,10.7,28.3,11.8,24z"}},mapping:[{actuationPoint:{0:1.5,1:1.5},controllerGroup:{controllerMode:{isAnalog:!1},analogSensitivityType:"STANDARD",controllerAssignment:"DPADLEFT"},outputType:"controllerGroup"},{actuationPoint:{0:1.5,1:1.5},isDefault:!1,isHyperShift:!1,outputType:"defaultGroup"}]}]
```

### `$M`

UTF-8`[7990754, 7991659]`, UTF-16`[7190139, 7191044]`, SHA-256`3e90374dd9b64f56b9228192a042179637c0128e3529fb1c377be71dfe150b4b`

```javascript
e=>{let t=e.children;const a=(0,g.useRef)(null),n=(0,g.useState)({top:0,left:0,right:100,bottom:100}),o=(0,pn.A)(n,2),i=o[0],s=o[1],r=()=>{const e=document.querySelector(".config-block"),t=null===e||void 0===e?void 0:e.getBoundingClientRect();if(t&&a.current){const e=30;a.current.style.top=t.top+"px",a.current.style.height=t.height+"px",a.current.style.width=window.innerWidth-2*e+"px";const n=a.current.getBoundingClientRect();s(n)}};return(0,g.useEffect)(()=>{const e=setTimeout(r,300);return()=>clearTimeout(e)},[]),(0,g.useEffect)(()=>{r();const e=document.querySelector(".body-wrapper.scrollable");return null===e||void 0===e||e.addEventListener("scroll",r,!0),window.addEventListener("resize",r,!0),()=>{window.removeEventListener("resize",r),window.removeEventListener("scroll",r)}},[]),(0,g.useLayoutEffect)(()=>{r()},[]),(0,un.jsx)("div",{ref:a,className:"controller-drag-area",children:t(i)})}
```

### `eh`

UTF-8`[7991669, 7991715]`, UTF-16`[7191054, 7191100]`, SHA-256`71640771a73ac925487b46c5dd12c8c592cd66da377370da1e043fa444e685ad`

```javascript
a.p+"static/media/icon_draggable.be674683.svg"
```

### `th`

UTF-8`[7991719, 7992058]`, UTF-16`[7191104, 7191443]`, SHA-256`7cf208250d166848cd6e1f912fb2bdc3af44227b2a45286f2d0d2dff3ca43c85`

```javascript
(0,g.forwardRef)((e,t)=>{let a=e.id,n=(e.icon,e.text),o=e.isHyperShiftOn;return(0,G.createPortal)((0,un.jsxs)("div",{className:"controller-button-drag-image ".concat(o?"hypershift-on":""),ref:t,children:[(0,un.jsx)("div",{className:"icon ".concat(a)}),(0,un.jsx)("div",{className:"text",children:(0,Xt.getTextItem)(n)})]}),document.body)})
```

### `ih`

UTF-8`[7992576, 7994525]`, UTF-16`[7191961, 7193910]`, SHA-256`803dfd2bbf12ec3eb2f0e757a3bf720c11ca8c565c80a87b2138e5c08ad81f38`

```javascript
e=>{let t=e.button;const a=(0,H.useSelector)(e=>e.customizeReducer.isHyperShiftOn),n=(0,g.useState)(!1),o=(0,pn.A)(n,2),i=o[0],s=o[1],r=(0,g.useState)(!1),E=(0,pn.A)(r,2),_=E[0],T=E[1],c=(0,g.useRef)(null),I=(0,g.useRef)(null),A=(0,g.useRef)(null),l=(0,g.useRef)(!1);(0,g.useEffect)(()=>{const e=e=>{l.current&&(e=>{A.current&&(A.current.style.top=e.pageY+"px",A.current.style.left=e.pageX+"px")})(e)},t=e=>{l.current&&(e.target.contains(c.current)||T(!1),bP.setIsDragging(!1),(e=>{e.stopPropagation(),s(!1),T(!1),document.body.classList.remove("dragging-something")})(e))};return document.addEventListener("mousemove",e),document.addEventListener("mouseup",t),()=>{document.removeEventListener("mousemove",e),document.removeEventListener("mouseup",t)}},[]);const O=(0,U.A)({ref:c,onMouseDown:e=>{e.preventDefault(),e.stopPropagation(),l.current=!0,(e=>{e.stopPropagation(),bP.setIsDragging(!0),bP.setData("controller-button",{data:structuredClone(t.mapping)}),s(!0),document.body.classList.add("dragging-something")})(e)},onMouseEnter:()=>{T(!0)},onMouseLeave:e=>{T(!1),l.current},id:t.id,className:"controller-button ".concat(i?"dragging":""," ").concat(a?"hypershift-on":"")},t.shape.data);return(0,g.useLayoutEffect)(()=>{if(!c.current||!_)return;const e=10,t=document.querySelector(".controller").getBoundingClientRect(),a=c.current.getBoundingClientRect(),n=I.current.getBoundingClientRect();let o=a.top-n.height-e;o<t.top&&(o=a.bottom+e);let i=a.left+a.width/2-n.width/2;i<t.left+e?i=t.left+e:i+n.width>t.right-e&&(i=t.right-e-n.width),I.current.style.top=o+"px",I.current.style.left=i+"px"},[_]),(0,un.jsxs)(un.Fragment,{children:["path"===t.shape.type?(0,un.jsx)("path",(0,U.A)({},O)):null,"circle"===t.shape.type?(0,un.jsx)("circle",(0,U.A)({},O)):null,_&&(0,un.jsx)(oh,{ref:I,className:"controller-button-key-tip",show:!0,type:(0,un.jsx)(Sn.A,{text:be.AC$}),name:t.name}),i&&(0,un.jsx)(ah,{ref:A,id:t.id,text:t.name,isHyperShiftOn:a})]})}
```

### `sh`

UTF-8`[7994535, 7994586]`, UTF-16`[7193920, 7193971]`, SHA-256`a9dd4383c71fe343283cb4e623e205d6b1e580aeea86d4b4204e298472e06d6e`

```javascript
a.p+"static/media/controller-skeleton.72fd3181.svg"
```

### `rh`

UTF-8`[7994590, 7998095]`, UTF-16`[7193975, 7197480]`, SHA-256`b076defb7d10725a3e6746be7c9773c8a23be1ebcb2d8e28fdaac1e49316646f`

```javascript
e=>{let t=e.bound;const a=(0,g.useState)(!1),n=(0,pn.A)(a,2),o=n[0],i=n[1],s=(0,g.useRef)(null),r=(0,g.useRef)(!1),E=(0,g.useRef)(!0),_=(0,g.useRef)({top:0,left:0}),T=(0,g.useRef)({atStart:{top:0,left:0},dragging:{top:0,left:0},atEnd:{top:0,left:0}}),c=(0,g.useRef)(!1),I=(0,g.useRef)(!1),A=(0,H.useSelector)(e=>e.customizeReducer.isPanelOpen),l=(0,g.useRef)(!1),O=()=>{s.current.classList.remove("dragging","drag-start"),s.current.parentNode.classList.remove("dragging","drag-start")},d=e=>{s.current.classList.add(e),s.current.parentNode.classList.add(e)},S=(e,a)=>{const n=A?200:0,o=s.current.offsetWidth,i=s.current.offsetHeight,r=t.right-t.left-o,E=t.bottom-t.top-i;let _=Math.max(e,n);_=Math.min(_,r);let T=Math.max(a,0);return T=Math.min(T,E),s.current.style.top=T+"px",s.current.style.left=_+"px",{top:T,left:_}};return(0,g.useLayoutEffect)(()=>{if(!o)return;if(E.current){E.current=!1;const e=(e=>{const a=document.querySelector(".keyboard-svg").getBoundingClientRect(),n=t.top+t.height/2-e.height/2;console.log("bound height",t.height),console.log("bound top",t.top);const o=a.left-e.width;return{top:n-t.top,left:o-t.left}})(s.current.getBoundingClientRect());_.current.left=e.left,_.current.top=e.top}A!==I.current&&(A?_.current.left<=200&&(_.current.left+=200,c.current=!0):c.current&&(_.current.left-=200,c.current=!1),I.current=A);const e=S(_.current.left,_.current.top);_.current.top=e.top,_.current.left=e.left;const a=e=>{l.current=!0},n=e=>{l.current=!1},i=e=>{r.current&&(e=>{if(l.current)return!0;const t=e.pageX-T.current.atStart.left,a=e.pageY-T.current.atStart.top,n=_.current;S(t+n.left,a+n.top),O(),d("dragging")})(e)},u=e=>{r.current&&(r.current=!1,(e=>{const t=e.pageX-T.current.atStart.left,a=e.pageY-T.current.atStart.top;_.current.top+=a,_.current.left+=t;const n=S(_.current.left,_.current.top);_.current.top=n.top,_.current.left=n.left,O()})(e))};return document.addEventListener("mouseleave",a),document.addEventListener("mouseenter",n),document.addEventListener("mousemove",i),document.addEventListener("mouseup",u),()=>{document.removeEventListener("mouseleave",a),document.removeEventListener("mouseenter",n),document.removeEventListener("mousemove",i),document.removeEventListener("mouseup",u)}},[o,t,A]),(0,g.useEffect)(()=>{const e=e=>{i(t=>{const a=!t;return a?e.target.classList.add("open"):e.target.classList.remove("open"),a})},t=document.querySelector(".toggle-controller");return t.addEventListener("click",e),()=>t.removeEventListener("click",e)},[o]),o?(0,un.jsxs)("div",{className:"controller",ref:s,onMouseDown:e=>{r.current=!0,(e=>{T.current.atStart.left=e.pageX,T.current.atStart.top=e.pageY,O(),d("drag-start")})(e)},onClick:()=>r.current=!1,children:[(0,un.jsx)("div",{className:"controller-content",children:(0,un.jsxs)("div",{className:"img",children:[(0,un.jsx)("img",{src:sh}),(0,un.jsx)("svg",{xmlns:"http://www.w3.org/2000/svg",version:"1.1",id:"Controller",x:"0px",y:"0px",viewBox:"0 0 320 220",children:YM().map(e=>(0,un.jsx)(ih,{button:e},e.id))})]})}),(0,un.jsx)("img",{src:eh,onMouseDown:e=>e.preventDefault(),alt:"",className:"controller-draggable-icon"}),(0,un.jsx)("div",{className:"controller-close-button",onClick:()=>{const e=document.querySelector(".toggle-controller");i(t=>{const a=!t;return a?e.classList.add("open"):e.classList.remove("open"),a})},onMouseDown:e=>e.stopPropagation(),children:(0,un.jsx)("img",{src:eE,alt:""})}),(0,un.jsx)("div",{className:"controller-feat-desc",children:(0,un.jsx)(Sn.A,{text:be.arE})})]}):null}
```

### `YM`

UTF-8`[7972294, 7972411]`, UTF-16`[7171679, 7171796]`, SHA-256`ca932b077f1125485f05f2cd82c1a3710b01baab5e2e1c976876b9077a9b8bd8`

```javascript
()=>(VM.forEach(e=>{e.mapping.forEach(e=>{e.actuationPoint&&(e.actuationPoint=He.fH.createDefaultActuation())})}),VM)
```

### `oh`

UTF-8`[7992570, 7992572]`, UTF-16`[7191955, 7191957]`, SHA-256`32fcec9c2b3834eb5df31ae6ab2f82e189a7f2fb48eceb2accc24d73180d56df`

```javascript
nh
```

### `ah`

UTF-8`[7992111, 7992113]`, UTF-16`[7191496, 7191498]`, SHA-256`6bde0b830d8bd56dea61c5c1cb648c7ffca6ffce2923ad1db9f29079cac947e0`

```javascript
th
```

### `eE`

UTF-8`[7590990, 7591032]`, UTF-16`[6790379, 6790421]`, SHA-256`98a65fbd0ceb8052447abb8fb196b6e1e61b26adf7ca5dfe431cede570e50056`

```javascript
a.p+"static/media/icon_close.55fe41f1.svg"
```

### `nh`

UTF-8`[7992117, 7992542]`, UTF-16`[7191502, 7191927]`, SHA-256`2cabc049b029836127e0cae57571c9a8f1c870fb687d0c77b50112f4612407de`

```javascript
(0,g.forwardRef)((e,t)=>{e.index;let a=e.type,n=e.name,o=e.className,i=e.show;return(0,G.createPortal)((0,un.jsx)("div",{className:o||"",ref:t,children:(0,un.jsx)("div",{className:"key-tip ".concat(i?"show":""),children:(0,un.jsxs)("div",{className:"warning-content-wrapper",children:[(0,un.jsx)("div",{className:"type",children:a}),(0,un.jsx)("div",{className:"name ",children:(0,Xt.getTextItem)(n)})]})})}),document.body)})
```

### `wM`

UTF-8`[7960332, 7960625]`, UTF-16`[7159717, 7160010]`, SHA-256`c7e813411c0dbc74d3203456dd9322028085a945a5e397218859680121f92364`

```javascript
null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.isLowProfile&&null!==_e.DeviceInfo&&void 0!==_e.DeviceInfo&&_e.DeviceInfo.defaultActuation?_e.DeviceInfo.defaultActuation:tn.r?[{x:1.5,y:0},{x:2.2,y:85},{x:2.9,y:170},{x:3.6,y:255}]:[{x:.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]
```

### `FP`

UTF-8`[7920493, 7920495]`, UTF-16`[7119878, 7119880]`, SHA-256`42e969e9dee66e64259c0abc3bdc13e4f8957c47f99a9d2993a51282d5e5ad1d`

```javascript
!1
```

### `BP`

UTF-8`[7920505, 7920512]`, UTF-16`[7119890, 7119897]`, SHA-256`21c02aa09aebb9ffc045f64add51bd447a6d1d18a0e48f6a462893ef39bd9053`

```javascript
new Map
```

### `Sh`

UTF-8`[8001598, 8011370]`, UTF-16`[7200983, 7210755]`, SHA-256`4ae192476deac26619dabd84f34eee566d3131f2f314fd1e769d45138a9f2c8d`

```javascript
class Sh extends g.Component{constructor(e){var t;super(e),t=this,this.handleFocusVolumeButton=()=>{if(this.props.activeButton)if(dh.includes(this.props.activeButton.buttonKey)){const e=document.querySelector("circle#DIAL_CLICK.svg-key");this.state.specialTipShowed||(this.setPositionForMediaKeys({current:e},"mediaVolume"),this.setState({specialTipShowed:!0}),this.multimediaRef={current:e})}else this.state.specialTipShowed&&this.setState({specialTipShowed:!1})},this.handleResize=()=>{let e="";this.currentKey&&(this.state.specialTipShowed?e="mediaVolume":this.state.specialTipMediaShowed&&(e="pauseandplay"),e&&this.hoverSpecial(this.multimediaRef,e))},this.setCtx=e=>{this.ctx=e},this.hideTip=e=>{this.setState({tipShowed:!1})},this.setTipDom=e=>{this.tipDom=e},this.setDom=e=>{this.canvasDom=e,this.rect=this.canvasDom.getBoundingClientRect()},this.clickBtn=async(e,t,a)=>{if(!(ve.IOh.includes(e)||["MEDIA_PREV","MEDIA_PAUSE_PLAY","MEDIA_NEXT"].includes(e)||"macro"===this.props.displayMode&&["DIAL_LEFT_ROTATION","DIAL_CLICK","DIAL_RIGHT_ROTATION","mediaVolume","pauseandplay"].includes(e))&&(void 0===this.props.activeButton||this.props.activeButton.buttonKey!==e)){if(this.props.isMappingChanged&&this.props.activeButton)return this.buttonKey=e,this.refToSet=a.current,void this.props.displaySaveAlert(this.updateActiveButton,a.current);this.props.updateActiveButton(e),this.props.setKey(a.current)}},this.updateActiveButton=()=>{this.props.updateActiveButton(this.buttonKey)},this.hoverBtn=function(e,a){let n,o=arguments.length>2&&void 0!==arguments[2]&&arguments[2],i=t.props.buttonList[e];if("mediaVolume"===i.buttonKey||"pauseandplay"===i.buttonKey)return console.log("mediavolume"),void t.hoverSpecial(a,i.buttonKey);switch(i.inputType){case"AnalogInput":if(ve.IOh.includes(i.buttonKey))n=i;else{let e=i.inputID;n=t.props.mappingList.find(t=>t.inputID===e)}break;case"MouseInput":let e=i.MouseInput;n=t.props.mappingList.find(t=>t.inputID===e);break;case"DKMInput":let a=i.DKMInput;n=t.props.mappingList.find(e=>e.inputID===a)}t.currentKey=a.current,"WINDOWS_SHORTCUT"===i.assignment?i.defaultRequireSynapse=!0:i.defaultRequireSynapse=!1,setTimeout(()=>t.setState({hoverIndex:e,tipShowed:!0,activeHoverKey:i,hoverKeyMapping:n,specialChildTipShowed:o}),0)},this.leaveBtn=()=>{setTimeout(()=>this.setState({hoverIndex:-1,tipShowed:!1,specialChildTipShowed:!1}),0)},this.toggleButtonPanel=()=>{this.props.isMappingChanged?this.props.displaySaveAlert(this.updateToggleButtonPanel):this.updateToggleButtonPanel()},this.updateToggleButtonPanel=()=>{this.props.isPanelOpen&&this.props.fromPanel&&(this.props.setActiveButton(""),this.props.toggleMappingPanel(!1)),this.props.toggleButtonPanel(!this.props.isPanelOpen)},this.toggleHyperShift=async()=>{this.props.isMappingChanged?this.props.displaySaveAlert(this.updateHypeShift):this.updateHypeShift()},this.updateHypeShift=async()=>{let e=(0,Pr.updatedProductWithLayoutAndEdition)(this.props.layoutId);this.props.loadButtonInfo(e.groupList[0].group.buttonList),await this.props.toggleHyperShift(!this.props.isHyperShiftOn),this.props.setActiveButton(""),this.props.updateMappingChanged(!1),this.props.toggleMappingPanel(!1)},this.setMultiDial=e=>{this.multiDial=e},this.setSpecialTipDom=e=>{this.specialTipDom=e},this.setSpecialTipMediaDom=e=>{this.specialTipMediaDom=e},this.hoverSpecial=(e,t)=>{this.multimediaRef=e,this.setPositionForMediaKeys(e,t)},this.setPositionForMediaKeys=(e,t)=>{var a,n,o,i;let s=e.current.getBoundingClientRect();const r=document.querySelector(".drawer-open"),E=document.querySelector(".main-container"),_=document.querySelector(".body-wrapper"),T=null!==(a=null===(n=document.querySelector(".customize #body-wrapper"))||void 0===n?void 0:n.scrollLeft)&&void 0!==a?a:0,c=null!==(o=null===(i=document.querySelector(".customize #body-wrapper"))||void 0===i?void 0:i.scrollTop)&&void 0!==o?o:0;var I,A,l;"mediaVolume"===t?(this.specialTipDom.current.style.left=s.left-(null!==(I=null===r||void 0===r||null===(A=r.getBoundingClientRect())||void 0===A?void 0:A.width)&&void 0!==I?I:0)-15+T+"px",this.specialTipDom.current.style.top=s.top-(E.getBoundingClientRect().height-_.getBoundingClientRect().height+20)+c+"px","macro"===this.props.displayMode&&(this.specialTipDom.current.style.left="694.5px",this.specialTipDom.current.style.top="106px"),this.setState({specialTipShowed:!0})):(this.specialTipMediaDom.current.style.left=s.left-10-(null!==(l=null===r||void 0===r?void 0:r.clientWidth)&&void 0!==l?l:0)-15+T+"px",this.specialTipMediaDom.current.style.top=s.top-(E.getBoundingClientRect().height-_.getBoundingClientRect().height+25)+c+"px","macro"===this.props.displayMode&&(this.specialTipMediaDom.current.style.left="654px",this.specialTipMediaDom.current.style.top="110px"),this.setState({specialTipMediaShowed:!0}))},this.hideSpecialTip=()=>{const e=this.props,t=e.isMappingOpen,a=e.activeButton;(!this.state.specialChildTipShowed&&!t||a&&!["DIAL_RIGHT_ROTATION","DIAL_CLICK","DIAL_LEFT_ROTATION"].includes(a.buttonKey))&&this.setState({specialTipShowed:!1})},this.hideSpecialMediaTip=()=>{this.state.specialChildTipShowed||this.setState({specialTipMediaShowed:!1})},this.state={hyperShiftOn:!1,hoverIndex:-1,activeButton:-1,tipShowed:!1,activeHoverKey:{},hoverKeyMapping:{},specialTipShowed:!1,specialChildTipShowed:!1,specialTipMediaShowed:!1,wrist:!0},this.multiDialImg={VOLUME_DOWN:{active:vM,inactive:GM},MUTE_VOLUME:{active:yM,inactive:gM},VOLUME_UP:{active:HM,inactive:fM}},this.mediaButtonImg={PLAY_PAUSE:{active:mM,inactive:mM},NEXT_TRACK:{active:PM,inactive:PM},PREVIOUS_TRACK:{active:MM,inactive:MM}},this.configDom=f().createRef(),this.leftCol=f().createRef(),this.rightCol=f().createRef(),this.multimediaRef=f().createRef(),this.hyperTips="Configure Razer Hypershift shortcuts here. Enjoy an extra set of buttons using the Raer Hypershift key.",this.leftCol=0,this.rightCol=0,this.mappings=[],this.filteredMappings=[],this.tipDom=null,this.specialTipDom=null,this.specialTipMediaDom=null,this.currentKey=null,this.refToSet=null}componentDidUpdate(e){var t,a;this.state.wrist||e.wristrestConnected===this.props.wristrestConnected||this.setState({wrist:!0}),(null===(t=this.props.activeButton)||void 0===t?void 0:t.buttonKey)!==(null===(a=e.activeButton)||void 0===a?void 0:a.buttonKey)&&this.handleFocusVolumeButton()}componentDidMount(){this.setState({wrist:this.props.wristrestConnected}),window.addEventListener("resize",this.handleResize),document.getElementById("body-wrapper").addEventListener("scroll",this.handleResize)}componentWillUnmount(){window.removeEventListener("resize",this.handleResize)}render(){const e=128===this.props.editionId?UM:hM,t=128===this.props.editionId?LM:pM,a=this.state,n=a.activeButton,o=a.specialTipShowed,i=a.specialTipMediaShowed,s=this.setMultiDial;let r="wrist-rest";return r+=this.props.wristrestConnected?" show":" hide",(0,un.jsxs)(g.Fragment,{children:[(0,un.jsx)(JM,{hoverKeyMapping:this.state.hoverKeyMapping?this.state.hoverKeyMapping:this.state.activeHoverKey,activeKey:this.state.activeHoverKey,setTipDom:this.setTipDom,show:this.state.tipShowed,currentKey:this.currentKey,isHyperShiftOn:this.props.isHyperShiftOn,category:_e.DeviceInfo.category}),(0,un.jsx)(NM,{multiDialImg:this.multiDialImg,dialImg:e,buttonList:this.props.buttonList,setTipDom:this.setSpecialTipDom,show:o,hideTip:this.hideSpecialTip,clickBtn:this.clickBtn,activeBtn:n,hyperShiftOn:this.props.isHyperShiftOn,hoverBtn:(e,t)=>this.hoverBtn(e,t,!0),leaveBtn:this.leaveBtn}),(0,un.jsx)(CM,{multiDialImg:this.mediaButtonImg,dialImg:t,buttonList:this.props.buttonList,setTipDom:this.setSpecialTipMediaDom,show:i,hideTip:this.hideSpecialMediaTip,clickBtn:this.clickBtn,activeBtn:n,hyperShiftOn:this.props.isHyperShiftOn,hoverBtn:(e,t)=>this.hoverBtn(e,t,!0),leaveBtn:this.leaveBtn,shape:"rect",height:"23",width:"78"}),"macro"===this.props.displayMode?null:(0,un.jsx)($M,{children:e=>(0,un.jsx)(rh,{bound:e})}),(0,un.jsxs)("div",{className:"config-wrapper dot-bg",style:{position:"relative"},children:[(0,un.jsxs)("div",{className:"config-block padding-for-wrist-rest-device",ref:this.configDom,children:[(0,un.jsx)(OM,{buttonList:this.props.buttonList,clickBtn:this.clickBtn,activeBtn:n,hoverBtn:this.hoverBtn,leaveBtn:this.leaveBtn,hyperShiftOn:this.props.isHyperShiftOn,setMultiDial:s}),(0,un.jsx)(bM,{pid:_e.DeviceInfo.productId,editionId:this.props.editionId,layoutId:this.props.layoutId,alt:kn.onLang(this.props.lang).productName,category:_e.DeviceInfo.category}),this.state.wrist?(0,un.jsx)("div",{className:r}):null]}),(0,un.jsx)("div",{className:"dim-corner"})]}),(0,un.jsxs)("div",{className:"flex config-row mt20 mb10",children:["macro"===this.props.displayMode?null:(0,un.jsxs)(un.Fragment,{children:[(0,un.jsx)("div",{className:"toggle-controller hover-btn"}),(0,un.jsx)("div",{className:"toggle-drawer ".concat(this.props.isPanelOpen?"drawer-open":""," hover-btn"),onClick:this.toggleButtonPanel})]}),(0,un.jsxs)("div",{className:"hyper-wrapper ".concat(this.props.isHyperShiftOn?"hyper-on":""),onClick:this.toggleHyperShift,children:[(0,un.jsx)("div",{className:"text standard",children:(0,un.jsx)(Sn.A,{text:be.iWJ})}),(0,un.jsx)("div",{className:"text hypershift",children:(0,un.jsx)(Sn.A,{text:be.LUv})})]}),(0,un.jsx)("div",{children:(0,un.jsxs)("div",{style:{position:"relative"},children:[(0,un.jsx)("div",{className:"help"}),(0,un.jsx)("div",{style:{width:362},className:"tip hypershift-mode-tip",children:(0,un.jsx)(Sn.A,{text:be.Dq6})})]})})]}),"macro"!==this.props.displayMode?(0,un.jsxs)(ZE,{children:[(0,un.jsxs)(e_,{direction:"left",children:[(0,un.jsx)(EP,{}),(0,un.jsx)(AP,{deviceType:"analog"})]}),(0,un.jsxs)(e_,{direction:"right",children:[(0,un.jsx)(ZM,{mappingList:this.props.mappingList}),(0,un.jsx)(Oh,{})]})]}):null]})}}
```

### `60481 complete module (analog generation predicate)`

UTF-8`[6033820, 6034051]`, UTF-16`[5235230, 5235461]`, SHA-256`ba3151fa85a03eb73462f7a9b53d811a254140beb11a831a1decb12756faae34`

```javascript
(e,t,a)=>{"use strict";a.d(t,{T:()=>r,r:()=>s});var n,o,i=a(78193);const s="analogV1"===(null===(n=i.DeviceInfo)||void 0===n?void 0:n.AnalogGenVersion),r="analogV2"===(null===(o=i.DeviceInfo)||void 0===o?void 0:o.AnalogGenVersion)}
```

## Complete relevant current CSS

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/css/main.1c5a651a.css` SHA-256`389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8`

UTF-8`[3439, 3465]` SHA-256`ffba1b8a5431e5f65bc2be1b5b6e7c655d1cfa615e701f0e19dffe39a7f34700`

```css
div{box-sizing:border-box}
```

UTF-8`[11866, 12025]` SHA-256`fc31886a3d085a618db8938cc32bb0881c425bf9600147ec5295a93a71d150b0`

```css
::-webkit-scrollbar-thumb{background:#ffffff4d;background-clip:padding-box;border:1px solid #0000;border-radius:4px;box-sizing:border-box;height:6px;width:6px}
```

UTF-8`[14403, 14566]` SHA-256`951461200725c7be06dd1c979824e81639e4ec2a9ddd70bc7df47dafc04315db`

```css
.cloud-switch{background-color:#222;box-sizing:border-box;display:flex;flex-shrink:0;height:56px;height:49px;justify-content:center;padding:10px 0 20px;width:100%}
```

UTF-8`[30979, 31401]` SHA-256`be1829ad984ab0adcbeeecff53ad9c331c19e65a79b2a53990cee7de2927af77`

```css
[tooltip]:before{background-color:#000;border:1px solid #5d5d5d;box-sizing:border-box;color:#ccc;content:attr(tooltip);display:block;font-size:14px;height:auto;line-height:16px;opacity:0;padding:8px 10px;pointer-events:none;position:absolute;right:0;text-align:left;top:calc(100% + 5px);transition:visibility 0s,opacity .3s linear;visibility:hidden;white-space:nowrap;width:auto;will-change:visibility,opacity;z-index:100}
```

UTF-8`[31453, 31835]` SHA-256`b5c14a5d1c1e0743c5e30d714fc9934bae36007256cb33f803d2fc9608046f54`

```css
[data-tooltip]:before{background-color:#000;border:1px solid #5d5d5d;box-sizing:border-box;color:#ccc;content:attr(data-tooltip);display:block;font-size:14px;height:auto;line-height:16px;opacity:0;padding:8px 10px;pointer-events:none;position:relative;text-align:left;top:60px;transition:visibility 0s,opacity .3s linear;visibility:hidden;width:300px;will-change:visibility,opacity}
```

UTF-8`[43774, 44166]` SHA-256`05d9013b3ba3794a3d7469176d907b2796c403352adb0f3ba23fd2c46ef97cfc`

```css
.slider-more::-webkit-slider-thumb{-webkit-appearance:none;appearance:none;background-image:url(../../static/media/path.0086a00e.svg);background-position:50%;background-repeat:no-repeat;background-size:cover;border-radius:3px;box-sizing:border-box;height:20px;-webkit-transition:transform .2s,background .3s;transition:transform .2s,background .3s;width:14px;will-change:transform,background}
```

UTF-8`[44539, 44812]` SHA-256`5d7a1e3ebb17bed80e8097ba7de8b313c724e2ec8c693167ab074f467d7b0ceb`

```css
.slider::-webkit-slider-thumb{-webkit-appearance:none;appearance:none;background:#44d62c;border-radius:8px;box-sizing:border-box;height:16px;-webkit-transition:transform .2s,background .3s;transition:transform .2s,background .3s;width:16px;will-change:transform,background}
```

UTF-8`[53845, 54096]` SHA-256`0b82a34a1da63ebf601b0922db9bb79e2eac6e74f5d75e1fc62981146aa3e639`

```css
input[name=profile]{background-color:#111!important;border:1px solid #44d62c;box-sizing:border-box;color:#ccc;display:none;font-family:Roboto,sans-serif;font-size:14px;height:27px;line-height:17px;padding:5px;position:absolute;width:230px;z-index:100}
```

UTF-8`[73815, 74222]` SHA-256`8202bb06e1e35314a83e6c626333fba4c11c7428385aede2e9b1259693575a57`

```css
.widget .button{align-items:center;background:#707070;border:1px solid #0000004d;border-radius:3px;box-sizing:border-box;display:flex;flex:none;flex-direction:row;flex-grow:0;font-family:Roboto;font-size:12px;font-style:normal;font-weight:400;gap:10px;height:27px;justify-content:center;line-height:14px;margin-bottom:15px;order:2;padding:7px 16px 6px;text-align:center;text-transform:uppercase;width:173px}
```

UTF-8`[83320, 83534]` SHA-256`aa7e0650d33c7be3ecc36bca851988cf2f83e0531f0b3c73858df6e19e7ee357`

```css
.check-box{background-color:#0000;border:1px solid #737373;border-radius:2.4px;box-sizing:border-box;display:inline-block;height:20px;margin-right:10px;position:relative;transition:border-color .3s ease;width:20px}
```

UTF-8`[83635, 83883]` SHA-256`c07057fcc6e5c8b2fe6ea0484f7975054d24ecb276a2fbb1a0e9657c7174c42b`

```css
.check-box:after,.check-box:before{background-color:#0000004d;background-color:#111;border-radius:2.4px;box-sizing:border-box;content:"";display:inline-block;height:0;position:absolute;transform-origin:left top;transition:opacity .5 ease;width:3px}
```

UTF-8`[85804, 86026]` SHA-256`8a8b456ea681ce8b813d7080e2c06ff9dab1afe53c004832499fc9f8af61c8e0`

```css
[type=radio]:checked+label:before,[type=radio]:not(:checked)+label:before{background:#0000;border:1px solid #737373;border-radius:100%;box-sizing:border-box;content:"";height:20px;left:0;position:absolute;top:0;width:20px}
```

UTF-8`[111798, 111824]` SHA-256`4848e0db1588d450a3ae246e0e2151119893944d6fd07eb9ed4470ed0d52a443`

```css
.key-tip{padding:8px 10px}
```

UTF-8`[111824, 111850]` SHA-256`ffaa005839ab831cb3282ebb6b16807253c29fb164d3425c03785a7822ff9a32`

```css
.key-tip.show{z-index:999}
```

UTF-8`[111850, 111984]` SHA-256`7ec824c5540c62afeb0a557cc2660c44d28ad99e28912f4a22e1e52767b6ba7e`

```css
.key-tip .name{overflow:hidden!important;overflow-wrap:break-word;text-overflow:ellipsis;text-transform:capitalize;white-space:nowrap}
```

UTF-8`[112085, 112267]` SHA-256`63323612f30731529f4f6c1c2adcc500d749984d018bd9b8b34d9a6a8b7221c3`

```css
.key-config .stepper,.modes-area .stepper,.stepper{border:1px solid #5d5d5d;box-sizing:border-box;height:27px;position:relative;transition:opacity .2s;width:60px;will-change:opacity}
```

UTF-8`[112267, 112494]` SHA-256`70f5f23a925ad166136fbdd3828d8980e8027f9790840b6eb50a9c9bd94e8a9d`

```css
.key-config .stepper,.modes-area .stepper input,.stepper{background-color:#111;border:none;box-sizing:border-box;color:#ccc;font-size:14px;height:25px;left:0;line-height:17px;padding:5px 18px 5px 5px;text-align:left;width:58px}
```

UTF-8`[113492, 113925]` SHA-256`7e8316e7272a01ad9f171d97316c79c3c01fc4be78ecfcb732d7a4b1b01c6aba`

```css
.save-alert .button{align-items:center;background:#44d62c;border:1px solid #0000004d;border-radius:3px;box-sizing:border-box;color:#222;display:flex;flex:none;flex-direction:row;flex-grow:0;font-family:Roboto;font-size:12px;font-style:normal;font-weight:400;gap:10px;height:27px;justify-content:center;justify-self:center;line-height:14px;order:5;padding:7px 16px 6px;text-align:center;text-transform:uppercase;width:173px;z-index:5}
```

UTF-8`[115020, 115056]` SHA-256`997c66c0c5b8d84b821e92b481119abb1cfb69b70f87a6a9325452ff23859b9d`

```css
.customize .key-tip{padding:initial}
```

UTF-8`[153197, 153390]` SHA-256`10e45435991e9c7afebfa1e226d209a47b37960213cfea827d69cbba108f97d5`

```css
.dashboard .box-item{background-color:#111;border:2px solid #0000;border-radius:5px;box-sizing:border-box;cursor:grab;flex-direction:column;left:0;padding:10px 20px 9px;position:absolute;top:0}
```

UTF-8`[158838, 158992]` SHA-256`fabd293d1534c1ffa3a11c238fd683be0890a23c56549eb4864421746776a2da`

```css
.dashboard .box-no-device{border:2px dashed #5d5d5d;border-radius:5px;box-sizing:border-box;flex-direction:column;padding:10px 20px 9px;position:absolute}
```

UTF-8`[160022, 160173]` SHA-256`6f913b737ec78e2f66731032229417dedb795fa4dd8416a94e304da6938c3aed`

```css
.dashboard .box-flip .box-flip-inner .box-flip-front{align-items:center;background-color:#111;border:2px solid #111;box-sizing:border-box;padding:25px}
```

UTF-8`[172860, 173281]` SHA-256`bc3fed2480d9a1d037794d07913a95ce45711f794bbb6095cf06a21b6da6a707`

```css
.app-row .app-tip{background-color:#000;border:1px solid #5d5d5d;box-sizing:border-box;color:#ccc;display:block;font-size:12px;height:auto;left:50px;line-height:16px;max-width:270px;opacity:0;padding:8px 10px;pointer-events:none;position:absolute;text-align:left;top:calc(100% - 5px);transition:visibility 0s,opacity .3s linear;visibility:hidden;white-space:pre-wrap;width:auto;will-change:visibility,opacity;z-index:100}
```

UTF-8`[190492, 190678]` SHA-256`9eaa253da214c37cf206650fb81d4a4339a17fae79bfcf38a07943c62d92aab9`

```css
.effect-wrapper .effects-area .stepper{border:1px solid #5d5d5d;box-sizing:border-box;height:27px;margin-top:10px;position:relative;transition:opacity .2s;width:60px;will-change:opacity}
```

UTF-8`[190678, 190886]` SHA-256`f09981c78cf765050ac8e511196600045b46329432c0b3a89ca31f83663824d0`

```css
.effect-wrapper .effects-area .stepper input{background-color:#111;border:none;box-sizing:border-box;color:#ccc;font-size:14px;height:25px;line-height:17px;padding:5px 18px 5px 5px;text-align:left;width:58px}
```

UTF-8`[194851, 195091]` SHA-256`23cfd9170f73559b28d3f200b1c9001e8a7388a66744966e3b3bd174e7b98399`

```css
.static-lighting-active{border:2px solid #fff;border-radius:5px;box-shadow:0 0 2px 1px rgba(0,0,0,.702),inset 0 0 2px 1px rgba(0,0,0,.702);box-sizing:border-box;display:block;height:11px;margin:3.75px;position:absolute;width:11px;z-index:1}
```

UTF-8`[195181, 195224]` SHA-256`c6423fbe4c66399c5f40ecdb8e33264a894114406d001f3f7fd10dfbab5ae058`

```css
#lightEffects canvas{box-sizing:border-box}
```

UTF-8`[196055, 196158]` SHA-256`4b2610cdc13b7adc513b85188c5a9bb0f869835e896684cd6767484ae54a0ab6`

```css
.picked-row .picked{border:1px solid #fff;box-sizing:border-box;height:24px;margin-top:10px;width:24px}
```

UTF-8`[196158, 196286]` SHA-256`605cc1b82e819fa95a3b72d6bbe08df22e13a5d4caab109186623b995d5ff736`

```css
.picked-row .to-black{border:1px solid #0000004d;box-sizing:border-box;height:16px;margin-left:10px;margin-top:14px;width:130px}
```

UTF-8`[196502, 196640]` SHA-256`0240fdedb8c34f6f479d6dd44d9960c19cf038bf938d98804e2d882e77d498ec`

```css
.picked-row .bright-ness:after,.picked-row .bright-ness:before{box-sizing:border-box;content:"";display:block;position:absolute;width:8px}
```

UTF-8`[196767, 196968]` SHA-256`d2fd0fc5970a6da7c7fbaa8934464fc93f9351211bdef1b5de2914bc172d9fa3`

```css
.input-row input{background-color:#111;border:1px solid #5d5d5d;box-sizing:border-box;color:#ccc;font-size:14px;height:26px;line-height:19px;margin-right:5px;text-align:center;text-transform:uppercase}
```

UTF-8`[199620, 199896]` SHA-256`4437dae9ea991037eb40fb82274dd3610e7a6ed94a4fdff36385a033cc6be1aa`

```css
.preset.selected:before,.s3-options .preset.selected:before{border:2px solid #fff;border-radius:5px;box-shadow:0 0 2px 1px #000000b3,inset 0 0 2px 1px #000000b3;box-sizing:border-box;content:"";display:block;height:10px;left:4px;position:absolute;top:4px;width:10px;z-index:1}
```

UTF-8`[202289, 202518]` SHA-256`4c06a25ae5870675098685b76fb5cf24e2aea49d264c391320fd9a96d9fd2874`

```css
input.stage-input{background-color:#111;border:1px solid #5d5d5d;box-sizing:border-box;color:#ccc;font-size:14px;height:26px;line-height:19px;margin-right:5px;text-align:center;transition:border .3s;width:60px;will-change:border}
```

UTF-8`[216346, 216909]` SHA-256`ca41af28b122fbca8d2c0a72eb41d53cb434dee0e2e406d6d7f9084d0b87a2de`

```css
.analog-tutorial-modal-v2 .buttons-block button{align-items:center;background:#707070;border:1px solid #0000004d;border-radius:3px;box-sizing:border-box;color:#fff;cursor:pointer;display:flex;flex-direction:row;font-family:Roboto;font-size:12px;font-style:normal;font-weight:400;gap:10px;height:27px;justify-content:center;line-height:14px;min-width:90px;opacity:1;padding:7px 16px 6px;text-align:center;text-transform:uppercase;transition:background-color .3s,opacity .3s;-webkit-user-select:none;user-select:none;width:90px;will-change:background-color,opacity}
```

UTF-8`[240787, 241018]` SHA-256`476ec2914d965738dd4eb8502a3cf368c61556ba75dee049dd0960b50dedcd9a`

```css
.maptext .text-area{background-color:#111;border:1px solid #5d5d5d;box-sizing:border-box;color:#ccc;font-family:Roboto;font-size:14px;height:96px;line-height:17px;min-width:210px;overflow-y:auto;padding:5px;resize:none;width:210px}
```

UTF-8`[288024, 288264]` SHA-256`0f4a01c47fbe6ffd56bac2f0d03969f2ee69ff5db9999d4bc504be3a9c54e7b2`

```css
.carousel--item{align-items:center;box-sizing:border-box;cursor:pointer;display:flex;flex:none;flex-direction:column;height:212px;justify-content:center;padding:0 30px;position:relative;-webkit-user-select:none;user-select:none;width:248px}
```

UTF-8`[312151, 312327]` SHA-256`a32e0cf88c28fbc64f244c496a0b0d26e4dd312707c0cb07f0930385977e65cd`

```css
.iot-device-list .device-list-title .mac-tooltip-wrapper .tooltip-wrapper{box-sizing:border-box;left:0;padding-top:5px;position:absolute;top:100%;visibility:hidden;width:305px}
```

UTF-8`[314139, 314369]` SHA-256`b87d163c6b78f6b388902a279181ce29368c0c49801c6ded895c1ee841fb9b96`

```css
.iot-device-input,.iot-skeleton,.skeleton-loading-layout{align-items:center;background-color:#111;border-radius:5px;box-sizing:border-box;cursor:pointer;display:flex;flex:1 1;height:40px;padding:10px;position:relative;width:375px}
```

UTF-8`[334529, 334633]` SHA-256`6bc18ade0e1f18cd45f860c1648c8af33b496102b45050c4bd6940275d96ddc0`

```css
.MonitoringDashboard_popupContent__8o7ZR{box-sizing:border-box;height:100%;overflow-y:auto;padding:24px}
```

UTF-8`[335651, 335916]` SHA-256`862058e973f3e27635c5220325dda1fa2985bc08b494b84dfcdc3a5045fd1361`

```css
.MonitoringDashboard_sectionGroup__sPvdj{background:#111;border-radius:5px;box-sizing:border-box;height:-webkit-fit-content;height:fit-content;min-height:-webkit-min-content;min-height:min-content;padding:15px;text-align:left;transition:opacity .3s ease;width:100%}
```

UTF-8`[337496, 337660]` SHA-256`de25a1a139d08669fe09971d50c86b70b7d7a62f2d3fc18397a2f92ea3f8c378`

```css
.MonitoringDashboard_arrowIcon__n2xJg{align-items:center;align-self:stretch;background:none;border:none;box-sizing:border-box;color:#fff;display:flex;padding:0 4px}
```

UTF-8`[353395, 353645]` SHA-256`0c133b9413d0a26073b9b023aff41e5185bc6a2649b2fecefa7bf2149c80e3cc`

```css
input[name=keymap]{background-color:#111!important;border:1px solid #44d62c;box-sizing:border-box;color:#ccc;display:none;font-family:Roboto,sans-serif;font-size:14px;height:27px;line-height:17px;padding:5px;position:absolute;width:230px;z-index:100}
```

UTF-8`[385921, 386335]` SHA-256`c841cdb29bd09498928369acbd376c85367a9b35ee1f814b9040cd03709a9a38`

```css
.optionButtonGroup_option-button__C62Cv{-webkit-appearance:none;appearance:none;background:#0000;border:1px solid #5d5d5d;border-radius:3px;box-sizing:border-box;color:#ffffffe6;cursor:pointer;font-family:Roboto;font-size:12px;font-weight:400;height:27px;letter-spacing:0;line-height:14px;margin:0;min-width:90px;outline:none;padding:7px 16px 6px;text-align:center;text-transform:uppercase;transition:all .2s ease}
```

UTF-8`[390686, 390789]` SHA-256`c18ac70dfa6f636eca83219c24c68d35fc59339d6593c679b4b85f0a159f5fed`

```css
.combined-key-list-wrapper{align-items:center;box-sizing:border-box;display:flex;flex-direction:column}
```

UTF-8`[391232, 391302]` SHA-256`bf034ec2c1b9ee8b2b0c094aa04e42312e587c9abbddbbaf6a336e0b89ee3497`

```css
.combined-key-item{box-sizing:border-box!important;margin:0!important}
```

UTF-8`[391888, 391936]` SHA-256`f42c81913a01ef6968ea945ad2068d00bdb065d6f9216b3c8595ba3682a0344d`

```css
.padding-for-wrist-rest-device{padding-top:25px}
```

UTF-8`[394727, 394860]` SHA-256`2fe8a0d97f081143c8ed28bffa95cccad68c1df431f5de1ebaa5e4b67b2af91d`

```css
.controller-drag-area{height:100%;left:50%;pointer-events:none;position:fixed;top:0;transform:translateX(-50%);width:100%;z-index:10}
```

UTF-8`[394860, 394912]` SHA-256`d375f3a8fce4ee4c9958ccff4109b1e2ad10f25cbdb02d0a471f5758aa76f7e3`

```css
.controller-drag-area.dragging{background:#0000001a}
```

UTF-8`[394912, 395059]` SHA-256`75b673c40a0c8d87cf4d49445dc966b4a3e2bce6210e2d104184b10d2532677d`

```css
.controller{background:#111;border:1px solid #5d5d5d;border-radius:5px;left:0;padding:20px;pointer-events:auto;position:absolute;top:0;width:360px}
```

UTF-8`[395059, 395149]` SHA-256`ee13499e34f1cd2263894b2094eb6a3a9ae47ba9861b59d16a18932d690922d8`

```css
.controller-draggable-icon{left:50%;position:absolute;top:10px;transform:translateX(-50%)}
```

UTF-8`[395149, 395327]` SHA-256`6001fbf9d56d0b97e5d40e85ca445fc336852b359730dea6d3d0ab06fba1cc11`

```css
.controller-close-button{align-items:center;border-top-right-radius:4px;cursor:pointer;display:flex;height:34px;justify-content:center;position:absolute;right:0;top:0;width:34px}
```

UTF-8`[395327, 395395]` SHA-256`d280eb545f01c218b10e1a4b5859a15053db4675ef547b88f5927b968c0a6c54`

```css
.controller-content{margin-bottom:20px;position:relative;width:100%}
```

UTF-8`[395395, 395471]` SHA-256`f285c81efb62a54e0ac1d2246cfe28c69a2f1989be9279179ab96dcd4833aff4`

```css
.controller-content .img{padding-bottom:68.75%;position:relative;width:100%}
```

UTF-8`[395471, 395583]` SHA-256`98a2a3cb0d05bd33299eb4eceffddb5c061dbebd9669ed6200687d50cca0bcee`

```css
.controller-content .img img,.controller-content .img svg{height:100%;left:0;position:absolute;top:0;width:100%}
```

UTF-8`[395583, 395714]` SHA-256`e0e3a86baab381e0e1bbbcdd827294789af8540146111f5f6fb099c3310e5526`

```css
.controller-button{fill:#0000;stroke:#5d5d5d;stroke-width:2;stroke-linejoin:round;stroke-miterlimit:10;opacity:0;position:relative}
```

UTF-8`[395714, 395776]` SHA-256`d3811daee36f60c6aac38f9c192cc35c9ab6284091ffedb0a2b80b02392b5292`

```css
.controller-button:hover{stroke:#44d62c;cursor:grab;opacity:1}
```

UTF-8`[395776, 395830]` SHA-256`0f7f0d50785f8ceda80adc17703c66e2c198bb11cdd6e79a119d7c08dff11450`

```css
.controller-button.hypershift-on:hover{stroke:#fd8611}
```

UTF-8`[395830, 395899]` SHA-256`03fa80b06d4d5128aa324c3d6677c7fc4662d48491aaac0024b9088977bb9a89`

```css
.controller-button.dragging{stroke:#44d62c;cursor:grabbing;opacity:1}
```

UTF-8`[395899, 395956]` SHA-256`9d1fc0a252c2306fd06056ab9d5bc2d2a8b071215fbce5be5aaf679f94e5fc76`

```css
.controller-button.dragging.hypershift-on{stroke:#fd8611}
```

UTF-8`[395956, 396183]` SHA-256`43806ea7f9e4b54ff76c5cd3d5707aaed16af98b02e16489efea32a5731a61b7`

```css
.controller-button-drag-image{align-items:center;background:#44d62c;border-radius:5px;cursor:grabbing!important;display:flex;height:40px;padding:10px;position:fixed;transform:translate(10px,10px);white-space:nowrap;z-index:100}
```

UTF-8`[396183, 396246]` SHA-256`f6cc3550e64eb12e8d09169af2ad0e18ea8af4ae6c61aaa1af6bd420ae44b4d6`

```css
.controller-button-drag-image:active{cursor:grabbing!important}
```

UTF-8`[396246, 396427]` SHA-256`19b48108bcc331556daf0e6d48e1fbc7ef0c3c1fc49e13dee9ba783740aa5783`

```css
.controller-button-drag-image .icon{background-position:50%;background-repeat:no-repeat;border-radius:50%;height:20px;margin-right:10px;min-width:20px;padding-right:10px;width:20px}
```

UTF-8`[396427, 396531]` SHA-256`d264d978a4a485d95ce0ae16c3a377d809ae974aafd486df99a1ca555c5c1f3f`

```css
.controller-button-drag-image .icon.xbox-a{background-image:url(../../static/media/xbox-a.c85cb7b9.svg)}
```

UTF-8`[396531, 396635]` SHA-256`28160687202109db8ff51c8267f3a158e7b8a37d07718dbf244ef6ac75d60cce`

```css
.controller-button-drag-image .icon.xbox-b{background-image:url(../../static/media/xbox-b.3e7f0305.svg)}
```

UTF-8`[396635, 396739]` SHA-256`faddf300219331b54ded35f62e142c633df421a444d9b473cb32735fd4749a73`

```css
.controller-button-drag-image .icon.xbox-x{background-image:url(../../static/media/xbox-x.4c09810f.svg)}
```

UTF-8`[396739, 396843]` SHA-256`dc02db89c7c27c9267b87cb359957b8b1b79b6af1b05cbc757e9a176a9458c05`

```css
.controller-button-drag-image .icon.xbox-y{background-image:url(../../static/media/xbox-y.9c2fb89a.svg)}
```

UTF-8`[396843, 396966]` SHA-256`c0410d838e2591983b9e84dc0fe9c0216f650894015c692d3848eac85ed666f0`

```css
.controller-button-drag-image .icon.trigger-right{background-image:url(../../static/media/xbox-trigger-right.7dfd89a0.svg)}
```

UTF-8`[396966, 397087]` SHA-256`da1593dd0e0299021bddd93ff52a364d45c6b4cf99134e8ea3898f06db9c87b6`

```css
.controller-button-drag-image .icon.trigger-left{background-image:url(../../static/media/xbox-trigger-left.4d0f408c.svg)}
```

UTF-8`[397087, 397208]` SHA-256`0ce2473da1df48c4638eda9f06e80cc9c13261f765e3b19d365e5572c15d0af6`

```css
.controller-button-drag-image .icon.right-bumper{background-image:url(../../static/media/xbox-button-right.54513544.svg)}
```

UTF-8`[397208, 397327]` SHA-256`befbfa84f559087026b0a10912424e18871c24a79a88ef7d848fbd262c425f62`

```css
.controller-button-drag-image .icon.left-bumper{background-image:url(../../static/media/xbox-button-left.956f0830.svg)}
```

UTF-8`[397327, 397456]` SHA-256`5e93e54f1b04c1ee661c24afa9aeee90ecc283dbb1edccba3109bd4368dbea6a`

```css
.controller-button-drag-image .icon.rightjoystick-up{background-image:url(../../static/media/xbox-rightjoystick-up.20cc1a34.svg)}
```

UTF-8`[397456, 397591]` SHA-256`62a2f8262ea4e3ad8a71f029b78d26a7cbb2b543c4e66ae345e6d0aae79cffb0`

```css
.controller-button-drag-image .icon.rightjoystick-right{background-image:url(../../static/media/xbox-rightjoystick-right.6eb651a2.svg)}
```

UTF-8`[397591, 397724]` SHA-256`cff5a64974f5bf856664a1a0faa1cbaf0cae61a1bb4e9cc62cc253723a1f8306`

```css
.controller-button-drag-image .icon.rightjoystick-left{background-image:url(../../static/media/xbox-rightjoystick-left.57986288.svg)}
```

UTF-8`[397724, 397857]` SHA-256`eff822e84a8598a118bed0ee2e29cf114c6ccb156ffa8d52e5317398c07aa886`

```css
.controller-button-drag-image .icon.rightjoystick-down{background-image:url(../../static/media/xbox-rightjoystick-down.bb1eccbf.svg)}
```

UTF-8`[397857, 397992]` SHA-256`089968c6ed48c81f12799fddbaed0e81558184d3a9227edf29fcd93c705cb6a4`

```css
.controller-button-drag-image .icon.rightjoystick-press{background-image:url(../../static/media/xbox-rightjoystick-press.72ccb46d.svg)}
```

UTF-8`[397992, 398119]` SHA-256`1de0fc65b78fa7a4d93390dc6e319bf41098bf3200c9122b80cb79d34246bf98`

```css
.controller-button-drag-image .icon.leftjoystick-up{background-image:url(../../static/media/xbox-leftjoystick-up.dea6602a.svg)}
```

UTF-8`[398119, 398252]` SHA-256`60bf3473c37282ded9a96661bd9c389f74f92ca6802b109b761aab8c52ddf4b3`

```css
.controller-button-drag-image .icon.leftjoystick-right{background-image:url(../../static/media/xbox-leftjoystick-right.d7503399.svg)}
```

UTF-8`[398252, 398383]` SHA-256`f978884461b9c11bd6c0ee988808eacdeb7a99349218bdee23b11206d4bcdeb2`

```css
.controller-button-drag-image .icon.leftjoystick-left{background-image:url(../../static/media/xbox-leftjoystick-left.bd19a0fe.svg)}
```

UTF-8`[398383, 398514]` SHA-256`e71d97287437c9458cc59dde1adaabacb7ecda3573cfefe6533559671a5e5e93`

```css
.controller-button-drag-image .icon.leftjoystick-down{background-image:url(../../static/media/xbox-leftjoystick-down.8a28a654.svg)}
```

UTF-8`[398514, 398647]` SHA-256`a14582a6d1d146e09b4a20422afea5ff78f72c6c64fd933b819f1322d32e64e9`

```css
.controller-button-drag-image .icon.leftjoystick-press{background-image:url(../../static/media/xbox-leftjoystick-press.a58c272c.svg)}
```

UTF-8`[398647, 398757]` SHA-256`3c2e3e08e5265e29f8186678c19eafb477650d975f441f1e8ac1cbf64a7cbe3f`

```css
.controller-button-drag-image .icon.xbox-menu{background-image:url(../../static/media/xbox-menu.029cb487.svg)}
```

UTF-8`[398757, 398867]` SHA-256`464064b99229ab255d9886d18c7b78f01a8cee9e9a473a6fc434270229dbe026`

```css
.controller-button-drag-image .icon.xbox-view{background-image:url(../../static/media/xbox-view.771d8231.svg)}
```

UTF-8`[398867, 398978]` SHA-256`88059a52f1b3b33addd933e4cfbf768ef32c2733c85f93ab52db16e225759705`

```css
.controller-button-drag-image .icon.dpad-up{background-image:url(../../static/media/xbox-dpad-up.8f32398c.svg)}
```

UTF-8`[398978, 399095]` SHA-256`976ea6ddb2babc5c3f9e7fffa98bff7c6322e3bc7d5310606d88b04e37725b11`

```css
.controller-button-drag-image .icon.dpad-right{background-image:url(../../static/media/xbox-dpad-right.34fa065b.svg)}
```

UTF-8`[399095, 399210]` SHA-256`96a5d9663bbfb5747a4d3213f631e730843f84f0842476d697b48b8ea4500eaa`

```css
.controller-button-drag-image .icon.dpad-down{background-image:url(../../static/media/xbox-dpad-down.b22e39fe.svg)}
```

UTF-8`[399210, 399325]` SHA-256`2da3aacb7f28e48f9e7140c64614ea1c1598bdfd052fe85c751ead1f3ff5a703`

```css
.controller-button-drag-image .icon.dpad-left{background-image:url(../../static/media/xbox-dpad-left.91e17572.svg)}
```

UTF-8`[399325, 399406]` SHA-256`622b589f6144a152554aa26a0e4a0ffe0247a9828a747e7d6681300f7b3d42ad`

```css
.controller-button-drag-image .text{color:#212121;font-size:14px;font-weight:700}
```

UTF-8`[399406, 399469]` SHA-256`922b30f90db4c682152401c7e2adb25d58af2dad36542b86f615936bae70da7a`

```css
.controller-button-drag-image.hypershift-on{background:#fd8611}
```

UTF-8`[399469, 399524]` SHA-256`46e460acffbfcdf51543ed8f558d1f7761b0c220d66f68463096c32242ec047d`

```css
.controller-feat-desc{font-size:14px;text-align:center}
```

UTF-8`[399524, 399596]` SHA-256`dc6fd1cf3364557b91b7cae8cc6b073eb0ad164ccc6790a2237f3b29f7c81df6`

```css
body .controller-button-key-tip{left:0;position:fixed;top:0;z-index:100}
```

UTF-8`[399596, 399687]` SHA-256`397ea8c25880de8e129f2860c32b08fad022dd33b0786d1e9ae90ce8630aa44f`

```css
body .controller-button-key-tip .key-tip{align-items:center;display:flex;position:relative}
```

UTF-8`[399687, 399737]` SHA-256`c33afea8bf52bc98c4f235af81d0c4752c836c57b88d9121ee1deac8bdc3bccb`

```css
body.dragging-something{cursor:grabbing!important}
```

UTF-8`[399737, 400010]` SHA-256`c7c93142301f8e2f68a9bbcac1dbe770b6ff7aa918a075ec2ef5ea3cc95981f7`

```css
.toggle-controller{background-color:#111;background-image:url(../../static/media/controller-fill-grey-icon.a9ec3fff.svg);background-position:50%;background-repeat:no-repeat;background-size:18px;border:1px solid #5d5d5d;border-radius:14px;height:27px;width:38px;z-index:100}
```

UTF-8`[400010, 400136]` SHA-256`402b8d1807b330b58d47d66a37c597c915a0751b2ea52aac91ef37754885da7a`

```css
.toggle-controller.open{background-image:url(../../static/media/controller-fill-green-icon.68e5616a.svg);border-color:#44d62c}
```

UTF-8`[427799, 428088]` SHA-256`6c6736137404a5e83869f996af10be3d15c2fdc0f9f9a22360be2c6dd96dcd5e`

```css
.key-tip{background-color:#000;border:1px solid #5d5d5d;line-height:14px;max-width:300px;min-height:75px;opacity:0;padding:initial;position:absolute;text-align:center;transition:opacity .3s,visibility 0s,left 0s,top 0s;visibility:hidden;will-change:opacity,visibility,left,top;z-index:102}
```

UTF-8`[428088, 428139]` SHA-256`07bf5cd3ff6dadc9004228572b39ba5abee6aad331951e20164eef061311326b`

```css
.key-tip .warning-content-wrapper{padding:8px 10px}
```

UTF-8`[428139, 428229]` SHA-256`ac5a5bbcd504c9b72a46f85f43fe2a4190d6d6c80717810660a943c3de81fa95`

```css
.key-tip .tip-msg{font-family:Roboto;font-size:12px;padding:2px 10px;white-space:pre-line}
```

UTF-8`[428229, 428294]` SHA-256`c140ccfcccfd32c38f283290ec6da3ddf4e4b0430240ad7fba33bd6f2afa7d47`

```css
.key-tip .devicenotconnected{background-color:#c8323c;color:#000}
```

UTF-8`[428294, 428353]` SHA-256`2d00bfa01926aa3b4f26819a01074c1056bca75054aad20aca2cf44a4f852074`

```css
.key-tip .greenmessage{background-color:#44d62c;color:#000}
```

UTF-8`[428353, 428413]` SHA-256`4eb92d7912238b1768bea9dc7f8f5227aa18434b738b5d6f67e63d3aa5ed4fa1`

```css
.key-tip .orangemessage{background-color:#fd8611;color:#000}
```

UTF-8`[428413, 428469]` SHA-256`68183537394ef43950d39ffb6ed5542c0387489a0086ae2319cd2a40649100b9`

```css
.key-tip .notmapped{background-color:#5d5d5d;color:#ccc}
```

UTF-8`[428469, 428536]` SHA-256`27d3185c1081651816b1b74a32bf1378cfd779e90d82152102057162b5b4431b`

```css
.key-tip .gamingmode-notmapped{background-color:#c8323c;color:#000}
```

UTF-8`[428536, 428582]` SHA-256`3bd76f902f375d0ddf4a106cc784952e94dce913900550785d7158736be4d120`

```css
.key-tip.display-warning{border-color:#c8323c}
```

UTF-8`[428582, 428625]` SHA-256`dbd22e1bce4560e208768465e9c5eadf58eff7181d8852ebdc1f27f5f9edd8f8`

```css
.key-tip.show{opacity:1;visibility:visible}
```

UTF-8`[428625, 428730]` SHA-256`bb2835f7bf7062bfd9f2e82b6b15c5a9b115845bca61d7f317234d0c2ae0edf3`

```css
.key-tip .index{color:#707070;font-size:14px;line-height:14px;margin-bottom:6px;text-transform:uppercase}
```

UTF-8`[428730, 428789]` SHA-256`d448e8c48b83ded925dc7e2b28b1734309dfc1f91f86902f5732a84159793e0c`

```css
.key-tip .index.special-character{text-transform:lowercase}
```

UTF-8`[428789, 428844]` SHA-256`743508cea8dee39a085897e767f2381f2b84f4b12852ce1e9533c4bc65bb36e1`

```css
.key-tip .name.special-name-character{line-height:26px}
```

UTF-8`[428844, 428927]` SHA-256`9478964b4132da50eff058c0af96ab6902793f09f05cb414c61db841af933b8b`

```css
.key-tip .type{color:#ccc;font-size:12px;line-height:14px;text-transform:uppercase}
```

UTF-8`[428927, 428985]` SHA-256`28e96da6c459eb1f1b7483f98c7ca3d27622e6be23b7dcd66d6c6fadaf68d2fb`

```css
.key-tip .name{color:#ccc;font-size:18px;line-height:18px}
```

UTF-8`[428985, 429078]` SHA-256`4e447866c5b197f1ed6d20f40dc4e73130c0cb4c183e4a651b2fdc5b457d440f`

```css
.key-tip .name,.key-tip .no-text-transform:first-line{min-height:19px;text-transform:inherit}
```

UTF-8`[429909, 430049]` SHA-256`72835eff41c5fa9252ea547597c9ca289b204506696147efb61357428357c31c`

```css
.stepper{border:1px solid #5d5d5d;box-sizing:border-box;height:27px;position:relative;transition:opacity .2s;width:60px;will-change:opacity}
```

UTF-8`[430049, 430190]` SHA-256`6b40952f3a15cf8028cc4e42b4ad30a28ab566d9db64110a825aa3b0f87a4d17`

```css
.stepper input{background-color:#111;border:none;box-sizing:border-box;color:#ccc;font-size:14px;line-height:17px;text-align:left;width:58px}
```

UTF-8`[438868, 439106]` SHA-256`585918dbeecb946df5d581dfbbd0ea84ae3d90fc09ecd3d475866dc3f359ac14`

```css
.key-config .body textarea{background-color:#111;border:1px solid #5d5d5d;box-sizing:border-box;color:#ccc;font-family:Roboto;font-size:14px;height:96px;line-height:17px;min-width:210px;overflow-y:auto;padding:5px;resize:none;width:210px}
```

UTF-8`[445640, 445980]` SHA-256`0b4f5f361e0a4bdb3434ef921d718fba5c5d21efdb7ab0a7a6152ee23a7fdd7e`

```css
.dynamic-key-stroke-body .map-header-item-adjustment .rapid-trigger-slider::-webkit-slider-thumb{-webkit-appearance:none;appearance:none;background:#44d62c;border-radius:8px;box-sizing:border-box;height:16px;-webkit-transition:transform .2s,background .3s;transition:transform .2s,background .3s;width:16px;will-change:transform,background}
```

UTF-8`[453674, 453992]` SHA-256`33a59687c9c99f3b59c2e84cb2c31be66c66d551a61acf4af5c0eb59ebdc16bd`

```css
.actuation-vertical-slider::-webkit-slider-thumb{-webkit-appearance:none;appearance:none;background:#ccc;background:var(--StateColor);border-radius:8px;box-sizing:border-box;height:16px;-webkit-transition:transform .2s,background .3s;transition:transform .2s,background .3s;width:16px;will-change:transform,background}
```

UTF-8`[458993, 459280]` SHA-256`ff86a7fbc477114c82bb93dc66479f3cfdec784f43aa3ab5d0538be29a44030e`

```css
.rapid-trigger-slider::-webkit-slider-thumb{-webkit-appearance:none;appearance:none;background:#fd8611;border-radius:8px;box-sizing:border-box;height:16px;-webkit-transition:transform .2s,background .3s;transition:transform .2s,background .3s;width:16px;will-change:transform,background}
```

UTF-8`[470874, 471053]` SHA-256`1565101a6d701eac2e17c0411591b5c4a429257894c0d80cb823cb6860a0793e`

```css
.skst-key-input,.skst-key-input-warning{background:#1f1f1f;box-sizing:border-box;font-size:12px;height:30px;margin:0 2px;max-width:30px;text-align:center;text-transform:lowercase}
```

UTF-8`[471134, 471313]` SHA-256`9f663e6a2c29d297e9422e052c75bd8d598daf1dbb1371b9daf364c215885994`

```css
.skst-key-record-item-v3{align-items:center;border:1px solid #666;border-radius:4px;box-sizing:border-box;color:#999;display:flex;height:30px;justify-content:right;min-width:30px}
```

## Original SVG resources

### `controller-skeleton.72fd3181.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/controller-skeleton.72fd3181.svg` SHA-256`1d0cb6e1a6ed76ed32983ca41ee837307ead2e07c81e8ba84d791e14ba2de15f`

```xml
<?xml version="1.0" encoding="utf-8"?>
<!-- Generator: Adobe Illustrator 26.1.0, SVG Export Plug-In . SVG Version: 6.00 Build 0)  -->
<svg version="1.1" id="Controller" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" x="0px" y="0px"
	 viewBox="0 0 320 220" style="enable-background:new 0 0 320 220;" xml:space="preserve">
<style type="text/css">
	.st0{fill:#222222;fill-opacity:0;}
	.st1{stroke:#222222;stroke-width:2;stroke-miterlimit:10;}
	.st2{stroke:#5D5D5D;stroke-width:2;stroke-linejoin:round;stroke-miterlimit:10;}
	.st2.active, 
	.st2:hover {
		stroke: #44d62c;
		stroke-width: 2px;
		stroke-linejoin: round;
		cursor: grab;
	}
	.st3{fill:#D2D2D2;}
</style>
<rect id="Container" class="st0" width="320" height="220"/>
<g id="Controller_00000030473021102778257660000002963152661268862393_" transform="translate(0 9)">
	<path id="Controller_Path" class="st1" d="M41.8,202L41.8,202c-4.4,0-8.8-0.8-12.8-2.5c-4.2-1.8-8-4.5-11.1-7.9
		c-3.6-4.1-6.2-8.9-7.9-14c-2.1-6.8-3.1-13.9-3-21c0-8.7,0.3-17.2,0.8-25c0.5-7.5,1.2-14.7,2.2-21.5c1.6-11.6,4.1-23.1,7.7-34.3
		c2.9-9,6.6-17.8,10.9-26.2c4-7.9,8.3-15.2,12.5-22.2v0c0.9-1.4,1.9-2.8,3-4.1l0.1-0.1l0-0.1c1.2-2.2,2.8-4,4.8-5.5
		c2.1-1.5,4.5-2.7,7-3.5c5.3-1.9,11.9-2.8,18.7-3.6l0.6-0.1C81,9.6,86.8,9.1,92.6,9c3.3-0.1,6.6,0.3,9.8,1c2.5,0.5,4.8,1.8,6.6,3.6
		c0.9,1.5,3.1,2.7,6.8,3.7c4.2,1,8.4,1.7,12.6,2.1c11.4,1.2,24.6,1.4,31.5,1.4c10.7,0,21.3-0.5,32-1.7c4.3-0.4,8.6-1.1,12.8-2.2
		c3.8-1,6-2.1,6.8-3.5c1.6-1.8,3.8-3.1,6.2-3.5c3.1-0.7,6.4-1.1,9.6-1c5.9,0.1,11.7,0.6,17.5,1.4l0.5,0.1c6.6,0.8,14,1.8,19.8,3.6
		c2.5,0.7,4.9,1.9,7.1,3.5c1.8,1.3,3,3.2,3.6,5.3l0.1,0.2l0.1,0.2c1.1,1.3,2,2.6,2.9,4.1v0v0c4.2,7.1,8.5,14.4,12.5,22.2
		c4.3,8.4,8,17.2,10.9,26.2c3.6,11.2,6.1,22.7,7.7,34.3c1,6.8,1.7,14,2.2,21.5c0.5,7.9,0.8,16.3,0.8,25v1.1c0,6.8-1,13.5-3,20
		c-1.6,5.2-4.3,10-7.9,14c-3.1,3.4-6.8,6.1-11.1,7.9c-4.1,1.7-8.4,2.5-12.8,2.5c-5.2,0-10.4-1.1-15.2-3.1l0,0l0,0
		c-14.4-6.1-22.7-13.9-30.1-20.8c-10-9.4-18.7-17.5-38.7-17.5h-68.5c-20,0-28.7,8.1-38.7,17.5c-7.4,6.9-15.7,14.7-30.1,20.8l0,0l0,0
		C52.1,200.9,47,202,41.8,202z"/>
</g>
<g id="RT" transform="translate(-216.61 -0.5)">
	<path id="Right_Trigger" class="st2" d="M484.7,24.8c-7.1-2-15.2-3.1-22.6-4.1l0,0l0,0l0,0l-0.7-0.1c-6.9-1-15.6-2.1-23.8-2.1
		c-1,0-2,0-3,0.1c3.4-2.5,6.4-4.5,9.2-6.2c6.4-3.8,11.7-5.6,16-5.6c0.5,0,0.8,0,1.2,0.1l0,0l0,0c4.5,0.4,6.2,2,10.6,6.4l0,0
		C474.5,15.8,478.4,19.7,484.7,24.8z"/>
</g>
<g id="LT" transform="translate(-216.61 -0.5)">
	<path id="Left_Trigger" class="st2" d="M267.7,24.8c6.3-5.1,10.2-8.9,13-11.6l0,0c4.5-4.4,6.1-6,10.6-6.4l0,0l0,0
		c0.4,0,0.7-0.1,1.2-0.1c4.3,0,9.6,1.8,16,5.6c2.8,1.7,5.8,3.7,9.2,6.2c-1,0-2-0.1-3-0.1c-8.2,0-16.8,1.2-23.8,2.1l-0.7,0.1l0,0l0,0
		l0,0C283,21.7,274.8,22.8,267.7,24.8z"/>
</g>
<g id="RB" transform="translate(0.209 9)">
	<path id="Right_Bumper" class="st2" d="M276.1,23.8l-67.5-8c1-0.4,1.9-1.1,2.5-2.1c1.6-2.1,3.9-3.5,6.5-4.1
		c3.2-0.8,6.5-1.2,9.7-1.1c5.9,0.1,11.8,0.7,17.6,1.6l0.5,0.1c6.6,0.9,14.1,1.9,19.9,4c2.6,0.9,5.1,2.2,7.2,3.9
		C274.3,19.6,275.5,21.6,276.1,23.8z"/>
</g>
<g id="LB" transform="translate(38 17.534)">
	<path id="Left_Bumper" class="st2" d="M6,15.3l67.5-8c-1-0.4-1.9-1.1-2.5-2.1c-1.6-2.1-3.9-3.5-6.5-4.1c-3.2-0.8-6.5-1.2-9.7-1.1
		c-5.9,0.1-11.8,0.7-17.6,1.6l-0.5,0.1c-6.6,0.9-14.1,1.9-19.9,4c-2.6,0.9-5.1,2.2-7.2,3.9C7.9,11.1,6.6,13,6,15.3z"/>
</g>
<g id="Select" transform="translate(-0.501 9)">
	<circle id="Select_Button" class="st2" cx="142" cy="67.4" r="10"/>
	<path id="Select_Path" class="st3" d="M148,72.4h-8v-6h8V72.4z M141,71.4h6v-4h-6V71.4z M136,62.4v6h2v-1h-1v-4h6v1h1v-2H136z"/>
</g>
<g id="Menu" transform="translate(0.503 9)">
	<circle id="Menu_Button" class="st2" cx="178" cy="67" r="10"/>
	<path id="Menu_Path" class="st3" d="M183,70.5h-10v-1h10V70.5z M183,66.5h-10v1h10V66.5z M183,63.5h-10v1h10V63.5z"/>
</g>
<g id="Right_Joystick" transform="translate(-0.5 9.5)">
	<path id="Right_JoystickUp" class="st2" d="M219.1,100.4l-7.1,7.1c-7.5-7.7-19.7-7.9-27.5-0.5c-0.2,0.2-0.3,0.3-0.5,0.5l-7.1-7.1
		c0,0,0.1-0.1,0.1-0.1l0.2-0.2c11.6-11.5,30.3-11.4,41.7,0.2C219,100.3,219,100.3,219.1,100.4z"/>
	<path id="Right_JoystickRight" class="st2" d="M227.5,121c0,7.9-3.2,15.5-8.9,21.1l-7.1-7.1c7.7-7.4,7.9-19.6,0.6-27.4l7.1-7.1
		C224.5,106,227.5,113.3,227.5,121z"/>
	<path id="Right_Joystick_Down" class="st2" d="M218.5,142.2c-11.4,11.1-29.6,11.1-41,0l7.1-7.1c7.5,7.1,19.3,7.1,26.8,0
		L218.5,142.2z"/>
	<path id="Right_JoystickLeft" class="st2" d="M184.5,135l-7.1,7.1c-11.6-11.3-11.8-29.9-0.6-41.6l7.1,7.1
		C176.5,115.3,176.8,127.5,184.5,135z"/>
	<circle id="Right_Joystick_Press" class="st2" cx="198" cy="121.4" r="19.5"/>
</g>
<g id="A" transform="translate(0 9)">
	<circle id="A_Button" class="st2" cx="253" cy="87" r="10"/>
	<path id="Path_A" class="st3" d="M257.6,91.3h-1.8l-0.9-2.5H251l-0.8,2.5h-1.8l3.7-9.8h1.8L257.6,91.3z M254.5,87.5l-1.4-3.9
		c-0.1-0.2-0.1-0.4-0.1-0.6h0c0,0.2-0.1,0.4-0.1,0.6l-1.3,3.9H254.5z"/>
</g>
<g id="B" transform="translate(-0.501 9)">
	<circle id="B_Button" class="st2" cx="273" cy="67" r="10"/>
	<path id="Path_B" class="st3" d="M270,71.9v-9.8h3.1c0.8,0,1.6,0.2,2.3,0.6c0.5,0.4,0.8,1,0.8,1.6c0,0.5-0.2,1-0.5,1.4
		c-0.3,0.4-0.8,0.7-1.3,0.9v0c0.6,0,1.2,0.3,1.6,0.7c0.4,0.4,0.6,1,0.6,1.6c0,0.8-0.4,1.6-1,2c-0.7,0.5-1.6,0.8-2.5,0.8L270,71.9z
		 M271.6,63.4v2.8h1.1c0.5,0,0.9-0.1,1.3-0.4c0.3-0.3,0.5-0.7,0.5-1.1c0-0.8-0.6-1.3-1.7-1.3L271.6,63.4z M271.6,67.5v3.1h1.4
		c0.5,0,1-0.1,1.4-0.4c0.3-0.3,0.5-0.7,0.5-1.2c0-1-0.7-1.5-2.1-1.5L271.6,67.5z"/>
</g>
<g id="X" transform="translate(-0.501 9)">
	<circle id="X_Button" class="st2" cx="233" cy="67" r="10"/>
	<path id="Path_X" class="st3" d="M237.2,71.9h-1.9l-2-3.6c-0.1-0.1-0.1-0.3-0.2-0.5h0c0,0.1-0.1,0.3-0.2,0.5l-2.1,3.6h-2L232,67
		l-3-4.9h2l1.8,3.3c0.1,0.2,0.2,0.4,0.3,0.7h0c0.1-0.3,0.2-0.5,0.3-0.7l1.8-3.3h1.8l-3,4.9L237.2,71.9z"/>
</g>
<g id="Y" transform="translate(0 9)">
	<circle id="Y_Button" class="st2" cx="253" cy="47" r="10"/>
	<path id="Path_Y" class="st3" d="M257,42.1l-3.2,6.2v3.6h-1.6v-3.5l-3.1-6.3h1.9l1.9,4.2c0,0,0.1,0.2,0.2,0.6h0
		c0.1-0.2,0.1-0.4,0.2-0.6l2-4.2H257z"/>
</g>
<g id="Left_Joystick" transform="translate(-0.001 9.5)">
	<path id="Left_Joystick_Up" class="st2" d="M88.1,46.4L81,53.5c-7.4-7.7-19.7-7.9-27.5-0.5c-0.2,0.2-0.3,0.3-0.5,0.5l-7.1-7.1
		c0,0,0.1-0.1,0.1-0.1l0.2-0.2c11.6-11.5,30.2-11.4,41.7,0.2C88,46.3,88,46.3,88.1,46.4z"/>
	<path id="Left_Joystick_Right" class="st2" d="M96.5,67c0,7.9-3.2,15.5-8.9,21.1L80.5,81c7.7-7.4,7.9-19.6,0.6-27.4l7.1-7.1
		C93.5,52,96.5,59.3,96.5,67z"/>
	<path id="Left_Joystick_Down" class="st2" d="M87.5,88.2c-11.4,11.1-29.6,11.1-41,0l7.1-7.1c7.5,7.1,19.3,7.1,26.8,0L87.5,88.2z"/>
	<path id="Left_Joystick_Left" class="st2" d="M53.5,81l-7.1,7.1c-11.6-11.3-11.8-29.9-0.6-41.6l7.1,7.1
		C45.5,61.3,45.8,73.5,53.5,81z"/>
	<ellipse id="Left_Joystick_Press" class="st2" cx="67" cy="67.4" rx="19.5" ry="19.5"/>
</g>
<g id="Dpad" transform="translate(85 100)">
	<path id="Dpad_Background" class="st1" d="M66,30.5c0-3.6-0.6-7.1-1.9-10.5H47V2.9c-6.8-2.6-14.2-2.6-21,0V20H8.9
		c-2.6,6.8-2.6,14.2,0,21H26v17.1c6.8,2.6,14.2,2.6,21,0V41h17.1C65.4,37.6,66,34.1,66,30.5z"/>
	<path id="Dpad_Up" class="st2" d="M36.5,5c2.2,0,4.4,0.3,6.5,0.8V20H30V5.8C32.1,5.3,34.3,5,36.5,5z"/>
	<path id="Dpad_Right" class="st2" d="M62,30.5c0,2.2-0.3,4.4-0.8,6.5H47V24h14.2C61.7,26.1,62,28.3,62,30.5z"/>
	<path id="Dpad_Down" class="st2" d="M30,41h13v14.2c-4.3,1.1-8.7,1.1-13,0V41z"/>
	<path id="Dpad_Left" class="st2" d="M11.8,24H26v13H11.8C10.7,32.7,10.7,28.3,11.8,24z"/>
</g>
</svg>

```

### `icon_draggable.be674683.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/icon_draggable.be674683.svg` SHA-256`01415d956dc6b497a1d4e2109e08f631497cc201490fc9ac22b04499545275f2`

```xml
<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="18" height="8" viewBox="0 0 18 8"><defs><style>.a{fill:none;}.b{clip-path:url(#a);}.c{fill:#ccc;}</style><clipPath id="a"><rect class="a" width="8" height="18"/></clipPath></defs><g class="b" transform="translate(18) rotate(90)"><path class="c" d="M21,4.5A1.5,1.5,0,1,0,19.5,6,1.5,1.5,0,0,0,21,4.5Zm0,5A1.5,1.5,0,1,0,19.5,11,1.5,1.5,0,0,0,21,9.5Zm0,5A1.5,1.5,0,1,0,19.5,16,1.5,1.5,0,0,0,21,14.5Zm0,5A1.5,1.5,0,1,0,19.5,21,1.5,1.5,0,0,0,21,19.5Zm-5-15A1.5,1.5,0,1,0,14.5,6,1.5,1.5,0,0,0,16,4.5Zm0,5A1.5,1.5,0,1,0,14.5,11,1.5,1.5,0,0,0,16,9.5Zm0,5A1.5,1.5,0,1,0,14.5,16,1.5,1.5,0,0,0,16,14.5Zm0,5A1.5,1.5,0,1,0,14.5,21,1.5,1.5,0,0,0,16,19.5Z" transform="translate(-13 -3)"/></g></svg>
```

### `controller-fill-grey-icon.a9ec3fff.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/controller-fill-grey-icon.a9ec3fff.svg` SHA-256`5f7fe8b837768c8caae51c6db4666f88b0b1b61144a1e22bf0b227d4503c6504`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#222;opacity:0;}.b{fill:#ccc;}</style></defs><rect class="a" width="20" height="20"/><path class="b" d="M19.968,10.495V10.5L18.917,3.893A4.093,4.093,0,0,0,15.767.671a25.128,25.128,0,0,0-11.534,0,4.093,4.093,0,0,0-3.15,3.221L.033,10.5A3.058,3.058,0,0,0,3.162,14a3.6,3.6,0,0,0,3.422-2.331L6.818,11h6.364l.235.666a3.6,3.6,0,0,0,3.421,2.324A3.057,3.057,0,0,0,20,11.042a3.133,3.133,0,0,0-.03-.548M9,6.41a.409.409,0,0,1-.409.409H6.823V8.59A.409.409,0,0,1,6.414,9h-.82a.409.409,0,0,1-.417-.4V6.819H3.409A.409.409,0,0,1,3,6.41V5.592a.408.408,0,0,1,.409-.408H5.177V3.412A.409.409,0,0,1,5.586,3h.82a.409.409,0,0,1,.409.409V5.184H8.6a.408.408,0,0,1,.4.408Zm4.509,2.339H13.5a1.246,1.246,0,1,1,.009,0m2-3H15.5a1.247,1.247,0,1,1,.007,0" transform="translate(0 3)"/></svg>
```

### `controller-fill-green-icon.68e5616a.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/controller-fill-green-icon.68e5616a.svg` SHA-256`1cda5332b70e62c4f74a65cf0bbefeabdb2fd643a77cd77ca4282455d5d72864`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a,.b{fill:#44d62c;}.a{opacity:0;}</style></defs><rect class="a" width="20" height="20"/><path class="b" d="M19.968,10.495V10.5L18.917,3.893A4.093,4.093,0,0,0,15.767.671a25.128,25.128,0,0,0-11.534,0,4.093,4.093,0,0,0-3.15,3.221L.033,10.5A3.058,3.058,0,0,0,3.162,14a3.6,3.6,0,0,0,3.422-2.331L6.818,11h6.364l.235.666a3.6,3.6,0,0,0,3.421,2.324A3.057,3.057,0,0,0,20,11.042a3.133,3.133,0,0,0-.03-.548M9,6.41a.409.409,0,0,1-.409.409H6.823V8.59A.409.409,0,0,1,6.414,9h-.82a.409.409,0,0,1-.417-.4V6.819H3.409A.409.409,0,0,1,3,6.41V5.592a.408.408,0,0,1,.409-.408H5.177V3.412A.409.409,0,0,1,5.586,3h.82a.409.409,0,0,1,.409.409V5.184H8.6a.408.408,0,0,1,.4.408Zm4.509,2.339H13.5a1.246,1.246,0,1,1,.009,0m2-3H15.5a1.247,1.247,0,1,1,.007,0" transform="translate(0 3)"/></svg>
```

### `icon_close.55fe41f1.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/icon_close.55fe41f1.svg` SHA-256`53b67d2c7d30aa84869391d651541e981b2f9ac508a2c0c04d37f5f4a8faef11`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><path d="m13.409 12.005 5.3-5.3a1 1 0 0 0 0-1.414 1 1 0 0 0-1.414 0l-5.3 5.3-5.3-5.3a1 1 0 0 0-1.414 0 1 1 0 0 0 0 1.414l5.3 5.3-5.3 5.3A1 1 0 0 0 6.7 18.719l5.3-5.3 5.3 5.3a1 1 0 0 0 1.414-1.414Z" transform="translate(-1.989 -2)" style="fill:#999"/></svg>
```

### `path.0086a00e.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/path.0086a00e.svg` SHA-256`4a0fcd9621e96a2a46fb2d9f60b78288d009d736cc6036bbfefd6cf212874a6a`

```xml
<svg width="14" height="20" viewBox="0 0 14 20" fill="none" xmlns="http://www.w3.org/2000/svg">
<path d="M0 11.0714C0 11.0714 6.125 20 7 20C7.875 20 14 11.0714 14 11.0714V3C14 1.34314 12.6569 0 11 0H3C1.34315 0 0 1.34315 0 3V11.0714Z" fill="#44D62C"/>
</svg>

```

### `xbox-a.c85cb7b9.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-a.c85cb7b9.svg` SHA-256`67b5af974eeea36dc8fa9c4e5be805aaf764210e7876cd61cd4ddd9764c1ddb0`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a,.d{fill:none;}.a{stroke:#2e2e2e;stroke-miterlimit:10;}.b{fill:#44d62c;}.c{stroke:none;}</style></defs><circle cx="10" cy="10" r="10"/><g class="a"><circle class="c" cx="10" cy="10" r="10"/><circle class="d" cx="10" cy="10" r="9.5"/></g><path class="b" d="M-17.427-51.2h-1.784l-.882-2.5h-3.855l-.848,2.5h-1.777L-22.9-61h1.832Zm-3.1-3.822-1.361-3.91a4.656,4.656,0,0,1-.13-.615h-.027a3.874,3.874,0,0,1-.137.615l-1.346,3.91Z" transform="translate(32 66)"/></svg>
```

### `xbox-b.3e7f0305.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-b.3e7f0305.svg` SHA-256`63ac2cd9b105b30392e73697ce2f400aa7cb3561f4b69157814c9b743cc7c949`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a,.d{fill:none;}.a{stroke:#2e2e2e;stroke-miterlimit:10;}.b{fill:#ff000b;}.c{stroke:none;}</style></defs><circle cx="10" cy="10" r="10"/><g class="a"><circle class="c" cx="10" cy="10" r="10"/><circle class="d" cx="10" cy="10" r="9.5"/></g><path class="b" d="M9,16.8V7h3.11a3.682,3.682,0,0,1,2.253.622,1.933,1.933,0,0,1,.83,1.62,2.315,2.315,0,0,1-.471,1.449,2.542,2.542,0,0,1-1.306.875v.028a2.562,2.562,0,0,1,1.617.748,2.282,2.282,0,0,1,.6,1.644,2.468,2.468,0,0,1-.992,2.037,3.918,3.918,0,0,1-2.5.78Zm1.62-8.5V11.1h1.053a2.023,2.023,0,0,0,1.329-.4,1.387,1.387,0,0,0,.482-1.132q0-1.257-1.681-1.257Zm0,4.094v3.1h1.388a2.108,2.108,0,0,0,1.4-.421,1.438,1.438,0,0,0,.495-1.158q0-1.524-2.112-1.525Z" transform="translate(-2 -2)"/></svg>
```

### `xbox-x.4c09810f.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-x.4c09810f.svg` SHA-256`d2d5180c2bc8c4f4afa20c8ea7911bb9f82ea0103114ff1da15c19717ae2edfc`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a,.d{fill:none;}.a{stroke:#2e2e2e;stroke-miterlimit:10;}.b{fill:#00a8ff;}.c{stroke:none;}</style></defs><circle cx="10" cy="10" r="10"/><g class="a"><circle class="c" cx="10" cy="10" r="10"/><circle class="d" cx="10" cy="10" r="9.5"/></g><path class="b" d="M50.211,16.8H48.263l-2.01-3.6a3.478,3.478,0,0,1-.2-.465h-.028c-.041.1-.109.256-.205.465l-2.071,3.6H41.789L45,11.874,42.049,7h1.989l1.771,3.309c.113.218.216.437.307.656h.021q.2-.43.342-.684L48.317,7h1.832l-3.021,4.86Z" transform="translate(-36 -2)"/></svg>
```

### `xbox-y.9c2fb89a.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-y.9c2fb89a.svg` SHA-256`0bfe205f0e450bde7ea55e43bac887d39e191617354af2242f09acfef511e3ed`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a,.d{fill:none;}.a{stroke:#2e2e2e;stroke-miterlimit:10;}.b{fill:#ffba00;}.c{stroke:none;}</style></defs><circle cx="10" cy="10" r="10"/><g class="a"><circle class="c" cx="10" cy="10" r="10"/><circle class="d" cx="10" cy="10" r="9.5"/></g><path class="b" d="M15.985,7.1,12.772,13.34V16.9H11.146V13.374L8.015,7.1H9.867L11.8,11.3q.034.075.2.567h.021a3.88,3.88,0,0,1,.225-.567l2.017-4.2Z" transform="translate(-2 -2)"/></svg>
```

### `xbox-trigger-right.7dfd89a0.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-trigger-right.7dfd89a0.svg` SHA-256`05d02efc0723242ae95bd58f599eb23c478751c5016f1d70966749343e7c542b`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#222;opacity:0;}.b,.c{fill-rule:evenodd;}.c{fill:none;stroke-miterlimit:10;}.d{fill:#fff;}.e,.f{stroke:none;}.f{fill:#2e2e2e;}</style></defs><rect class="a" width="20" height="20"/><path class="b" d="M5.006,16.5,6.7,5.452A1.5,1.5,0,0,1,8.251,4C19.232,4.551,19,18.574,19,19.4s-.879.553-.879.553l-12.3-2.418A.944.944,0,0,1,5.006,16.5Z" transform="translate(-2 -2)"/><g class="c" transform="translate(-2 -2)"><path class="e" d="M5.006,16.5,6.7,5.452A1.5,1.5,0,0,1,8.251,4C19.232,4.551,19,18.574,19,19.4s-.879.553-.879.553l-12.3-2.418A.944.944,0,0,1,5.006,16.5Z"/><path class="f" d="M 17.99692153930664 18.9124813079834 C 17.97101402282715 17.452880859375 17.77450752258301 14.56134700775146 16.74031066894531 11.76506042480469 C 15.15601634979248 7.481406211853027 12.2838716506958 5.205286026000977 8.203847885131836 4.999894618988037 C 8.203145980834961 4.99989652633667 8.202466011047363 4.999900817871094 8.201765060424805 4.999905109405518 L 8.218609809875488 5.003230094909668 L 8.192948341369629 5.000014305114746 C 7.761098384857178 5.008091449737549 7.709874629974365 5.407546043395996 7.687889575958252 5.579110145568848 L 7.686339855194092 5.591169834136963 L 7.684509754180908 5.603179931640625 L 6.010175704956055 16.55075073242188 C 6.02125358581543 16.5550651550293 6.034488677978516 16.55971717834473 6.050143718719482 16.56449317932129 L 17.99692153930664 18.9124813079834 M 18.43004035949707 20.00009536743164 C 18.26143074035645 20.00009536743164 18.12100028991699 19.95599937438965 18.12100028991699 19.95599937438965 L 5.817999839782715 17.53800010681152 C 4.872999668121338 17.2859992980957 5.005999565124512 16.50200080871582 5.005999565124512 16.50200080871582 L 6.695999622344971 5.452000141143799 C 6.872512817382813 4.074664115905762 7.970154762268066 3.999893665313721 8.206302642822266 3.999893665313721 C 8.235049247741699 3.999893665313721 8.250999450683594 4.000999927520752 8.250999450683594 4.000999927520752 C 19.23200035095215 4.551000118255615 19 18.57399940490723 19 19.40299987792969 C 19 19.90063285827637 18.68325233459473 20.00009536743164 18.43004035949707 20.00009536743164 Z"/></g><path class="d" d="M7.788,10.213A9.336,9.336,0,0,1,9.275,10.1a2.493,2.493,0,0,1,1.663.432,1.361,1.361,0,0,1,.464,1.079,1.436,1.436,0,0,1-.975,1.351v.024a1.372,1.372,0,0,1,.752,1.039,10.623,10.623,0,0,0,.424,1.5H10.587a7.618,7.618,0,0,1-.36-1.272c-.152-.7-.408-.927-.967-.943h-.5V15.53H7.788ZM8.764,12.6h.584c.663,0,1.079-.351,1.079-.887,0-.592-.416-.864-1.056-.864a2.73,2.73,0,0,0-.607.049Z" transform="translate(-2 -2)"/><path class="d" d="M13.412,10.964H11.877v-.823h4.069v.823H14.4V15.53h-.984Z" transform="translate(-2 -2)"/></svg>
```

### `xbox-trigger-left.4d0f408c.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-trigger-left.4d0f408c.svg` SHA-256`a880629f96e01c83a559e4d6f2c93684fa907f4110e1c9024ce1aa52135688c5`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#222;opacity:0;}.b,.c{fill-rule:evenodd;}.c{fill:none;stroke-miterlimit:10;}.d{fill:#fff;}.e,.f{stroke:none;}.f{fill:#2e2e2e;}</style></defs><rect class="a" width="20" height="20"/><path class="b" d="M18.182,17.538l-12.3,2.418S5,20.232,5,19.4,4.768,4.551,15.749,4A1.5,1.5,0,0,1,17.3,5.452l1.69,11.05A.944.944,0,0,1,18.182,17.538Z" transform="translate(-2 -2)"/><g class="c" transform="translate(-2 -2)"><path class="e" d="M18.182,17.538l-12.3,2.418S5,20.232,5,19.4,4.768,4.551,15.749,4A1.5,1.5,0,0,1,17.3,5.452l1.69,11.05A.944.944,0,0,1,18.182,17.538Z"/><path class="f" d="M 6.003077983856201 18.9124813079834 L 17.94985580444336 16.56449317932129 C 17.96551132202148 16.55971717834473 17.97874641418457 16.5550651550293 17.98982429504395 16.55075073242188 L 16.31549072265625 5.603179931640625 L 16.31365966796875 5.591169834136963 L 16.31211090087891 5.57912015914917 C 16.28995132446289 5.406191825866699 16.23808097839355 5.001771926879883 15.79609870910645 4.999897003173828 C 11.71607494354248 5.205288410186768 8.84398365020752 7.481406211853027 7.259690284729004 11.76506042480469 C 6.225492477416992 14.56134700775146 6.02898645401001 17.452880859375 6.003077983856201 18.9124813079834 M 5.569958686828613 20.00009536743164 C 5.316734790802002 20.00009536743164 5 19.90068244934082 5 19.40299987792969 C 5 18.57399940490723 4.76800012588501 4.551000118255615 15.74900054931641 4.000999927520752 C 15.74900054931641 4.000999927520752 15.76498126983643 3.999893665313721 15.79369735717773 3.999893665313721 C 16.02974319458008 3.999893665313721 17.12747573852539 4.074569702148438 17.30400085449219 5.452000141143799 L 18.99399948120117 16.50200080871582 C 18.99399948120117 16.50200080871582 19.12700080871582 17.2859992980957 18.1820011138916 17.53800010681152 L 5.879000186920166 19.95599937438965 C 5.879000186920166 19.95599937438965 5.738534450531006 20.00009536743164 5.569958686828613 20.00009536743164 Z"/></g><path class="d" d="M9.788,10.141h.983v4.565h2.215v.824h-3.2Z" transform="translate(-2 -2)"/><path class="d" d="M13.988,10.964H12.453v-.823h4.069v.823h-1.55V15.53h-.984Z" transform="translate(-2 -2)"/></svg>
```

### `xbox-button-right.54513544.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-button-right.54513544.svg` SHA-256`9b9ddcc33c03842966f9fb512f7338dcf2ab448449a1fbed669bf3eb4ca96452`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#222;opacity:0;}.b,.c{fill-rule:evenodd;}.c{fill:none;stroke-miterlimit:10;}.d{fill:#fff;}.e,.f{stroke:none;}.f{fill:#2e2e2e;}</style></defs><rect class="a" width="20" height="20"/><path class="b" d="M21.989,10.377l-.752,4.292c-.9,3.033-7.709,2.989-10.531,3.182-2.46.168-7.954.148-7.954.148C1.993,18,2,17.407,2,17.407V12.819c0-.888,1.053-.74,1.053-.74H5.139a.711.711,0,0,0,.677-.444l.753-4.071a.648.648,0,0,1,.9-.518L20.786,9.119C22.195,9.377,21.989,10.377,21.989,10.377Z" transform="translate(-2 -2)"/><g class="c" transform="translate(-2 -2)"><path class="e" d="M21.989,10.377l-.752,4.292c-.9,3.033-7.709,2.989-10.531,3.182-2.46.168-7.954.148-7.954.148C1.993,18,2,17.407,2,17.407V12.819c0-.888,1.053-.74,1.053-.74H5.139a.711.711,0,0,0,.677-.444l.753-4.071a.648.648,0,0,1,.9-.518L20.786,9.119C22.195,9.377,21.989,10.377,21.989,10.377Z"/><path class="f" d="M 7.493919372558594 8.061618804931641 L 6.799320220947266 11.81688022613525 L 6.785379409790039 11.89223957061768 L 6.760120391845703 11.96459007263184 C 6.56648063659668 12.51928997039795 5.990110397338867 13.07899951934814 5.138999938964844 13.07899951934814 L 3.052999496459961 13.07899951934814 L 3.001008987426758 13.08483982086182 L 3 13.08465957641602 L 3 16.99954032897949 C 3.08203125 16.99965286254883 3.183120727539063 16.99974060058594 3.30088996887207 16.99974060058594 C 4.862279891967773 16.99974060058594 8.70281982421875 16.98546981811523 10.63776969909668 16.85332870483398 C 10.97103977203369 16.83053970336914 11.35995960235596 16.81087875366211 11.77170944213867 16.79007911682129 C 13.31781005859375 16.71194839477539 15.43525981903076 16.60494995117188 17.20495986938477 16.20019912719727 C 18.42546272277832 15.92105865478516 19.93622589111328 15.38848495483398 20.26345825195313 14.43101978302002 L 20.99670791625977 10.24603843688965 C 20.96534538269043 10.21670532226563 20.85470390319824 10.14974403381348 20.61766242980957 10.10484218597412 L 7.493919372558594 8.061618804931641 M 7.183773040771484 6.999580383300781 C 7.343879699707031 6.999580383300781 7.471000671386719 7.045999526977539 7.471000671386719 7.045999526977539 L 20.7859992980957 9.118999481201172 C 22.19499969482422 9.376999855041504 21.98900032043457 10.3769998550415 21.98900032043457 10.3769998550415 L 21.23699951171875 14.66899967193604 C 20.33600044250488 17.70199966430664 13.52799987792969 17.65800094604492 10.70600032806396 17.85099983215332 C 8.719111442565918 17.98669052124023 4.752935409545898 17.9997386932373 3.30088996887207 17.9997386932373 C 2.955133438110352 17.9997386932373 2.75200080871582 17.99900054931641 2.75200080871582 17.99900054931641 C 1.993000030517578 17.99900054931641 2 17.4069995880127 2 17.4069995880127 L 2 12.81900024414063 C 2 12.15299987792969 2.592311859130859 12.06974983215332 2.888467788696289 12.06974983215332 C 2.987186431884766 12.06974983215332 3.052999496459961 12.07899951934814 3.052999496459961 12.07899951934814 L 5.138999938964844 12.07899951934814 C 5.660999298095703 12.07899951934814 5.815999984741211 11.63500022888184 5.815999984741211 11.63500022888184 L 6.569000244140625 7.563999176025391 C 6.655117034912109 7.095663070678711 6.953523635864258 6.999580383300781 7.183773040771484 6.999580383300781 Z"/></g><path class="d" d="M13.113,15.443H12.027l-.894-1.5a3.167,3.167,0,0,0-.237-.353,1.339,1.339,0,0,0-.234-.24.783.783,0,0,0-.26-.137,1.064,1.064,0,0,0-.316-.043H9.711v2.273H8.785v-5.6h1.844a2.642,2.642,0,0,1,.726.094,1.665,1.665,0,0,1,.579.283,1.309,1.309,0,0,1,.384.472,1.485,1.485,0,0,1,.139.663,1.608,1.608,0,0,1-.088.544,1.4,1.4,0,0,1-.25.442,1.525,1.525,0,0,1-.39.33,1.954,1.954,0,0,1-.514.211V12.9a1.474,1.474,0,0,1,.265.19c.076.069.147.14.215.213a2.513,2.513,0,0,1,.2.248c.067.092.141.2.221.322ZM9.711,10.6v1.82h.773a1.156,1.156,0,0,0,.4-.066.9.9,0,0,0,.314-.192.855.855,0,0,0,.207-.3,1.037,1.037,0,0,0,.075-.4.8.8,0,0,0-.254-.629,1.065,1.065,0,0,0-.731-.226Z" transform="translate(-2 -2)"/><path class="d" d="M13.766,15.443v-5.6h1.777a2.1,2.1,0,0,1,1.287.355,1.107,1.107,0,0,1,.475.926,1.319,1.319,0,0,1-.27.828,1.451,1.451,0,0,1-.746.5v.016a1.463,1.463,0,0,1,.924.428,1.3,1.3,0,0,1,.346.939A1.411,1.411,0,0,1,16.992,15a2.24,2.24,0,0,1-1.43.445Zm.925-4.855v1.594h.6a1.159,1.159,0,0,0,.76-.229.794.794,0,0,0,.275-.646q0-.719-.961-.719Zm0,2.34V14.7h.793a1.207,1.207,0,0,0,.8-.24.821.821,0,0,0,.283-.662q0-.871-1.207-.871Z" transform="translate(-2 -2)"/></svg>
```

### `xbox-button-left.956f0830.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-button-left.956f0830.svg` SHA-256`7d752136979f50414c5e8411a75aee9f1c18e09aaf2a58c42f99fbaa5f864dfd`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#222;opacity:0;}.b,.c{fill-rule:evenodd;}.c{fill:none;stroke-miterlimit:10;}.d{fill:#fff;}.e,.f{stroke:none;}.f{fill:#2e2e2e;}</style></defs><rect class="a" width="20" height="20"/><path class="b" d="M3.214,9.119,16.529,7.046a.648.648,0,0,1,.9.518l.753,4.071a.711.711,0,0,0,.677.444h2.086S22,11.931,22,12.819v4.588s.007.592-.752.592c0,0-5.494.02-7.954-.148-2.822-.193-9.63-.149-10.531-3.182l-.752-4.292S1.8,9.377,3.214,9.119Z" transform="translate(-2 -2)"/><g class="c" transform="translate(-2 -2)"><path class="e" d="M3.214,9.119,16.529,7.046a.648.648,0,0,1,.9.518l.753,4.071a.711.711,0,0,0,.677.444h2.086S22,11.931,22,12.819v4.588s.007.592-.752.592c0,0-5.494.02-7.954-.148-2.822-.193-9.63-.149-10.531-3.182l-.752-4.292S1.8,9.377,3.214,9.119Z"/><path class="f" d="M 16.50608062744141 8.061618804931641 L 3.38233757019043 10.10484218597412 C 3.145296096801758 10.14974403381348 3.03465461730957 10.21670532226563 3.003292083740234 10.24603843688965 L 3.736541748046875 14.43101978302002 C 4.063774108886719 15.38848495483398 5.57453727722168 15.92105865478516 6.795040130615234 16.20019912719727 C 8.564740180969238 16.60494995117188 10.68218994140625 16.71194839477539 12.22829055786133 16.79007911682129 C 12.64004039764404 16.81087875366211 13.02896022796631 16.83053970336914 13.3621301651001 16.85331916809082 C 15.29718017578125 16.98546981811523 19.13772010803223 16.99974060058594 20.69911003112793 16.99974060058594 C 20.81676864624023 16.99974060058594 20.9178295135498 16.99965286254883 20.9999942779541 16.99954032897949 L 20.99993515014648 13.07899951934814 L 20.94700050354004 13.07899951934814 L 18.86100006103516 13.07899951934814 C 18.00988960266113 13.07899951934814 17.43352127075195 12.51928997039795 17.2398796081543 11.96459007263184 L 17.21462059020996 11.89223957061768 L 17.20067977905273 11.81688022613525 L 16.50608062744141 8.061618804931641 M 16.81622695922852 6.999580383300781 C 17.04648590087891 6.999580383300781 17.34487342834473 7.095615386962891 17.43099975585938 7.563999176025391 L 18.18400001525879 11.63500022888184 C 18.18400001525879 11.63500022888184 18.3390007019043 12.07899951934814 18.86100006103516 12.07899951934814 L 20.94700050354004 12.07899951934814 C 20.94700050354004 12.07899951934814 21.01281356811523 12.06974983215332 21.11153221130371 12.06974983215332 C 21.40768814086914 12.06974983215332 22 12.15299987792969 22 12.81900024414063 L 22 17.4069995880127 C 22 17.4069995880127 22.00699996948242 17.99900054931641 21.24800109863281 17.99900054931641 C 21.24800109863281 17.99900054931641 21.0446662902832 17.9997386932373 20.69911003112793 17.9997386932373 C 19.24733543395996 17.9997386932373 15.28103828430176 17.9867000579834 13.29399967193604 17.85099983215332 C 10.47200012207031 17.65800094604492 3.663999557495117 17.70199966430664 2.76300048828125 14.66899967193604 L 2.01099967956543 10.3769998550415 C 2.01099967956543 10.3769998550415 1.805000305175781 9.376999855041504 3.214000701904297 9.118999481201172 L 16.52899932861328 7.045999526977539 C 16.52899932861328 7.045999526977539 16.65615272521973 6.999580383300781 16.81622695922852 6.999580383300781 Z"/></g><path class="d" d="M10.422,15.443H7.32v-5.6h.93v4.816h2.172Z" transform="translate(-2 -2)"/><path class="d" d="M11.23,15.443v-5.6h1.778A2.1,2.1,0,0,1,14.3,10.2a1.107,1.107,0,0,1,.475.926,1.319,1.319,0,0,1-.27.828,1.451,1.451,0,0,1-.746.5v.016a1.463,1.463,0,0,1,.924.428,1.3,1.3,0,0,1,.345.939A1.412,1.412,0,0,1,14.457,15a2.24,2.24,0,0,1-1.43.445Zm.926-4.855v1.594h.6a1.159,1.159,0,0,0,.76-.229.794.794,0,0,0,.275-.646q0-.719-.961-.719Zm0,2.34V14.7h.793a1.208,1.208,0,0,0,.8-.24.821.821,0,0,0,.283-.662q0-.871-1.207-.871Z" transform="translate(-2 -2)"/></svg>
```

### `xbox-rightjoystick-up.20cc1a34.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-rightjoystick-up.20cc1a34.svg` SHA-256`7b085eb402b33600c5ee28b6486f74e9565d375ac3ac448810970760925b3254`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#2e2e2e;}.b{fill:#44d62c;}.c{fill:#fff;}</style></defs><path class="a" d="M12,4a8,8,0,1,1-8,8,8,8,0,0,1,8-8Zm0-2A10,10,0,1,0,22,12,10.011,10.011,0,0,0,12,2Z" transform="translate(-2 -2)"/><path class="b" d="M12,2A9.969,9.969,0,0,0,4.93,4.93L6.34,6.34a8.015,8.015,0,0,1,11.32,0l1.41-1.41A9.969,9.969,0,0,0,12,2Z" transform="translate(-2 -2)"/><circle cx="6" cy="6" r="6" transform="translate(4 4)"/><path class="c" d="M15.162,15.443H13.805l-1.119-1.875a4.226,4.226,0,0,0-.3-.442,1.783,1.783,0,0,0-.293-.3,1,1,0,0,0-.325-.171,1.385,1.385,0,0,0-.4-.053h-.469v2.841H9.752v-7h2.305a3.3,3.3,0,0,1,.908.118,2.123,2.123,0,0,1,.723.354,1.642,1.642,0,0,1,.48.59,1.87,1.87,0,0,1,.174.828,2,2,0,0,1-.111.681,1.744,1.744,0,0,1-.312.552,1.938,1.938,0,0,1-.488.413,2.493,2.493,0,0,1-.642.263v.02a1.8,1.8,0,0,1,.332.237c.094.086.184.174.268.266a3.071,3.071,0,0,1,.252.31c.083.115.174.25.275.4ZM10.908,9.384v2.275h.967a1.438,1.438,0,0,0,.5-.083,1.137,1.137,0,0,0,.393-.239,1.078,1.078,0,0,0,.259-.381,1.3,1.3,0,0,0,.092-.5,1,1,0,0,0-.316-.786,1.33,1.33,0,0,0-.914-.283Z" transform="translate(-2 -2)"/></svg>
```

### `xbox-rightjoystick-right.6eb651a2.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-rightjoystick-right.6eb651a2.svg` SHA-256`2de48629611f974aefdaf981a16006a82f9cc253af28191b65cd7020da832aad`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#2e2e2e;}.b{fill:#44d62c;}.c{fill:#fff;}</style></defs><path class="a" d="M54,12a8,8,0,1,1-8-8,8,8,0,0,1,8,8Zm2,0A10,10,0,1,0,46,22,10.011,10.011,0,0,0,56,12Z" transform="translate(-36 -2)"/><path class="b" d="M56,12a9.969,9.969,0,0,0-2.93-7.07L51.66,6.34a8.015,8.015,0,0,1,0,11.32l1.41,1.41A9.969,9.969,0,0,0,56,12Z" transform="translate(-36 -2)"/><circle cx="6" cy="6" r="6" transform="translate(4 4)"/><path class="c" d="M49.162,15.443H47.805l-1.119-1.875a4.226,4.226,0,0,0-.3-.442,1.784,1.784,0,0,0-.293-.3,1,1,0,0,0-.325-.171,1.385,1.385,0,0,0-.4-.053h-.469v2.841H43.752v-7h2.305a3.3,3.3,0,0,1,.908.118,2.123,2.123,0,0,1,.723.354,1.642,1.642,0,0,1,.48.59,1.87,1.87,0,0,1,.174.828,2,2,0,0,1-.111.681,1.744,1.744,0,0,1-.312.552,1.938,1.938,0,0,1-.488.413,2.493,2.493,0,0,1-.642.263v.02a1.8,1.8,0,0,1,.332.237c.094.086.184.174.268.266a3.071,3.071,0,0,1,.252.31c.083.115.174.25.275.4ZM44.908,9.384v2.275h.967a1.438,1.438,0,0,0,.5-.083,1.137,1.137,0,0,0,.393-.239,1.078,1.078,0,0,0,.259-.381,1.3,1.3,0,0,0,.092-.5,1,1,0,0,0-.316-.786,1.33,1.33,0,0,0-.914-.283Z" transform="translate(-36 -2)"/></svg>
```

### `xbox-rightjoystick-left.57986288.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-rightjoystick-left.57986288.svg` SHA-256`9c8389c866d784434828601a7b263bc314e296d6dbf7e9a419c0ec6d70a1fac4`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#2e2e2e;}.b{fill:#44d62c;}.c{fill:#fff;}</style></defs><path class="a" d="M106,12a8,8,0,1,1,8,8,8,8,0,0,1-8-8Zm-2,0A10,10,0,1,0,114,2a10.011,10.011,0,0,0-10,10Z" transform="translate(-104 -2)"/><path class="b" d="M104,12a9.969,9.969,0,0,0,2.93,7.07l1.41-1.41a8.015,8.015,0,0,1,0-11.32l-1.41-1.41A9.969,9.969,0,0,0,104,12Z" transform="translate(-104 -2)"/><circle cx="6" cy="6" r="6" transform="translate(4 4)"/><path class="c" d="M117.162,15.443h-1.357l-1.119-1.875a4.225,4.225,0,0,0-.295-.442,1.784,1.784,0,0,0-.293-.3,1,1,0,0,0-.325-.171,1.385,1.385,0,0,0-.4-.053h-.469v2.841h-1.156v-7h2.305a3.3,3.3,0,0,1,.908.118,2.123,2.123,0,0,1,.723.354,1.642,1.642,0,0,1,.48.59,1.87,1.87,0,0,1,.174.828,2,2,0,0,1-.111.681,1.744,1.744,0,0,1-.312.552,1.938,1.938,0,0,1-.488.413,2.493,2.493,0,0,1-.642.263v.02a1.8,1.8,0,0,1,.332.237c.094.086.184.174.268.266a3.071,3.071,0,0,1,.252.31c.083.115.174.25.275.4Zm-4.254-6.059v2.275h.967a1.438,1.438,0,0,0,.5-.083,1.137,1.137,0,0,0,.393-.239,1.078,1.078,0,0,0,.259-.381,1.3,1.3,0,0,0,.092-.5,1,1,0,0,0-.316-.786,1.33,1.33,0,0,0-.914-.283Z" transform="translate(-104 -2)"/></svg>
```

### `xbox-rightjoystick-down.bb1eccbf.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-rightjoystick-down.bb1eccbf.svg` SHA-256`1336da81181ea936b41665df0878a944a3da4bc218556261e5c4fc66c66cd6d5`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#2e2e2e;}.b{fill:#44d62c;}.c{fill:#fff;}</style></defs><path class="a" d="M80,20a8,8,0,1,1,8-8,8,8,0,0,1-8,8Zm0,2A10,10,0,1,0,70,12,10.011,10.011,0,0,0,80,22Z" transform="translate(-70 -2)"/><path class="b" d="M80,22a9.969,9.969,0,0,0,7.07-2.93l-1.41-1.41a8.015,8.015,0,0,1-11.32,0l-1.41,1.41A9.969,9.969,0,0,0,80,22Z" transform="translate(-70 -2)"/><circle cx="6" cy="6" r="6" transform="translate(4 4)"/><path class="c" d="M83.162,15.443H81.805l-1.119-1.875a4.225,4.225,0,0,0-.295-.442,1.784,1.784,0,0,0-.293-.3,1,1,0,0,0-.325-.171,1.385,1.385,0,0,0-.4-.053h-.469v2.841H77.752v-7h2.305a3.3,3.3,0,0,1,.908.118,2.123,2.123,0,0,1,.723.354,1.642,1.642,0,0,1,.48.59,1.87,1.87,0,0,1,.174.828,2,2,0,0,1-.111.681,1.744,1.744,0,0,1-.312.552,1.938,1.938,0,0,1-.488.413,2.493,2.493,0,0,1-.642.263v.02a1.8,1.8,0,0,1,.332.237c.094.086.184.174.268.266a3.071,3.071,0,0,1,.252.31c.083.115.174.25.275.4ZM78.908,9.384v2.275h.967a1.438,1.438,0,0,0,.5-.083,1.137,1.137,0,0,0,.393-.239,1.078,1.078,0,0,0,.259-.381,1.3,1.3,0,0,0,.092-.5,1,1,0,0,0-.316-.786,1.33,1.33,0,0,0-.914-.283Z" transform="translate(-70 -2)"/></svg>
```

### `xbox-rightjoystick-press.72ccb46d.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-rightjoystick-press.72ccb46d.svg` SHA-256`384a861dd9e907ae7f16a67e20a9b27c68c5c076eff5cf1b1a733f8dbeb72487`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#2e2e2e;}.b{fill:#44d62c;}</style></defs><path class="a" d="M12,4a8,8,0,1,1-8,8,8,8,0,0,1,8-8Zm0-2A10,10,0,1,0,22,12,10.011,10.011,0,0,0,12,2Z" transform="translate(-2 -2)"/><circle class="b" cx="6" cy="6" r="6" transform="translate(4 4)"/><path d="M15.162,15.443H13.805l-1.119-1.875a4.226,4.226,0,0,0-.3-.442,1.783,1.783,0,0,0-.293-.3,1,1,0,0,0-.325-.171,1.385,1.385,0,0,0-.4-.053h-.469v2.841H9.752v-7h2.305a3.3,3.3,0,0,1,.908.118,2.123,2.123,0,0,1,.723.354,1.642,1.642,0,0,1,.48.59,1.87,1.87,0,0,1,.174.828,2,2,0,0,1-.111.681,1.744,1.744,0,0,1-.312.552,1.938,1.938,0,0,1-.488.413,2.493,2.493,0,0,1-.642.263v.02a1.8,1.8,0,0,1,.332.237c.094.086.184.174.268.266a3.071,3.071,0,0,1,.252.31c.083.115.174.25.275.4ZM10.908,9.384v2.275h.967a1.438,1.438,0,0,0,.5-.083,1.137,1.137,0,0,0,.393-.239,1.078,1.078,0,0,0,.259-.381,1.3,1.3,0,0,0,.092-.5,1,1,0,0,0-.316-.786,1.33,1.33,0,0,0-.914-.283Z" transform="translate(-2 -2)"/></svg>
```

### `xbox-leftjoystick-up.dea6602a.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-leftjoystick-up.dea6602a.svg` SHA-256`344c13b525f915a90fdc7b87686477687429bfed9d86c1a5f5355e37d79fba84`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#2e2e2e;}.b{fill:#44d62c;}.c{fill:#fff;}</style></defs><path class="a" d="M46,4a8,8,0,1,1-8,8,8,8,0,0,1,8-8Zm0-2A10,10,0,1,0,56,12,10.011,10.011,0,0,0,46,2Z" transform="translate(-36 -2)"/><path class="b" d="M46,2a9.969,9.969,0,0,0-7.07,2.93l1.41,1.41a8.015,8.015,0,0,1,11.32,0l1.41-1.41A9.969,9.969,0,0,0,46,2Z" transform="translate(-36 -2)"/><circle cx="6" cy="6" r="6" transform="translate(4 4)"/><path class="c" d="M48.377,15.5H44.5v-7h1.162V14.52h2.715Z" transform="translate(-36 -2)"/></svg>
```

### `xbox-leftjoystick-right.d7503399.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-leftjoystick-right.d7503399.svg` SHA-256`cad0c67a0465694e95531e86ae3f9f524f36cdadd2e1a2272123347b73612001`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#2e2e2e;}.b{fill:#44d62c;}.c{fill:#fff;}</style></defs><path class="a" d="M-82,12a8,8,0,0,1-8,8,8,8,0,0,1-8-8,8,8,0,0,1,8-8,8,8,0,0,1,8,8Zm2,0A10.011,10.011,0,0,0-90,2a10.011,10.011,0,0,0-10,10A10.011,10.011,0,0,0-90,22,10.011,10.011,0,0,0-80,12Z" transform="translate(100 -2)"/><path class="b" d="M-80,12a9.969,9.969,0,0,0-2.93-7.07l-1.41,1.41A8.015,8.015,0,0,1-82,12a8.015,8.015,0,0,1-2.34,5.66l1.41,1.41A9.969,9.969,0,0,0-80,12Z" transform="translate(100 -2)"/><circle cx="6" cy="6" r="6" transform="translate(4 4)"/><path class="c" d="M-87.623,15.5H-91.5v-7h1.162V14.52h2.715Z" transform="translate(100 -2)"/></svg>
```

### `xbox-leftjoystick-left.bd19a0fe.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-leftjoystick-left.bd19a0fe.svg` SHA-256`530b6c6eb6a3d7c3f85a9b3e0ac723139468de2358912cbfe4f581a1519a04e4`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#2e2e2e;}.b{fill:#44d62c;}.c{fill:#fff;}</style></defs><path class="a" d="M-30,12a8,8,0,0,1,8-8,8,8,0,0,1,8,8,8,8,0,0,1-8,8,8,8,0,0,1-8-8Zm-2,0A10.011,10.011,0,0,0-22,22,10.011,10.011,0,0,0-12,12,10.011,10.011,0,0,0-22,2,10.011,10.011,0,0,0-32,12Z" transform="translate(32 -2)"/><path class="b" d="M-32,12a9.969,9.969,0,0,0,2.93,7.07l1.41-1.41A8.014,8.014,0,0,1-30,12a8.014,8.014,0,0,1,2.34-5.66l-1.41-1.41A9.969,9.969,0,0,0-32,12Z" transform="translate(32 -2)"/><circle cx="6" cy="6" r="6" transform="translate(4 4)"/><path class="c" d="M-19.623,15.5H-23.5v-7h1.162V14.52h2.715Z" transform="translate(32 -2)"/></svg>
```

### `xbox-leftjoystick-down.8a28a654.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-leftjoystick-down.8a28a654.svg` SHA-256`9821f034701fe97e31c8397a6e67c4959843cd7d2c3bd904d5c249017cdf8983`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#2e2e2e;}.b{fill:#44d62c;}.c{fill:#fff;}</style></defs><path class="a" d="M-56,20a8,8,0,0,1-8-8,8,8,0,0,1,8-8,8,8,0,0,1,8,8,8,8,0,0,1-8,8Zm0,2A10.011,10.011,0,0,0-46,12,10.011,10.011,0,0,0-56,2,10.011,10.011,0,0,0-66,12,10.011,10.011,0,0,0-56,22Z" transform="translate(66 -2)"/><path class="b" d="M-56,22a9.969,9.969,0,0,0,7.07-2.93l-1.41-1.41A8.014,8.014,0,0,1-56,20a8.014,8.014,0,0,1-5.66-2.34l-1.41,1.41A9.969,9.969,0,0,0-56,22Z" transform="translate(66 -2)"/><circle cx="6" cy="6" r="6" transform="translate(4 4)"/><path class="c" d="M-53.623,15.5H-57.5v-7h1.162V14.52h2.715Z" transform="translate(66 -2)"/></svg>
```

### `xbox-leftjoystick-press.a58c272c.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-leftjoystick-press.a58c272c.svg` SHA-256`1c76826b6ae8b2fb0a8f583e103da0d5c23c6ec0980a0e4c0984fd0c1d7dbf56`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#2e2e2e;}.b{fill:#44d62c;}</style></defs><path class="a" d="M-124,38a8,8,0,0,1,8,8,8,8,0,0,1-8,8,8,8,0,0,1-8-8,8,8,0,0,1,8-8Zm0-2a10.011,10.011,0,0,0-10,10,10.011,10.011,0,0,0,10,10,10.011,10.011,0,0,0,10-10,10.011,10.011,0,0,0-10-10Z" transform="translate(134 -36)"/><circle class="b" cx="6" cy="6" r="6" transform="translate(4 4)"/><path d="M-121.623,49.5H-125.5v-7h1.162V48.52h2.715Z" transform="translate(134 -36)"/></svg>
```

### `xbox-menu.029cb487.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-menu.029cb487.svg` SHA-256`0639884b9e2abdc41aeb20ed1927bb463bccc7fc1a3ca1dd3f151221565fc993`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a,.d{fill:none;}.a{stroke:#2e2e2e;stroke-miterlimit:10;}.b{fill:#fff;}.c{stroke:none;}</style></defs><circle cx="10" cy="10" r="10"/><g class="a"><circle class="c" cx="10" cy="10" r="10"/><circle class="d" cx="10" cy="10" r="9.5"/></g><path class="b" d="M-187,15h-10V14h10Zm0-4h-10v1h10Zm0-3h-10V9h10Z" transform="translate(202 -2)"/></svg>
```

### `xbox-view.771d8231.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-view.771d8231.svg` SHA-256`34a03041ecf88afa8b365f1787c22fbba7394e2139b02982daaac4ba3e58eea5`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a,.d{fill:none;}.a{stroke:#2e2e2e;stroke-miterlimit:10;}.b{fill:#fff;}.c{stroke:none;}</style></defs><circle cx="10" cy="10" r="10"/><g class="a"><circle class="c" cx="10" cy="10" r="10"/><circle class="d" cx="10" cy="10" r="9.5"/></g><path class="b" d="M18,17H10V11h8Zm-7-1h6V12H11ZM6,7v6H8V12H7V8h6V9h1V7Z" transform="translate(-2 -2)"/></svg>
```

### `xbox-dpad-up.8f32398c.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-dpad-up.8f32398c.svg` SHA-256`c3f635031a47e6936256894574061d22a99619fab0456fd229def73805b086be`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#222;opacity:0;}.b{fill:none;stroke-miterlimit:10;}.c{fill:#44d62c;}.d,.e{stroke:none;}.e{fill:#2e2e2e;}</style></defs><rect class="a" width="20" height="20"/><path d="M20,9H16a1,1,0,0,1-1-1V4a1,1,0,0,0-1-1H10A1,1,0,0,0,9,4V8A1,1,0,0,1,8,9H4a1,1,0,0,0-1,1v4a1,1,0,0,0,1,1H8a1,1,0,0,1,1,1v4a1,1,0,0,0,1,1h4a1,1,0,0,0,1-1V16a1,1,0,0,1,1-1h4a1,1,0,0,0,1-1V10A1,1,0,0,0,20,9Z" transform="translate(-2 -2)"/><g class="b" transform="translate(-2 -2)"><path class="d" d="M20,9H16a1,1,0,0,1-1-1V4a1,1,0,0,0-1-1H10A1,1,0,0,0,9,4V8A1,1,0,0,1,8,9H4a1,1,0,0,0-1,1v4a1,1,0,0,0,1,1H8a1,1,0,0,1,1,1v4a1,1,0,0,0,1,1h4a1,1,0,0,0,1-1V16a1,1,0,0,1,1-1h4a1,1,0,0,0,1-1V10A1,1,0,0,0,20,9Z"/><path class="e" d="M 10.00171184539795 4 C 10.00117111206055 4.00037956237793 10.00064563751221 4.000724792480469 10.000319480896 4.000724792480469 C 10.00015258789063 4.000724792480469 10.00003814697266 4.000635147094727 10 4.000410079956055 L 10 8 C 10 9.102800369262695 9.102800369262695 10 8 10 L 4.002920150756836 9.999199867248535 C 4.002119064331055 9.999621391296387 4.00090217590332 10.00072574615479 4.000320434570313 10.00072574615479 C 4.000152587890625 10.00072574615479 4.000038146972656 10.00063419342041 4 10.00041103363037 L 3.999200820922852 13.99707984924316 C 3.99974250793457 13.9981107711792 4.001413345336914 13.99982929229736 4.000411987304688 14 L 8 14 C 9.102800369262695 14 10 14.8971996307373 10 16 L 9.999199867248535 19.99707984924316 C 9.99974250793457 19.99811172485352 10.00141429901123 19.99983024597168 10.00041103363037 20 L 13.99707984924316 20.00079917907715 C 13.99788188934326 20.0003776550293 13.99909782409668 19.99927520751953 13.999680519104 19.99927520751953 C 13.99984741210938 19.99927520751953 13.99996185302734 19.99936485290527 14 19.99958801269531 L 14 16 C 14 14.8971996307373 14.8971996307373 14 16 14 L 19.99707984924316 14.00080013275146 C 19.99788093566895 14.00037860870361 19.99909782409668 13.99927425384521 19.99967956542969 13.99927425384521 C 19.99984741210938 13.99927425384521 19.99996185302734 13.99936580657959 20 13.99958896636963 L 20.00079917907715 10.00292015075684 C 20.00025749206543 10.0018892288208 19.99858665466309 10.00017070770264 19.99958801269531 10 L 16 10 C 14.8971996307373 10 14 9.102800369262695 14 8 L 14.00080013275146 4.002920150756836 C 14.00025749206543 4.001888275146484 13.99858570098877 4.000171661376953 13.99958896636963 4 L 10.00171184539795 4 M 10 3 L 14 3 C 14.55000019073486 3 15 3.450000762939453 15 4 L 15 8 C 15 8.552000045776367 15.44799995422363 9 16 9 L 20 9 C 20.54999923706055 9 21 9.449999809265137 21 10 L 21 14 C 21 14.55000019073486 20.54999923706055 15 20 15 L 16 15 C 15.44799995422363 15 15 15.44799995422363 15 16 L 15 20 C 15 20.54999923706055 14.55000019073486 21 14 21 L 10 21 C 9.449999809265137 21 9 20.54999923706055 9 20 L 9 16 C 9 15.44799995422363 8.552000045776367 15 8 15 L 4 15 C 3.450000762939453 15 3 14.55000019073486 3 14 L 3 10 C 3 9.449999809265137 3.450000762939453 9 4 9 L 8 9 C 8.552000045776367 9 9 8.552000045776367 9 8 L 9 4 C 9 3.450000762939453 9.449999809265137 3 10 3 Z"/></g><path class="c" d="M0,0H4A0,0,0,0,1,4,0V3A1,1,0,0,1,3,4H1A1,1,0,0,1,0,3V0A0,0,0,0,1,0,0Z" transform="translate(8 2)"/></svg>
```

### `xbox-dpad-right.34fa065b.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-dpad-right.34fa065b.svg` SHA-256`3d71a6d995fbb27ae402e6524fcce2a62230516c166caf0dd0f29c3877868f0f`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#222;opacity:0;}.b{fill:none;stroke-miterlimit:10;}.c{fill:#44d62c;}.d,.e{stroke:none;}.e{fill:#2e2e2e;}</style></defs><rect class="a" width="20" height="20"/><path d="M-53,20V16a1,1,0,0,1,1-1h4a1,1,0,0,0,1-1V10a1,1,0,0,0-1-1h-4a1,1,0,0,1-1-1V4a1,1,0,0,0-1-1h-4a1,1,0,0,0-1,1V8a1,1,0,0,1-1,1h-4a1,1,0,0,0-1,1v4a1,1,0,0,0,1,1h4a1,1,0,0,1,1,1v4a1,1,0,0,0,1,1h4A1,1,0,0,0-53,20Z" transform="translate(66 -2)"/><g class="b" transform="translate(66 -2)"><path class="d" d="M-53,20V16a1,1,0,0,1,1-1h4a1,1,0,0,0,1-1V10a1,1,0,0,0-1-1h-4a1,1,0,0,1-1-1V4a1,1,0,0,0-1-1h-4a1,1,0,0,0-1,1V8a1,1,0,0,1-1,1h-4a1,1,0,0,0-1,1v4a1,1,0,0,0,1,1h4a1,1,0,0,1,1,1v4a1,1,0,0,0,1,1h4A1,1,0,0,0-53,20Z"/><path class="e" d="M -54.00171279907227 20 C -54.00117111206055 19.99962043762207 -54.00064468383789 19.99927520751953 -54.00032043457031 19.99927520751953 C -54.00015258789063 19.99927520751953 -54.00003814697266 19.99936485290527 -54 19.99958992004395 L -54 16 C -54 14.8971996307373 -53.10279846191406 14 -52 14 L -48.0029182434082 14.00080013275146 C -48.00211715698242 14.00037860870361 -48.00090026855469 13.99927425384521 -48.00032043457031 13.99927425384521 C -48.00015258789063 13.99927425384521 -48.00003814697266 13.99936580657959 -48 13.99958896636963 L -47.99919891357422 10.00292015075684 C -47.9997444152832 10.0018892288208 -48.00141525268555 10.00017070770264 -48.00041198730469 10 L -52 10 C -53.10279846191406 10 -54 9.102800369262695 -54 8 L -53.99919891357422 4.002920150756836 C -53.9997444152832 4.001888751983643 -54.00141525268555 4.000170230865479 -54.00041198730469 4 L -57.9970817565918 3.999200105667114 C -57.99788284301758 3.999621868133545 -57.99909973144531 4.000725746154785 -57.99967956542969 4.000725746154785 C -57.99984741210938 4.000725746154785 -57.99996185302734 4.000634670257568 -58 4.000411033630371 L -58 8 C -58 9.102800369262695 -58.89720153808594 10 -60 10 L -63.9970817565918 9.999199867248535 C -63.99788284301758 9.999621391296387 -63.99909973144531 10.00072574615479 -63.99967956542969 10.00072574615479 C -63.99984741210938 10.00072574615479 -63.99996185302734 10.00063419342041 -64 10.00041103363037 L -64.00080108642578 13.99707984924316 C -64.00025939941406 13.9981107711792 -63.99858474731445 13.99982929229736 -63.99958801269531 14 L -60 14 C -58.89720153808594 14 -58 14.8971996307373 -58 16 L -58.00080108642578 19.99707984924316 C -58.0002555847168 19.99811172485352 -57.99858474731445 19.99982833862305 -57.99958801269531 20 L -54.00171279907227 20 M -54 21 L -58 21 C -58.54999923706055 21 -59 20.54999923706055 -59 20 L -59 16 C -59 15.44799995422363 -59.44800186157227 15 -60 15 L -64 15 C -64.55000305175781 15 -65 14.55000019073486 -65 14 L -65 10 C -65 9.449999809265137 -64.55000305175781 9 -64 9 L -60 9 C -59.44800186157227 9 -59 8.552000045776367 -59 8 L -59 4 C -59 3.450000047683716 -58.54999923706055 3 -58 3 L -54 3 C -53.45000076293945 3 -53 3.450000047683716 -53 4 L -53 8 C -53 8.552000045776367 -52.55199813842773 9 -52 9 L -48 9 C -47.45000076293945 9 -47 9.449999809265137 -47 10 L -47 14 C -47 14.55000019073486 -47.45000076293945 15 -48 15 L -52 15 C -52.55199813842773 15 -53 15.44799995422363 -53 16 L -53 20 C -53 20.54999923706055 -53.45000076293945 21 -54 21 Z"/></g><path class="c" d="M0,0H4A0,0,0,0,1,4,0V3A1,1,0,0,1,3,4H1A1,1,0,0,1,0,3V0A0,0,0,0,1,0,0Z" transform="translate(18 8) rotate(90)"/></svg>
```

### `xbox-dpad-down.b22e39fe.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-dpad-down.b22e39fe.svg` SHA-256`6430f2efc4872dd726a6e57f5b1878a84c00fbbafe1c77d9f705100ce57a2af5`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#222;opacity:0;}.b{fill:none;stroke-miterlimit:10;}.c{fill:#44d62c;}.d,.e{stroke:none;}.e{fill:#2e2e2e;}</style></defs><rect class="a" width="20" height="20"/><path d="M4,15H8a1,1,0,0,1,1,1v4a1,1,0,0,0,1,1h4a1,1,0,0,0,1-1V16a1,1,0,0,1,1-1h4a1,1,0,0,0,1-1V10a1,1,0,0,0-1-1H16a1,1,0,0,1-1-1V4a1,1,0,0,0-1-1H10A1,1,0,0,0,9,4V8A1,1,0,0,1,8,9H4a1,1,0,0,0-1,1v4A1,1,0,0,0,4,15Z" transform="translate(-2 -2)"/><g class="b" transform="translate(-2 -2)"><path class="d" d="M4,15H8a1,1,0,0,1,1,1v4a1,1,0,0,0,1,1h4a1,1,0,0,0,1-1V16a1,1,0,0,1,1-1h4a1,1,0,0,0,1-1V10a1,1,0,0,0-1-1H16a1,1,0,0,1-1-1V4a1,1,0,0,0-1-1H10A1,1,0,0,0,9,4V8A1,1,0,0,1,8,9H4a1,1,0,0,0-1,1v4A1,1,0,0,0,4,15Z"/><path class="e" d="M 13.99828815460205 20 C 13.99882888793945 19.99962043762207 13.99935436248779 19.99927520751953 13.999680519104 19.99927520751953 C 13.99984741210938 19.99927520751953 13.99996185302734 19.99936485290527 14 19.99958992004395 L 14 16 C 14 14.8971996307373 14.8971996307373 14 16 14 L 19.99707984924316 14.00080013275146 C 19.99788093566895 14.00037860870361 19.99909782409668 13.99927425384521 19.99967956542969 13.99927425384521 C 19.99984741210938 13.99927425384521 19.99996185302734 13.99936580657959 20 13.99958896636963 L 20.00079917907715 10.00292015075684 C 20.00025749206543 10.0018892288208 19.99858665466309 10.00017070770264 19.99958801269531 10 L 16 10 C 14.8971996307373 10 14 9.102800369262695 14 8 L 14.00080013275146 4.002920150756836 C 14.00025749206543 4.001888751983643 13.99858570098877 4.000170230865479 13.99958896636963 4 L 10.00292015075684 3.999200105667114 C 10.00211811065674 3.999621868133545 10.00090217590332 4.000725746154785 10.000319480896 4.000725746154785 C 10.00015258789063 4.000725746154785 10.00003814697266 4.000634670257568 10 4.000411033630371 L 10 8 C 10 9.102800369262695 9.102800369262695 10 8 10 L 4.002920150756836 9.999199867248535 C 4.002118587493896 9.999621391296387 4.000901699066162 10.00072574615479 4.000319480895996 10.00072574615479 C 4.000152587890625 10.00072574615479 4.000037670135498 10.00063419342041 4 10.00041103363037 L 3.999200105667114 13.99707984924316 C 3.999742746353149 13.9981107711792 4.001413822174072 13.99982929229736 4.000411510467529 14 L 8 14 C 9.102800369262695 14 10 14.8971996307373 10 16 L 9.999199867248535 19.99707984924316 C 9.99974250793457 19.99811172485352 10.00141429901123 19.99982833862305 10.00041103363037 20 L 13.99828815460205 20 M 14 21 L 10 21 C 9.449999809265137 21 9 20.54999923706055 9 20 L 9 16 C 9 15.44799995422363 8.552000045776367 15 8 15 L 4 15 C 3.450000047683716 15 3 14.55000019073486 3 14 L 3 10 C 3 9.449999809265137 3.450000047683716 9 4 9 L 8 9 C 8.552000045776367 9 9 8.552000045776367 9 8 L 9 4 C 9 3.450000047683716 9.449999809265137 3 10 3 L 14 3 C 14.55000019073486 3 15 3.450000047683716 15 4 L 15 8 C 15 8.552000045776367 15.44799995422363 9 16 9 L 20 9 C 20.54999923706055 9 21 9.449999809265137 21 10 L 21 14 C 21 14.55000019073486 20.54999923706055 15 20 15 L 16 15 C 15.44799995422363 15 15 15.44799995422363 15 16 L 15 20 C 15 20.54999923706055 14.55000019073486 21 14 21 Z"/></g><path class="c" d="M0,0H4A0,0,0,0,1,4,0V3A1,1,0,0,1,3,4H1A1,1,0,0,1,0,3V0A0,0,0,0,1,0,0Z" transform="translate(12 18) rotate(180)"/></svg>
```

### `xbox-dpad-left.91e17572.svg`

`local-ui-reverse/source/official/apps.razer.com/synapse/products/679/ui/static/media/xbox-dpad-left.91e17572.svg` SHA-256`4fd1e1703479381fe03700a1beeeb295199e1b6e498f1436a019486ccac0961d`

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 20 20"><defs><style>.a{fill:#222;opacity:0;}.b{fill:none;stroke-miterlimit:10;}.c{fill:#44d62c;}.d,.e{stroke:none;}.e{fill:#2e2e2e;}</style></defs><rect class="a" width="20" height="20"/><path d="M9,4V8A1,1,0,0,1,8,9H4a1,1,0,0,0-1,1v4a1,1,0,0,0,1,1H8a1,1,0,0,1,1,1v4a1,1,0,0,0,1,1h4a1,1,0,0,0,1-1V16a1,1,0,0,1,1-1h4a1,1,0,0,0,1-1V10a1,1,0,0,0-1-1H16a1,1,0,0,1-1-1V4a1,1,0,0,0-1-1H10A1,1,0,0,0,9,4Z" transform="translate(-2 -2)"/><g class="b" transform="translate(-2 -2)"><path class="d" d="M9,4V8A1,1,0,0,1,8,9H4a1,1,0,0,0-1,1v4a1,1,0,0,0,1,1H8a1,1,0,0,1,1,1v4a1,1,0,0,0,1,1h4a1,1,0,0,0,1-1V16a1,1,0,0,1,1-1h4a1,1,0,0,0,1-1V10a1,1,0,0,0-1-1H16a1,1,0,0,1-1-1V4a1,1,0,0,0-1-1H10A1,1,0,0,0,9,4Z"/><path class="e" d="M 10.00171184539795 4 C 10.00117111206055 4.00037956237793 10.00064563751221 4.000724792480469 10.000319480896 4.000724792480469 C 10.00015258789063 4.000724792480469 10.00003814697266 4.000635147094727 10 4.000410079956055 L 10 8 C 10 9.102800369262695 9.102800369262695 10 8 10 L 4.002920150756836 9.999199867248535 C 4.002119064331055 9.999621391296387 4.00090217590332 10.00072574615479 4.000320434570313 10.00072574615479 C 4.000152587890625 10.00072574615479 4.000038146972656 10.00063419342041 4 10.00041103363037 L 3.999200820922852 13.99707984924316 C 3.99974250793457 13.9981107711792 4.001413345336914 13.99982929229736 4.000411987304688 14 L 8 14 C 9.102800369262695 14 10 14.8971996307373 10 16 L 9.999199867248535 19.99707984924316 C 9.99974250793457 19.99811172485352 10.00141429901123 19.99983024597168 10.00041103363037 20 L 13.99707984924316 20.00079917907715 C 13.99788188934326 20.0003776550293 13.99909782409668 19.99927520751953 13.999680519104 19.99927520751953 C 13.99984741210938 19.99927520751953 13.99996185302734 19.99936485290527 14 19.99958801269531 L 14 16 C 14 14.8971996307373 14.8971996307373 14 16 14 L 19.99707984924316 14.00080013275146 C 19.99788093566895 14.00037860870361 19.99909782409668 13.99927425384521 19.99967956542969 13.99927425384521 C 19.99984741210938 13.99927425384521 19.99996185302734 13.99936580657959 20 13.99958896636963 L 20.00079917907715 10.00292015075684 C 20.00025749206543 10.0018892288208 19.99858665466309 10.00017070770264 19.99958801269531 10 L 16 10 C 14.8971996307373 10 14 9.102800369262695 14 8 L 14.00080013275146 4.002920150756836 C 14.00025749206543 4.001888275146484 13.99858570098877 4.000171661376953 13.99958896636963 4 L 10.00171184539795 4 M 10 3 L 14 3 C 14.55000019073486 3 15 3.450000762939453 15 4 L 15 8 C 15 8.552000045776367 15.44799995422363 9 16 9 L 20 9 C 20.54999923706055 9 21 9.449999809265137 21 10 L 21 14 C 21 14.55000019073486 20.54999923706055 15 20 15 L 16 15 C 15.44799995422363 15 15 15.44799995422363 15 16 L 15 20 C 15 20.54999923706055 14.55000019073486 21 14 21 L 10 21 C 9.449999809265137 21 9 20.54999923706055 9 20 L 9 16 C 9 15.44799995422363 8.552000045776367 15 8 15 L 4 15 C 3.450000762939453 15 3 14.55000019073486 3 14 L 3 10 C 3 9.449999809265137 3.450000762939453 9 4 9 L 8 9 C 8.552000045776367 9 9 8.552000045776367 9 8 L 9 4 C 9 3.450000762939453 9.449999809265137 3 10 3 Z"/></g><path class="c" d="M0,0H4A0,0,0,0,1,4,0V3A1,1,0,0,1,3,4H1A1,1,0,0,1,0,3V0A0,0,0,0,1,0,0Z" transform="translate(2 12) rotate(-90)"/></svg>
```

## Parent verification, 2026-10-11

Current source receipts: 5393 exact slices across 286 current source files validated. Resource registration independently checked 30 shared floating-controller originals across three products and 42 artwork source/display assets. `cargo check --locked --all-targets --offline` passed with warnings. Pure Rust tests passed: floating/controller-drop 7, mouse button-state 5, source artwork 3 (15 distinct tests). No application, vendor JavaScript, DLL, helper, installer or real device operation was executed; visual/runtime and real persistence acceptance remain absent.
