# 678 mounted TwoTap current-source checkpoint

## Source acquisition

Evidence is the existing current official product corpus, read directly from JS/CSS. No vendor JavaScript, application, DLL, helper, installer, or device was executed. Static extraction uses maintained tools/audit_keyboard_two_tap.cjs and Acorn. The receipts in keyboard-two-tap-current-source.json preserve per-file SHA-256, UTF-8 byte offsets, UTF-16 code-unit boundaries (exclusive end), exact slice hashes, raw methods and CSS.

| Source | SHA-256 |
| --- | --- |
| local-ui-reverse/source/official/apps.razer.com/synapse/products/678/ui/static/js/main.f78f0dcf.js | 8ddbd77a45fedbaad04379cd0d22855190908e1c5427c2629e269527f3a64180 |
| local-ui-reverse/source/official/apps.razer.com/synapse/products/678/ui/static/js/TwoTapKeyMapping.af611a20.chunk.js | c0da34959c0e061a935d11e5378f2e58470df3c92110a0c2b5057084d1bad382 |
| local-ui-reverse/source/official/apps.razer.com/synapse/products/678/ui/static/js/2383.5581abd9.chunk.js | edabbeee976307cd106a653a2a5f747e4a87255884b360dbd819ee704a689282 |

## Reverse-engineered mounted behavior

Customize uses module67742 TwoTap for AnalogInput and shared12383 mapping components. Mounted actuation class Y renders the guide, make-point value/slider and primary-only Sync. It does not mount a custom-release checkbox or release slider. Its separate raw-release and custom-release handlers are recorded as handlers, not additional visible controls.

Current product flags: analog_v1=false; low_profile=false; isShowDoubleMappingOption=false; disableControllerAnalogMapping=false. Therefore this product exposes secondary Keyboard only; extended Controller/Joystick/Mouse category paths are not mounted for the secondary panel. Primary Controller remains available and standalone analog Controller hides actuation; digital/unselected Controller shows it.

Actuation raw defaults are make=19656, release=max(make-unit,minBreak)=18018, unit=1638, range=1638..65535, minBreak=256. Newly enabled secondary make is36 slider ticks; its release starts at the reducer default make value. Secondary requires a10-tick gap and primary Keyboard eligibility at or below30 ticks. Y compares slider input against raw release before conversion; Rust retains that source asymmetry.

Save getter preserves actuation/rapid metadata, strips input metadata from slots, represents a Default key with custom actuation/rapid as empty keyboardGroup, and resets disabled secondary exactly. saveTwoTapChanges merges the complete list, handles branch-specific rapid removal, Hyper replacement, legacy flat macro conversion, opposite-layer creation and actuation synchronization. Remove resets secondary assignment so reopening requires selection.

Sync calls syncActuationKeys(all,list,{0:make,1:make},disabled). The main bundle's inline application-entry IIFE reducer Qa uses module3342.hF excluded IDs and3342.aC generateAnalogMappingList, then posts ON_SET_KEYMAPPING and ON_SYNC_GLOBAL_ACTUATION. Sync does not update active actuation slider state from a fabricated device response.

The Sync reducer is not module99857; that numeric module is SetCache. The direct source initializer is Qa, UTF-16 [6690822, 6691571) in the main file listed above, with slice SHA-256 e303cf83b7fb89990fa967b8a4c656386fe556ba31e529fe5f39172141f780b2. The durable receipt also includes its complete declaration, the c2U switch dispatch, and the Me -> module3342 import. Extraction now rejects missing/ambiguous inline helpers, dispatches or imports; it cannot silently omit this reducer.

## Rust implementation and UI/backend connection

keyboard_mapping_drawer.rs retains two independent slots, controller states and menus, primary/secondary panels, Add/Remove, exact currently implemented category payloads, threshold editors and source Save/Cancel/dirty-confirm flows. keyboard_mapping_two_tap.rs implements the current numeric/default rules, full-list merge and Sync intents. keyboard_mapping_controller.rs preserves actuation/rapid on selection/mode/custom-curve changes. Slot-specific element IDs prevent retained-state collisions.

Save and Sync use KeyboardActuationRequested with real source messages. Local snapshot _localAnalogMappingDraft is explicitly local and is removed from the vendor profile draft when restored. Neither draft mutation nor emitting an intent is successful device persistence. Existing submission state reports errors without synthesizing success.

## Verification and remaining required work

This checkpoint has static AST extraction, resource JSON validation and rustfmt verification. Parent owns consolidated cargo check and pure Rust tests; this document does not claim their result before the parent reports it. Runtime UI/visual/device acceptance is absent under the execution restriction.

Still required: actual SET_ANALOG_KEY_EVENT_CALLBACK/REMOVE_ANALOG_EVENT_CALLBACK register retries, observation and cleanup bridge; live slider/controller sensor overlays; source icon/help/geometry and key anchoring; missing primary mapping editors and their catalogs, conflict confirmations, submission/refresh/service/device persistence and error recovery. Custom-release callbacks exist in source but are not user controls in mounted Y; getter propagation beyond actuation/rapid needs its own caller audit. The optional useNikoFpsAnalogConfig stored preference and its multi-default filtering branch are not yet observed in Rust; no value is fabricated. These are explicit full-scope gaps, not completed or permanently deferred work.
