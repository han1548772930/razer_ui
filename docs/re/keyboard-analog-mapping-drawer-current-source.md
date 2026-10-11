# Current 678/679/688 mapping drawer

Audit date: 2026-10-11. This is an implementation checkpoint, not full page or runtime acceptance. Source files come from the three current product directories under `local-ui-reverse/source/official/apps.razer.com/synapse/products/`. Only static text/AST parsing was used.

## Acquisition and mounted callers

`tools/audit_keyboard_mapping_drawer.cjs 678 679 688 --prepare` reads each current main bundle, stylesheet, ButtonPanel, TwoTap, shared 2383 and all Map chunks; `.work/keyboard-mapping-drawer/receipts.json` retains SHA-256, UTF-8 byte offsets, original snippets and exported literal/function definitions. It hash-checks each main bundle against independently extracted full product metadata before generating `keyboard_mapping_drawer_data.json`. The extracted metadata preserves optional false versus absent `isKeyToggle`, `isSidePanelList`, `disableHypershiftMapping` and per-button capabilities. Vendor JavaScript is never evaluated.

| PID | ButtonPanel chunk | SHA-256 |
| --- | --- | --- |
| 678 | `ButtonPanelComponent.c9c2690e.chunk.js` | `b0c46762bdae90d188e15d8e39bbbb2e5861674524905e3bebb4ee6ff276b956` |
| 679 | `ButtonPanelComponent.70cedf00.chunk.js` | `bd68311a9431f9c48d91b3362f7468d58d0776d0f4060d85bb7d4e4f5a2c95d3` |
| 688 | `ButtonPanelComponent.9ac6ec3b.chunk.js` | `3187d7957a300b3fadf1355e290fc271f0ea94cb9a89dc124992d93b1e70acb2` |

The Customize root mounts ButtonPanel independently of the mapping popup. For current 679, popup caller branches load module67742 from TwoTapKeyMapping for AnalogInput and module12383 from shared2383 otherwise. `setSaveRef` is passed at main bytes8033449/8033968. Root drawer toggle starts at8003976, save-alert controller at8029046. These mounted branches were inspected independently in all three products.

The drawer width is230px; CSS places it at top86px, height `calc(100% - 20px)`, hidden left−230px or open left0 with200ms transition. Body shifts230px, and open body widgets use1000px width. Header67px, dropdown max180px, rows min-height50px/padding9px; hover#383838 and active#111111. Row index width56px/margin-left−10px, value block164px/border-left#707070/padding-left5px, assignment heading10px, value14px. Secondary mappings have a marker and tooltip.

Source `updateButtons` distinguishes no key property, func and multi; Hypershift changes its `HID`, `disableHypershiftMapping` and `isKeyToggle` filters. Rendering additionally omits disabled buttons, explicit `isSidePanelList:false`, the wrong `isKeyToggle` state and mediaVolume. Customized uses reconstructed `button.isMapped`; numerical counters are sorted only when all are integers.

## Source action and confirmation chain

Selecting the same active button is ignored. Another key, drawer toggle or Hypershift change asks the root save alert when the popup is dirty. Source callbacks deliberately remain separate:

- Save hides the alert, awaits `saveMapRef(true)`, then invokes only `nextActionRef`.
- Don't Save hides the alert, awaits `saveMapRef(false)`, then invokes only `dontSaveActionRef`. Drawer/key/Hyper callers typically supply only `nextActionRef`; Don't Save therefore does not automatically perform the pending change.
- Dismiss awaits `saveMapRef(false, event)` and changes sensitivity state. `saveMapping(false)` resets `canSave`/dirty flags without inherently removing staged controls.
- Explicit Cancel calls `closeMapping(true)` and clears the popup. Header close calls `closeMapping(false)` and confirms valid dirty state.

Shared2383 `saveTwoTapChanges` (679 byte36650) calls the TwoTap getter, merges the complete entry, updates active mapping, then calls `setMappingList`, resets dirty state and clears the popup. `getTwoTapMapping` preserves actuation/rapid, strips input metadata from slots, and changes primary Default to empty keyboardGroup when custom actuation or rapid remains. Secondary disabled uses `{actuationPoint:default,isDefault:false,outputType:"defaultGroup",isHyperShift:false}`. Any `isDefault:true` slot removes an entry; custom primary thresholds synchronize the opposite layer and may create an empty keyboard counterpart. Controller/joystick primary or an enabled secondary removes rapidTrigger. Replacing an existing primary hyperShiftGroup removes its opposite Hypershift entry.

Main61350 `setMappingList` dispatches13254.VE0, then marks mappings updated unless explicitly suppressed. Customize reducer creates the full mappingList and posts `ON_SET_KEYMAPPING` with `{mappingList:[...payload]}` (679 bytes7363615/7363715). Rust uses the existing `KeyboardActuationRequested` submission chain and its generation/error handling. These are real submission intents; they are not successful device writes. Popup cleanup's source event is13254.pEN=`REMOVE_ANALOG_EVENT_CALLBACK` (679 byte385198); real callback acquisition/removal remains a backend gap.

## Source conditions and editor payloads

Category head `C.openMap` has no per-category disabled predicate. An absent Rust editor must never be represented as a vendor capability restriction. Shared2383 `getDisplayFunctionList` (679 byte51921) clones `activeButton.functionList`, removes RAZER_HYPERSHIFT in Hyper/secondary, removes INTERDEVICE in secondary, removes APP_SPECIFIC unless profile name is one of the12 source applications, and removes AI_LAUNCHER when30078.eD returns false. Secondary choices further depend on `isShowDoubleMappingOption`,29267.tW and primary controller/joystick selection. Source Save conditions (byte60914) include Chroma profile availability, secondary keyboard eligibility, profile count≤1, valid explicit Keyboard/Controller/Joystick choices, actuation gap and current `canSave` state. Chroma availability can hide the action row.

| Editor | Source payload | Current Rust connection |
| --- | --- | --- |
| Default | `defaultGroup,isDefault:true`; opposite Default for Standard source Hypershift/Disable assignment | Staged; Save removes/reset-preserves analog metadata |
| Keyboard | `keyboardGroup:{keyGroup,key:<string inputID>,modifiers:<string inputIDs>,turboMode?:...}` | Static target categories/modifiers; Save, Cancel, local draft; key recording/turbo/combined mouse remain gaps |
| Disable | `disableGroup` only | Staged; full-list Save |
| Mouse | `mouseGroup:{mouseAssignment,turboMode?:{isTurbo,keysPerSecond}}` | Source option IDs; turbo guard and1..20 range; confirmation/collision gaps remain |
| Multimedia | `multimediaGroup:{multimediaAssignment,useTurbo:true for volume up/down}` | Source options and payload; full-list Save |
| Windows Shortcuts | `win8ShortcutsGroup:{windowsShortcutAssignment}` | Source base/additional/advanced-dial choices and payload; full-list Save |
| Device Brightness | `backlightGroup:{backlightAssignment}` | Source three options and payload; full-list Save |
| Hypershift | `hyperShiftGroup`; Standard generates both layers | Audited; conflict confirmation and actual editor not implemented |
| Macro | `macroGroup:{name,guid,macroPlaybackOption,repeatCount}` | Audited source; macro-library observation and editor not connected |
| Switch Profile | `profileNavigationGroup`; real profiles excluding current, optional lighting | Audited source; observed profile catalog/editor not connected |
| Switch Lighting | `lightPacGroup:{ID,Name,chromaEffectAssignment?}` | Audited source; observed Chroma profile/module/editor not connected |
| Launch | `launchGroup:{path}` or `{url}`; source normalization applies | Audited source; picker/URL editor not connected |
| Text | `textBlockGroup:{text}`,250-character textarea plus emoji/character-map actions | Audited source; native editor/emoji/helper paths not connected |
| Inter-device | `interDeviceGroup` from observed device catalog and nested editor | Audited source; real catalog/action editor not connected |
| AI Launcher | `aiLauncherGroup:{assignment,param,isUseVoiceInput}` from AI query | Audited source; availability/query/editor not connected |
| Controller | Mounted84350 `controllerGroup:{controllerAssignment,controllerMode:{isAnalog},analogSensitivityType,analogSensitivityList?:{analogSensitivityAssignment}}` | Source choices/duplicate gates, source preset modes/curve, four-point Custom drag, staged full-list Save; two-slot retained state and source secondary restrictions; live callback/complete visual gaps remain |
| Joystick | Mounted59184 button/axes/raxes groups with original?128 raw assignments | Button/Direction radios and source dropdowns, placeholders/duplicate/secondary guards, staged full-list Save/Cancel and local selection retention; native write-back/visual acceptance remain gaps |

The generator retains original per-product tables. Mouse choices use source base list, `extendSupportMappings.mouseGroup`, supportsBossKey and per-input ScrollMode restrictions; the audited three currently do not expose the latter capability. Turbo restrictions read69937.LrF, ControllerInput and `disableTurboInAssignment`. Mouse opposite left/right modifier behavior is unrelated and must not be inferred from an HID encoding. Keyboard category1 is source `Alphanumeric`, not `Alpha`.

Hypershift confirmation is a separate explicit SaveAction path: `getSecondaryFunctionKey` searches source DEFAULTMAPPINGS by keyInput.HID matching the active button; `checkIsSecondaryKeyMapped` compares those HIDs, and `checkKeyCanNotRemappedInHyperShift` tests MappingGroup and HID. Either condition with RAZER_HYPERSHIFT opens the confirmation titled RAZER_HYPERSHIFT/body SAVE_HYPERSHIFT_WARNING. Root `saveMapRef(true)` calls `saveChanges` directly; this must not be conflated with explicit `SaveAction -> handleSaveChanges`. The native Hyper editor and this confirmation still require implementation.

Mounted Controller is main module84350 (679 byte6488306), exporting MapController/default/getButtons, loaded directly by shared2383 `ct[CONTROLLER]`. The separate MapControllerV2 chunk is not evidence for this mounted analog editor. Module84350 chooses `DeviceInfo.analogSpecs.controllerGraph` before its fallback-generation graph; getButtons disables assignments already used in primary or secondary mappings for the current Hypershift layer. The generator retains this module's exports. Joystick uses its current MapJoyStick chunk, mode/axis/button payload and duplicate assignments/secondary direction rules; these require their own renderer rather than generic controller choices.

`keyboard_mapping_controller.rs` and its per-product static data now mount the primary84350 selector, analog/digital source choices, source STANDARD/FAST/SLOW/INSTANT/CUSTOM modes and181×128px curve. Primary and secondary assignments of the current layer contribute duplicate restrictions. Digital mappings retain controllerMode.isAnalog:false and omit analogSensitivityList. Analog curves emit the original four p1–p4 values; source graph conversion rounds x to two decimals and y to integer, except source controllerAssignmentsWithoutSynapse bypasses this conversion. Custom point1/4 edits y only; point2/3 edit x/y with the source3px minimum horizontal separation. Save removes rapidTrigger for Controller primary. Source graph mounting posts13254.fMo and removes13254.pEN on cleanup; no Rust callback/observation adapter or fake sensor values are introduced in this checkpoint. Controller assets/help/live indicator still need implementation and acceptance. Current678/679/688 source flags do not expose extended secondary Controller editing.

## Rust checkpoint and open gaps

`keyboard_mapping_drawer.rs` provides retained-metadata rows/filter, staged popup, source string key IDs, listed concrete editor payloads, Save/Cancel/header-confirm paths and existing middleware intents. `_localAnalogMappingDraft` is explicitly local snapshot storage, removed from the vendor draft on restore; it is not a profile/device save. Product profile restore cancels old pending request generations. Route/factory changes clear local popup state. Current unimplemented categories remain selectable but have missing body controls recorded here; neither a vendor disabled condition nor a vendor component-load failure is fabricated. Submission errors remain independent state pending an exact audit of the source error alert chain.

Still incomplete: all remaining listed editors; analog callback/overlay and complete mounted TwoTap visual parity; source backdrop placement/transition/key anchor; controller/analog callback acquisition/cleanup; row tooltip/secondary marker; precise source `button.isMapped` reconstruction and key-property filters; category app/AI observation filtering; source-specific profile/module service catalog chains; Hyper/collision/sensitivity warning confirmations; native service/device persistence and error recovery. These remain required work, not deferred integration or completed UI.

The child ran static extraction and rustfmt only. Parent owns consolidated cargo checks. No application, vendor JavaScript, target DLL, helper, installer or real device operation was executed; runtime visual/device acceptance is absent.

## Mounted TwoTap follow-up

Current678/679/688 now have independent primary/secondary slot state, source Keyboard-only secondary selection, Add/Remove reset, make-point sliders,10-tick gap/30-tick eligibility, numeric defaults and branch-specific full-list merge. Sync follows current99857.Qa through3342.QY/hF/aC and emits ON_SET_KEYMAPPING plus ON_SYNC_GLOBAL_ACTUATION. Mounted Y renders no custom-release checkbox/slider; its internal handlers are not evidence for extra UI. See keyboard-678-two-tap-current-source.md, keyboard-679-two-tap-current-source.md and keyboard-688-two-tap-current-source.md for each page and keyboard-two-tap-current-source.json for exact current receipts. Callback acquisition/cleanup and real service/device persistence remain explicit gaps.

## Mounted Joystick follow-up

Current raw12383 category loaders independently verified mounted59184 for678/679/688. keyboard_mapping_joystick.rs now implements its Button/Direction editor and exact profile payloads, duplicate/secondary direction rules, original dropdown arrow, source menu geometry and blur/outside cleanup. Quick Remapping is separate evidence and was not used for this renderer. See keyboard-{678,679,688}-joystick-current-source.md and keyboard-joystick-current-source.json. Shared visual timing/hover portal/scroll fidelity and runtime/native write-back acceptance remain recorded gaps.
