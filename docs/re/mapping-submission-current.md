# Current mapping submission chain: 190, 678, 679, 688

This audit reads the current official product JavaScript directly. The source was fetched as inert HTTPS bytes on 2026-10-11. The vendor JavaScript, DLLs, application and real device operations were not executed. The Rust implementation below prepares source data only; the complete submission chain remains unfinished.

## Source acquisition and exact receipts

`tools/acquire-mapping-submission-current.py` fetched each official middleware HTML and its declared main script. A static Acorn pass recovered the webpack `u` filename tables and actual startup chunk calls. `--declared-chunks` fetched those exact chunks, checking the declaring main hash first. It does not run webpack, import vendor modules, or search the whole machine.

- `mapping-submission-{190,678,679,688}-current-entry-acquisition.json`: URL, HTTP result, UTC fetch time, original SHA-256 and size.
- `mapping-submission-current-chunk-declarations.json`: complete declaring `u` assignment, UTF-16 boundaries, main hash and exact selected filenames.
- `mapping-submission-{190,678,679,688}-current-chunks-acquisition.json`: exact startup chunk acquisition evidence.
- `mapping-submission-{190,678,679,688}-current-source.json`: complete owning modules and relevant method/function source, original-file and excerpt hashes, UTF-16 boundaries. Existing prose is not implementation evidence.

The official middleware directories originally contained only manifests for these products. Previously cached middleware cannot establish the current submission implementation. This audit acquired the current official entry and chunks independently.

## Mouse 190

Current UI `main.81b09779.js`, module 1350, `101282..105520`, exports `setMappingList` as `c`. It awaits Redux `SET_MAPPINGLIST`, then sets `MAPPING_UPDATED` unless the second argument suppresses that update. This is a local Redux dispatch, not a hardware acknowledgement.

The reducer broadcasts `ON_SET_KEYMAPPING` with copied `mappingList`, current `viewIndex` and `sidepadLayout`. It retains the local action/timerTick record. The middleware dispatcher resolves source state-machine actions `updateLocalStorage`, `generateAppEngine`, and, only when its `obmReady` guard is true, `updateObm`.

Current MW `6120.c8b93f8fc599c9fca67a.js`, module 25947, `Lt` at `1519943..1523721`, receives:

```text
timerTick, from, mappingList, viewIndex, sidepadLayout,
generateAppEngine action, updateObm action, updateLocalStorage action,
version = 0, isMultipleMapping = false
```

Its order is:

1. Read actual current device cache and active profile; do not select a profile from the local editor draft.
2. If updating storage, retain `oldMapping`, generate the app engine when the source action requires it, and update the selected normal/side-panel mapping list for UI-origin actions. The shared function also handles BossKey rewriting and side-panel routing, although PID190 has no Naga side-panel branch.
3. Increment the genuine document version: absent version becomes 1; `Number.MAX_SAFE_INTEGER` wraps to 0; other values increment by 1.
4. Save via module 60763 `mZ` and publish the compact current runtime via module 20236 `lY`.
5. Refresh mappings and macro reducer data; dispatch task-maker `status:"completed"` and its postprocess event.
6. Separately enqueue the OBM task when allowed. Task-maker completion precedes this hardware work.
7. Publish the task-maker memory cache, including real version and OBM data. Errors dispatch `status:"error"` plus postprocess; an earlier mutation may already have occurred.

Module 25947 `je` at `1475709..1476944` generates app-engine mappings and hash, handles the lockable turbo stores with at most three conflict retries, and separately publishes turbo data to the native mapping-engine store. `St` at `1508597..1509145` removes prior OTFS helpers and inserts exact ScrollUp/ScrollDown helpers when DPI_OnTheFly is present. The full generator remains required; submitting raw UI mapping records as native `appEngine.mappings` is incorrect.

Module 60763 constructs `LocalStorageHook` with `PreSetLocalStorage` and `PostSetLocalStorage`. Its pre-hook refreshes active-profile data/report IDs. Its post-hook invokes mapping-engine publication according to actual device mode; some branches launch that work asynchronously. Saving the local document therefore does not prove engine publication.

Module 20236 `M6` (`ke`, `2212769..2214135`) compares actual default mapping hash, active profile, profile count, analog-data hash, profile engine hash and gamemode before writing:

```text
synapse_${actualPhysicalProductId}_${CONTAINER_ID}
```

through `RzDLLService.localStorageSetItem`. The compact runtime publisher `lY` (`Ee`, `2206250..2206975`) is independent and uses window storage.

The source OBM sync resolves affected profile slots and includes active slot 1. Removed mappings are identified by inputID plus `!!isHyperShift`, restored from source defaults and queued separately from new mappings. Macro creation/removal is also queued. Individual acknowledgement/error/cancellation must be traced through these queued tasks; task-maker success is insufficient.

## Analog keyboard differences

The active current task makers are:

| Product | Current MW chunk | Module | Function | UTF-16 range |
|---|---|---:|---|---|
| 678 | `6737.b4bf391c075aa0879a1d.js` | 42124 | `Ot` | 1373200..1380139 |
| 679 | `6737.d9169f212a8242741503.js` | 42124 | `Lt` | 813641..818780 |
| 688 | `6737.9be565ca0f83234feb88.js` | 42124 | `Lt` | 1368732..1373871 |

Arguments are `timerTick, from, {newMapping,isSelectAll=false,isResetKeybinds}, generateAppEngine, updateObm, updateLocalStorage, version=0`. This is different from the mouse positional mapping-list/view payload.

Before generation/persistence, the source:

- Removes `KEY_BACKSLASH` for German, French, UK, Nordic, Turkish, Swiss, SpanishEU, Italian and PortuguesePT layouts. Current enum values are 3, 4, 6, 7, 10, 15, 16, 17, 18. It does not remove `KEY_NON_US_POUND`.
- Replaces each existing AnalogInput output's break point with `max(make - actualActuationUnit, actualActuationMin)`. These values come from the current runtime converter, not arbitrary page defaults.
- Converts OFTM macro records to an AnalogInput wrapper with macro and default outputs. It first looks for the previous non-HyperShift mapping for that input, preserving its actual first actuation pair; otherwise it uses the actual preset configuration. Conversion occurs after the first break-point pass, so the converted pair must not be normalized a second time.
- Flattens the first output of right Shift, right Alt, Application and right Ctrl only for the app-engine input when ModTap is enabled. Persisted AnalogInput records stay wrapped, while the aliased first output also retains the newly assigned inputID/inputType/isHyperShift fields.
- Adds actual interrupt keys to AnalogInput macro/DKS app-engine inputs, using `KEY_ANY` only for the source `interruptionKey:"any"` case. Removes those transient fields from the persisted macro/DKS records afterward.
- Removes any singleKeySnapTap pair whose make or break key is assigned a first-output DKS mapping; retained pairs are numbered from 1. This removal runs even if that snap-trigger feature is disabled.
- Restores actual preset mappings for reset-keybinds only when the selected profile is a preset, after app-engine input generation.
- Handles keymaps, direction-mode and firmware-analog transformations in their source branches. Their executors must not be replaced with normal mouse processing.
- Skips the normal queued OBM sync when `isSelectAll` or the source keymap condition applies. Other calls can still perform source hardware preparation/configuration.
- Clears saving state in `finally`; source task-maker completion still does not prove OBM completion.

## Rust implementation in this change

`crates/razer-device/src/mapping_submission.rs` implements profile/version ownership guards, the recovered version wrap, layer-aware removed mappings and the analog normalization branches listed above, with explicit runtime/preset inputs. `MappingSubmissionPlan::prepare` does not change the supplied source document and does not persist or publish its result. Missing actual converter/default-actuation observations fail explicitly.

The module separates `MappingSubmissionIntent`, `MappingSubmissionPlan`, and actual `MappingSubmissionObservation` stages. The operation/generation/profile guards are Rust cancellation/ownership protections, not invented vendor parameters. The stage variants distinguish profile persistence, engine publication, individual OBM acknowledgements and failure/cancellation. Nothing emits these successful observations automatically.

Pure simulation tests cover version boundaries, stale profile/version rejection, JavaScript truthiness/layer removal, all three analog products, all excluded layouts, macro-conversion ordering, missing configuration, ModTap, transient interrupts, disabled snap-trigger conflict removal, preset reset ordering and isSelectAll. The parent runs compilation/tests; this subtask did not run cargo or Rust Analyzer.

## Native interface boundary and unfinished work

Current host `electron/modules/mapping_engine/win/index.js` declares `localStorageSetItem` as `void(string,string,pointer)`. Its method at `63388..64155` retains a callback `void(bool,string)` in `cbMap`, invokes the actual export with `payload.key` and `payload.value`, returns `{result,reason}`, and removes the callback after completion. Missing initialization returns an explicit failure. This exact wrapper is ABI evidence for static native follow-up, not authorization to call it during development.

The existing `SubmitGlobalShortcutMappings` Rust service branch uses that native export for the unrelated `synapseGlobalShortcuts` store. It is not an implementation of current product mapping generation, product profile storage, engine execution or OBM writes and cannot be reused as proof that product mappings are connected.

The full required work still includes:

1. Port each active current app-engine input/output generator and its native-compatible hash/turbo behavior; do not reuse the inactive duplicated generator merely because its function names match.
2. Port the actual current serial-aware profile/storage hooks, lock/retry semantics, native memory-storage publication and change observers. Local JSON draft persistence remains separate.
3. Use installed IDA Pro/Hex-Rays on private static copies to trace the current `mapping_engine.dll` localStorageSetItem implementation, key parser, native profile activation, driver/output executor, callbacks and cleanup. Retain original hashes, RVAs, function boundaries, pseudocode and cross references. No new native pseudocode is claimed in this receipt.
4. Port every actual OBM encoder, source mapping/default/slot selection, macro operation, direct HID/driver protocol, response parser and failure/cancellation path. The generic current rzDevice25 single-button setter is not sufficient to establish analog mapping or all mouse functions.
5. Connect the UI Save/Cancel/unsaved continuation to the genuine submission stages, preserve source confirmation semantics, refresh from actual state, and retain partial mutation evidence on failure. Cancel before submit is local draft rollback; stop after a write must not claim hardware rollback.

This audit and pure Rust plan are not a completed native mapping implementation, not a runtime acceptance result and not completion of all product UI.

2026-10-11 parent verification: the registered module passed the consolidated `cargo check --locked --all-targets --offline`; after the final source-alias/preset-observation corrections, `cargo test --locked --offline -p razer-device mapping_submission --lib` passed all ten simulation tests. The initially misplaced ModTap assertion in the ordinary macro fixture was corrected against the enabled source branch. Four current product receipts were independently revalidated: 20 file entries and 865 exact UTF-16 source spans; this count excludes the separate host ABI receipt. These results establish compilation and pure data behavior only; no successful persistence, engine submission or device write is inferred.
