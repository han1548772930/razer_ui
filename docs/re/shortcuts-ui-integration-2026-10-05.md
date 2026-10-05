# Current Global Shortcuts integration

Scope: current Dashboard `4608.e973916f.chunk.js / 94608`, its lazy Map components, and `55.4e8559cb.chunk.css`. This supersedes the historical `7282` references for the UI changed here. Nothing under the four prohibited historical source directories was read or restored.

The maintained static generator is `tools/audit-shortcuts-current.cjs`. Its `--check` mode compares `src/features/shortcuts_current.json` and `docs/re/shortcuts-current-ui-evidence.json` with the current source. Receipts contain complete module-local AST excerpts, CSS conditions, resource SHA-256 values and byte comparisons. No downloaded JavaScript is executed.

## Implemented source tree

`Fe` renders the widget containing `Oe → Te → be` and the sibling shared mapping component `se → ie`. The former combined “chord · action” ghost button and the 292px custom input/output form have been replaced by these two distinct pieces:

- `shortcuts_view.rs`: the 600px widget, localized `SHORTCUTS_HEADER`, `SHORTCUTS_MESSAGE`, top add button and bottom add box, output type/value rows, temporary empty row, input recorder, warning and three-dot actions.
- `shortcuts_mapping.rs`: 292px shared mapping container, 36px header, 250px body offset by the 40px collapsed category rail, and the source rail's 200ms width transition. `21368/E.functionList` supplies all eight categories in source order.
- `shortcuts.rs`: independent output draft and row input recording state. Existing validated launch, text, multimedia and Windows output models are retained.

The list has the source transparent 1px row border, selected green outline, white 10% hover background, width 600, margin-left -10 and padding 15px/20px. The assignment side uses source width 58%, the type is 10px/#6a6a6a, and the value is a separate line. The unmapped row uses the original 59×9 and 128×15 rectangles. Their paragraph margins are represented separately from the second rectangle's -5px visual offset.

The right side records input in a 190px field with a 27px minimum height, 5px left/top/bottom and 22px right padding. Border is #5d5d5d, green while listening/hovering, and #fd8611 for a duplicate. The recorder remains active across multiple inputs and stops when focus leaves. Its 100ms registration delay and disallowed input IDs come from `be`, including Pause/Cancel/Application/Print Screen/Scroll Lock/Num Lock. Right/middle/back/forward mouse buttons use `Te`'s four source mouse input IDs.

The three-dot trigger is 26×26 with a 20px original asset and margin-left 20. Its menu is 150×85, right -124/top 27, switches to right 0 at viewport width <=850, and contains 27px Edit/Duplicate/Delete rows. Click-away excludes the three-dot trigger itself. The existing source-style delete confirmation is still anchored left 274/top 53 within the row. Its typography and transition internals have not been declared fully re-audited.

Duplicate follows `Oe.duplicateShortcut`: copy the output, assign a new identity and clear the input/modifiers, without opening another edit form. Input conflicts appear as a 20px warning and a 300×49 tooltip at right 2/top 22. Source subtype/subvalue rendering remains dependent on adding the corresponding Macro/interdevice output models.

The add box uses relative top 17 and margin-bottom 25, preserving the difference from margin-top 17. Its dashed border changes #5d5d5d → #44d62c → #44d62cb3. Top add swaps the original normal/white images at 18px. Title margins are 10px on both sides. No source-external engine diagnostic/footer buttons remain in this widget.

## Correction to the original S02 review

The initial S02 review cited `.custom-global-shortcuts .key-config.right { left:calc(50% + 294px)!important }` and `ie.customStyleTop() == 132px` as the effective layout. Re-reading `Fe.render` showed the mapping component and `.custom-global-shortcuts` are **siblings**. Therefore the descendant selector does not match this component.

Later CSS that does match is:

```css
.body-wrapper .key-config.open.right {
  left:calc(50% + 296px);
  min-height:486px!important;
  top:142px!important;
}
@media screen and (max-width:1220px) {
  .body-wrapper .key-config.open.right { left:calc(50% + 110px) }
}
@media screen and (max-width:824px) {
  .body-wrapper .key-config.open.right { left:50% }
}
```

`top:142px!important` also wins over inline top 132. The independent review agent checked the same sibling tree. The header begins 36px above the body. A dedicated absolute positioning element keeps the source coordinates without Base popup viewport clamping, which would otherwise move this panel upward in short windows. Host/webview origin alignment and clipping remain subject to future static integration review; this was not visually verified by running the app.

## Mapping categories and actual capabilities

| Source category | Current connection |
| --- | --- |
| SWITCH_DEVICE_PROFILE | Source empty device selector and disabled five profile actions. Real ready-device/subdevice/single-profile filters and actual profile data are not adapted. |
| SWITCH_DEVICE_SENSITIVITY | Empty device branch and disabled initial sensitivity/DPI presentation. Full `40489` stages/XY/sliders and live device configuration link remain incomplete. |
| SWITCH_LIGHTING | Empty profile selector and current configure text. The action targets **Chroma Studio**, never labels the separate Chroma Dashboard as Studio. Parent handles the existing canonical Studio request; no Studio implementation or service state is fabricated. |
| MACRO | Empty source macro selector, playback choices, source creation text and direct local Macro editor action. Local Macro documents are not yet projected into this selector. |
| TEXT_FUNCTION | Source 210×96 textarea, padding 5, Roboto14/line17, 250 UTF-16 limit, ENTER_TEXT placeholder and counter. The emoji popup/categories/search, character-map action and their two toolbar icons are not yet implemented. |
| LAUNCH_PROGRAM | Current program/website radio tree, read-only program path and source folder action, editable website field, original 170×25/146px input geometry. Path and URL persist independently while toggling radios. Late path-picker results retain the generation/identity guard. |
| MULTIMEDIA | Current 10-item source list, using the existing local output model. |
| WINDOWS_SHORTCUT | Current 26-item source list, using the existing local output model. |

The 10 multimedia and 26 Windows items were compared in exact order with the existing strict output validator tables. Macro playback items follow current `getPlayBackSet` for global shortcuts; they remain disabled in the observed empty catalog. No fake macro name, profile name, ready device or installed-module result was inserted.

Some fine details are deliberately still listed as unfinished: text emoji UI, service-dependent category bodies, subtype rows, exact browser-normal line-height/font-baseline behavior, 200ms recorder/dots border interpolation, 300ms menu-row background interpolation, window visibility loss beyond focus notifications, and native input redirection. The implementation does not claim complete S01–S07 or pixel-perfect parity.

Final static review found and the parent corrected a caption mismatch in the retained legacy helpers. The generator now extracts keyboard names directly from current `46114/i` by inputID (shifted symbol descriptors without an inputID are excluded). Left modifiers are `Ctrl`, `Shift`, `Alt` without an invented left prefix; right modifiers use the source `Right Ctrl`, `Right Shift`, `Right Alt` names. Mouse captions and their retained selection state resolve the source `SCROLL_CLICK`, `RIGHT_CLICK`, `MOUSE_BUTTON_4`, `MOUSE_BUTTON_5` locale keys instead of fixed Chinese strings. These tables and the complete source module remain in the receipt.

## Local persistence versus native registration

Current `Oe` saves an output mapping before `be` assigns its input, and duplicate creates another mapped row with an empty input. `Te.updateMapping` also retains a `hasWarning` conflict. Thus local persistence cannot use native input validity as its storage schema.

- `validate_stored_shortcuts` accepts a valid output with no input and permits duplicate chords, but rejects missing/duplicate identities, malformed nonempty input records and malformed outputs. An empty input must have empty modifiers.
- Existing `validate_shortcuts` remains strict. It rejects missing input and duplicate chords. `shortcut_engine::encode_shortcuts` still invokes it and preserves its existing native safety restrictions. There is no new engine transport or dispatch.
- The removed “Apply to engine” button was already permanently disabled and had **no click handler**. “Check configuration” only evaluated the pure encoder and displayed its result. Removing that source-external diagnostics UI did not remove a running/native operation.
- The pure encoder is retained for future transport integration, explicitly annotated as having no current production caller. No dummy calls were added to silence warnings.

GPUI exposes modifier families but not physical left/right keys in its ordinary key event. Existing side-specific stored inputs remain intact until re-recorded; new recordings use the available Ctrl/Alt/Shift families. Source Hypershift is populated from native device-mode/input-redirection events, which are not connected here. The UI does not invent a Hypershift checkbox or synthesize an installed device event.

## Resource and verification receipts

The current Dashboard media cache lacked six shared SVGs. A normal static fetch failed with `network_error`; an escalated fetch was not executed because automatic approval review returned an infrastructure 503. No approval bypass occurred.

The actual resource preparation was completed without network access: the current Dashboard CSS references the same hashed filenames already present under the **current** product 182 cache (and, for several files, current product 653/Macro caches). Their bytes match the embedded assets. The static generator now verifies all six public control assets and all 16 normal/selected rail assets. Lighting rail PNGs are decoded directly from the current Dashboard CSS data URIs and compared byte for byte.

Permitted checks performed by this subtask: Acorn source parsing, CSS parsing, generated JSON `--check`, exact action-list comparisons, asset existence and byte/SHA-256 validation. The existing shortcut test source was maintained to reflect the new output-first/row-recorder/menu flow and UTF-16 insertion boundary, but **no tests were executed**. TextareaMode has no validator API; the source maximum is enforced by removing only the excess inserted span in its change notification, preserving the suffix and cursor instead of merely disabling Save. The parent reported the latest `cargo check --locked --all-targets` passed with the final textarea/recording/menu/absolute-position code. Formatting is likewise parent-owned; no application, build, installer, downloaded JavaScript or DLL was run.
