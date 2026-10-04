# Base Station V3 Chroma (3946) automation audit

Current source: `.ref/devices/3946/static/js/main.*.js`, matched to
`pending-product-3946-source.json`. Source metadata names this product **Razer
Base Station V3 Chroma**; August T2 is an internal source name. Acorn extraction
records complete component snippets, offsets, hashes, ten locale tables, six
action categories, seven quick effects and literal effect defaults in
`automation-current-evidence.json` / `automation_data.json`.

The formal Customize page now contains the product image, left Automation
widget and right LaunchSoundApp THX/7.1 card, matching the mounted root at
6953104. The developer preview renders the same complete `Automation` view.
Its action editor is an overlay opened from that real page, not a replacement
for the page. Parent navigation/profile chrome is owned by SourceWorkspace.

Automation implements one row per source category (maximum six), enable
switches, add/edit, pickup/putdown toggles, category-specific empty lists,
disconnected selection feedback, restore-previous output/microphone selection,
pause-game selection, macro/game/shortcut candidate selection, Chroma quick /
advanced / off selection, apply-to-all flags, source effect defaults, and
dirty/nonempty Save validation. Save/delete mutate only local preferences.
Editor cancellation discards its private draft. The local profile snapshot
contains source-shaped `automationWidget` records. The official storage field
is `synapse_3946.profiles[activeProfile].automation`: the current root's
`loadActiveProfileSettings` at UTF-16 offset 6984615 loads this field through
`MW_SET_AUTOMATION_TO_UI` into `automationReducer.automationWidget`. This is
profile state even though it has a separate reducer. `automationLinkedGame`
is another source profile field; its editor is still pending. The `persistence`
and `automation_actions` evidence entries retain these paths. Catalogs and
install/service observations are never restored from profile data. Locally saved
quick macros are restored into the local candidate list from their rule lanes.
The explicit
fixture is isolated from persistence.

`src/features/automation/theme.rs` defines the exact CSS palette for panel,
row, hover, pressed, foreground, muted, primary, border, footer, danger and
warning roles. Body uses Roboto 14px at the source 16px rem scale; widget title
uses RazerF5 16px. Widget and row widths/padding, 60px item height, 355px lane
width, 34/35px lane gaps, 37px category select, footer sizing and 109px modal
top margin are derived from the current CSS. Twenty-three SVG assets are recorded in
`automation-manifest.json`: nineteen exact copies from manifest-declared current
URLs and four type icons converted from literal JSX SVG paths. Inline receipts
retain the current source range, fragment and source/output hashes.

## Explicit remaining fidelity and integration work

- The root has no automation executor or catalogs adapter. Hardware triggers,
  installed-app detection, audio endpoints, Chroma profile retrieval, THX
  entitlement and activation-code retrieval are not synthesized. A management
  request that cannot be fulfilled reports that it is unavailable; fixture
  requests are visibly labeled as examples.
- Static and Breathing now render the actual source color parameters for each
  trigger lane. Static exposes one color and hides no-color; Breathing exposes
  Color 1 / Color 2 (including no-color) and Random color. Random disables both
  color pickers without discarding their values. Both reuse the native source
  palette/custom-color editor: the validator compares all forty native preset
  colors against the current 3946 `ZT` literal, including `#ffc182`.
  Source-mounted `LP` selects `cl` (`_l`, 6423938–6425144) and `Hl` (`vl`),
  retained under `quick_color_parameters`. Each edit changes only its lane's
  local `setting` and clears apply-to-other-devices as `LP`'s `P` callback does.
  The editor retains these changes until Save; Cancel discards the rule draft.
  Starlight now shares the two-color/random controls and adds the actual 1–3
  duration slider with Short/Medium/Long tags and no numeric tooltip. Its changes
  merge settings. Wave uses `DeviceInfo.WAVE_DIRECTION=UpDown` and values 3/4;
  `GT/yT` toggles on either half, including the selected half, and on the outside
  frame. Current arrow artwork, 42px frame, 40×30 segments, active green fill and
  hover border are prepared from the current source. Audio Meter mounts the
  Color Boost stepper (0.25–4, step 0.25, maxLength 4); unlike the other effects,
  `AI.changeColorBoost` replaces settings with `{colorBoost}` after rounding up
  to a quarter. There is no mounted hardware color picker or Wave speed branch
  for this product. Fire and Spectrum Cycling mount no parameter component
  in this current `LP` switch; shared speed variants are not evidence that 3946
  should display additional controls.
  The number editor is current module 44230, whose behavior is identical to the
  retained native Kiyo editor. Its caller-provided `stepperRegex` is not consumed
  by that module. Automation adds the higher-specificity `.modes-area .stepper`
  presentation (60×27; input 58×25 with 5px left padding and 17px line height),
  keeping the default Kiyo 62×26 presentation. The source receipts include the
  complete stepper module, resolved aliases, parameter labels and the relevant
  ordered CSS rules. Duration owns retained slider/focus state per trigger;
  keyboard changes explicitly update the draft because GPUI `set_value` does not
  emit a SliderEvent. Pointer changes also set the discrete value back into the
  retained slider to snap its percentage to Short/Medium/Long. Visible color
  captions, their 5px trigger gap, the independently centered Medium label and
  30px Apply-to-all gap follow the source layout. Mode switches and Off's
  apply-to-all toggle merge into the existing lane data. Reselecting an effect
  restores that lane's saved setting if present, otherwise the source default;
  this is not a cache of unsaved alternate effects. `yP.renderEffects` contains
  an OR of the two lane props, but the two `GP` mounts each receive only their
  own data and its connector does not inject either lane, so no cross-lane
  advanced panel is implied. These changes have no executor or hardware I/O.
  The duration observer also handles Base Slider accessibility adjustments,
  which do not emit Change; equality checks avoid rewriting the lane or clearing
  apply flags during parent synchronization. Every parameter edit emits LP's
  exact quick-effect envelope, with Audio Meter retaining only its colorBoost
  setting as required by AI.
- The source quick-macro entry opens a 500×369 local editor derived from `aH`
  at UTF-16 offsets 6920385–6931761. It follows the empty-type initial state,
  `Macro N` default naming, four-type dropdown, keyboard capture with a
  ten-key limit and removable pills, launch program/website selection,
  run-command and 54px multiline text (including the source's 250 UTF-16-unit
  limit and counter).
  Clear resets action input; Save requires a name and nonempty action and
  resolves duplicate names before adding a local candidate to the selected
  lane. It stores UI-readable local action data, without a vendor macro service
  payload, native macro execution or driver acknowledgement.
  The Program branch now opens a single-file picker and shows a read-only
  selected `.exe` path; cancellation or invalid selection retains the previous
  draft. Program and Website preserve separate values; Clear and type changes
  invalidate older picker results. GPUI cannot filter extensions in its native
  picker, so `.exe` is checked after selection. See
  [current program picker evidence](automation-quick-program-current-audit.md).
  Native type-menu row icons, source capture-session keyup
  behavior, exact pill hover/delete overlays and transition timing still need
  completion. Game browser/link editing and
  global-shortcut assignment remain follow-up work; their buttons continue to
  report the unavailable service boundary rather than inventing catalogs.
- Advanced Chroma profile selection requires the real profile catalog; source
  unavailable/install/empty states are represented. The THX activation overlay
  reports unavailable and does not generate a code.
- Source row background transitions (300ms) now have matching hover/pressed
  color states in the native row buttons. Action/text transitions (200ms),
  exact selector icon slots, original headset-toggle SVGs and anchored delete
  confirmation placement are still pending. Existing native controls supply
  keyboard behavior but are not evidence of exact source animations.
- The delete confirmation currently occupies its source-colored region inside
  the editor; it has not yet been converted to the original anchored overlay.
- No rendered geometry, platform font metrics, focus paths or animation timing
  have been observed because the user prohibits running apps/tests.

These limits mean that this is a substantial native continuation, not a claim
that product 3946 is already pixel-identical or feature-complete.

## Profile header source

`setProfileDropdownState` appears only in the current action export, with no
root page-switch dispatch. The profile reducer initial value at 6066429 is
`enableSwitchProfile: true`; generic adjustment-mode actions may change it.
Evidence snippets are retained in `profile_bar`. Parent integration must not
hide or disable this product's profile dropdown merely because Customize is
selected.

`isEnableProfileBar` only gates the synchronization icon. The constructor's
`KH` excludes `HELP`; subsequent `updateView` and `updateNavigation` exclude
both `HELP` and `TAB_CALIBRATION`. Static label resolution is recorded in
`profile_bar_labels` (`_$r`, `tPc`, and `lgc`). This constructor/navigation
discrepancy is preserved in the audit rather than mistaken for a condition on
the entire dropdown.

## Verification and seam

`node tools/extract-automation.cjs --check`, `python tools/prepare-automation.py`,
`python tools/validate-automation.py` and rustfmt are the allowed checks used.
Compilation is owned by the root task's final `cargo check --locked --all-targets`.
No application, build, tests, downloaded JavaScript or DLLs ran.

Integration API: `Automation::new(&Device,window,cx)`, `supports_page(pid,key)`,
`snapshot()`, `restore(Option<&Value>,window,cx)`, `dismiss(window,cx)`,
`AutomationChanged`, and `open_preview(window,cx)`. Resource embedding consumes
`assets/synapse/automation-manifest.json`.
