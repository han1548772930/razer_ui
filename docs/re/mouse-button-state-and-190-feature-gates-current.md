# Current mouse button state and PID190 feature gates

This review uses current acquired product source only. The maintained tools parse
JavaScript text with Acorn, without evaluating vendor code. Original file hashes,
UTF16 ranges, UTF8 byte ranges and exact source slices are in
`mouse-button-state-current-source.json` and `mouse-190-matcher-current-source.json`.
Offsets are zero-based and end-exclusive. No application, DLL, helper, installer,
real-device operation or downloaded JavaScript was executed.

## Button enablement mismatch and implementation

The generic Rust Customize page previously read descriptor `isEnabled` directly
for both its row disabled state and editor-supported predicate. Its descriptors
are not the result of the source mapping-state producer: for example, LeftClick
remains disabled when HyperShift is enabled and when another primary click exists.

PID163 source is
`local-ui-reverse/source/official/apps.razer.com/synapse/products/163/ui/static/js/main.efd0b513.js`,
SHA256 `74c2debf248fb2a9ca349e7f8b6ccfa2cbfbcf06479499b8f754d33b9b074d97`.
The normal page uses class EP beginning at 4811860; its connected wrapper mounts
EP at 4839835. The armory page also has class Uh beginning at 4979002, connected
at 4998067. Both variants are retained separately in the receipt.

| Source function, normal page | UTF16 range | Relevant behavior |
| --- | --- | --- |
| `loadFilteredMappings` | 4819335–4820829 | Filters current layer by boolean coercion; loads original rows for all views; chooses ordinary/New from `attachmentInfo.hasAttachement` and mouse category. |
| `updateMappingsToButtons` | 4821885–4823207 | Clears current rows, assigns mappings, derives enabled state and protects a remaining standard primary. |
| `updateMappingsToButtonsNew` | 4823240–4824806 | Uses original all-view LeftClick and other side-panel mappings for attachment primary protection. |
| `clearButtonValues` | 4827283–4828296 | Mouse rows reset mapped=false, enabled=true, assignmentValue=defaultValue; clears assignmentGroup and secondary fields. |
| `updateButtonWithMappings` | 4824837–4825027 | Nested first mapping writes assignment, later mappings write secondaryAssignment, using outer inputID. |
| `assignMappingGroup` | 4825052–4827105 | HyperShift output on HyperShift layer disables row; default output leaves cleared assignment; dispatches real output producer. |
| `assignMouseGroup` | 4814658–4814769 | Mapped=true, assignment=MOUSE_FUNCTION; source enum translation supplies assignmentValue. |

The ordinary source starts LeftClick enabled from `isHyperShiftOn`. It then
enables it when `viewIndex>=0 && assignmentValue!=="LeftClick"`; this comparison
is intentionally case-sensitive and uses a button-key literal, while normal
defaultValue is `LEFT_CLICK`. After assigning, any standard row whose
assignmentValue is `LEFT_CLICK` enables the original LeftClick. Finally:

- All rows with assignmentValue `LEFT_CLICK` start enabled.
- `mappingList.filter(m=>m.isHyperShift===false)` is strict false, whereas the
  earlier layer selection used boolean coercion. Missing flags are not counted.
- `ke.wP` counts only top-level `mouseGroup.mouseAssignment==="Click"`.
  It does not flatten nested mapping arrays.
- `ke.j4` observes any top-level inputID `LeftClick`, regardless of its output.
- With a positive explicit primary count and a LeftClick input mapping, exactly
  one count and one derived primary row disables that row.
- With zero explicit count, one derived primary and that row's buttonKey
  LeftClick, the row is disabled.

`ke.P_` finds matching inputID by strict equality. In module1867, V's expression
is [122513,122571), Y [122574,122759), and W [122762,123010).
Module4693 exports `VjA` to Xa, whose literal at 231033 is `LEFT_CLICK`.
Button loader module1350 concatenates each groupList entry's group.buttonList.

`tools/audit-mouse-button-state-current.cjs` checks 76 products, preserving 69
products with the source methods and their literal mouse/sensitivity enum tables.
The generated `mouse_button_state_current_data.json` carries source hashes,
method ranges and capabilities. Rust contains no product-ID eligibility list.
Missing PID162 source and PID222's data/source hash mismatch are explicit gaps.
PID199/208/221/226/235 did not expose this method family in their current main and
are not silently assigned a policy. The 69 verified products include the
separate PID70/PID190 surfaces; the generic Rust caller covers the other 67.

`mouse_button_state_current.rs::derive` is a pure non-attachment state projection.
It returns cloned rows with enabled/mapped/assignment fields for the current
mapping layer. It implements source primary helpers, clearing and nested
secondary assignment, empty-first-payload skip, mouse/sensitivity translations,
default/disable/HyperShift and directly available macro/text/launch/AI values.
It performs no save, transport or persistence.

The attachment/New branch needs real sidePanelMappings observations beyond the
six-argument interface and returns `AttachmentObservationRequired`. Other known
output producers not yet implemented return `UnresolvedAssignmentProducer`.
Those errors are gaps; they must not be converted into fabricated successful
observations. `isRequireSynapse` and full default assignment label restoration
are outside this projection and must retain their separate source producers.
The Rust UI bridge is owned by the parent task and is not claimed complete by
this review. Five focused pure tests were added; this child ran rustfmt only,
and leaves Cargo verification to the parent task.

The follow-up producer audit retains 2279 slices including all eight supported
assignment producers and their label constants. PID163 exact normal-page bodies
are macro at 4815058, text at 4818806, launch at 4817455, AI at 4817594,
sensitivity at 4814912, disable at 4819063 and HyperShift at 4817322; the receipt
contains their parsed expression boundaries for each product. Launch's actual
assignment value is `void 0!==payload.path ? payload.path : payload.url`; it does
not extract a file name. A present null path stays null. AI's value is exactly
`payload.assignment`. Macro uses a truthy name or one literal space. Text uses
`payload.text`; HyperShift uses `Razer Hypershift`; Disable uses `DISABLE`.
Sensitivity uses the source lookup and preserves unknown IDs. These source values
are distinct from later label-rendering or localization transformations.
The generated per-product verified_producers list requires both the real producer
value expression and its resolved label constant; PID131/143/148/185 currently
retain unresolved split-label module gaps and return an explicit producer error
for assigned rows instead of borrowing another product's labels.

## PID190 Performance missing sensitivity matcher

Current source is
`local-ui-reverse/source/official/apps.razer.com/synapse/products/190/ui/static/js/main.81b09779.js`,
SHA256 `4c6d60b23bebcc9423a561aa86ebf6ed24cd9cbe0938aa13d7fac46022a4dc2f`.
`uS` [4619705,4620009) always mounts sensitivity matcher NS after polling.
Only polling is conditioned on `isBle`; matcher dS's root return begins at
4617041 and has no visibility capability gate. Rust's PID190 Performance right
column contains only optional polling, so matcher is an implementation omission.

The matcher receipt contains full mounted classes AS, dS, uS, their state
connectors, initial state Tt, reducer Ot, action creators, CSS and resolved
localization keys. The source behavior includes:

- Mount sends `DPI_MATCHER_UI_COMMAND` with payload `{type:"GET_STATE"}`.
- Calibrate sets popup visible and adds a window click handler. A click outside
  the Calibrate control while popup is visible sends START and removes handler.
- Close hides popup, removes handler, and sends CANCEL for state `calibrating`,
  otherwise RESET. Unmount removes handler.
- Source `cancel` compares the calibrationState object itself to the string
  `calibrating`; `closePopUp` compares calibrationState.state. Preserve the
  actual expression rather than silently correcting it.
- Calibration state changes except ready open popup and set firstLoad. The
  first error path closes popup as coded. Ready/calibrating/error/completed
  drive the mounted step content; completed alone enables Apply. Apply hides
  popup and sends CONFIRM. Error Retry opens popup and sends RESET.
- Profile selection assigns profile.dpi to `stages[active-1]`, calls the source
  X-stage producer, and selects matcher GUID. It must use the traced producer,
  not an invented DPI field patch.
- Profile dataset localizes object names and actual observed device productName,
  prepends a disabled none sentinel when selection is absent, and disables dots
  for that sentinel. Rename trims, validates source uniqueness, rejects none/None
  and limits mounted input to 32 characters.
- Delete sends DELETE_PROFILE with selected GUID. Delete All has its own
  confirmation; accepted deletion sends DELETE_ALL_PROFILES.
- AS.render mounts inline dots actions. Its alternative portal renderer and
  handleDotsClick geometry are defined but are not called by mounted render.
- Ot consumes MW_SET_DPI_MATCHER_STATE. Profile rename/add/select/delete update
  source state, issue ON_UPDATE_SENSITIVITY_PROFILE and enqueue a source action.
  These paths are not equivalent to an acknowledged native write.

The actual host/native command handler, response/refresh/failure chain and Rust
widget remain gaps. Source semantics acquisition is separate from implementation
and runtime acceptance.

## PID190 Customize missing multipairing conditional mount

CO [4534803,4543795) initializes two local observation flags false. Mount reads
`!!TO(DeviceInfo)` and asynchronously reads `global.multipairing.provisionFlags`.
TO begins at 4530583 and finds an observed device with `master` truthy and numeric
productId equal to the current valid positive numeric productId. Its render gate
at 4543644 is precisely:

```js
oe.DeviceInfo.isMultiPairingSupported &&
this.state.showMultiPairingFeature &&
this.state.isSlaveDevice
    ? jsx(cO, {deviceInfo: oe.DeviceInfo}) : null
```

PID190 source capability `isMultiPairingSupported` is true. The Rust Customize
surface has no corresponding observation producer or cO widget. Preserve the
capability and connect provision flags plus paired-device observation; mount only
when all three are observed true. Missing observations remain unknown. This
widget and its submission chain remain implementation gaps.

## Parent verification, 2026-10-11

Current source receipts: 5393 exact slices across 286 current source files validated. Resource registration independently checked 30 shared floating-controller originals across three products and 42 artwork source/display assets. `cargo check --locked --all-targets --offline` passed with warnings. Pure Rust tests passed: floating/controller-drop 7, mouse button-state 5, source artwork 3 (15 distinct tests). No application, vendor JavaScript, DLL, helper, installer or real device operation was executed; visual/runtime and real persistence acceptance remain absent.
