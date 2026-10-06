# 1383 OLED runtime conditions — current source

This audit reads the 1383 current `asset-manifest.json` only. The maintained
extractor parses webpack modules and CSS without evaluating vendor JavaScript.
Every receipt records the full source-file SHA-256 and UTF-16 excerpt offsets.
See `audio-oled-runtime-current-evidence.json` for the exact source.

## Source and implementation

- Module 33397 `I` starts `isBle=false,isDongle=false`; `hn` starts language and
  its changed counter at zero; `Cn` starts animation/none with zero counts and
  progress. Module 79826 `T.errorPayload` is null. These are retained as source
  initial state, with an explicit observation API for real connection/transport
  values. Opening the page never invents a dongle connection.
- Module 51278 `xx` mounts its no-dongle warning at 400×117 without a footer.
  WarningAlert uses its own current CSS, warning icon from module 27875,
  100px top margin, orange border/header and body-relative layer 200.
  No close icon, Escape dismissal or backdrop dismissal is added.
- `gx` mounts the simple loader, animation/image/reset progress, and language
  progress. Simple loading follows `Qv`'s 100-unit SVG geometry, two-second
  linear 0/180/720-degree rotation, 10%/50%/10% dash lengths and square caps.
  `cx` retains the source 400px/260px-minimum panel, labels, item counter and
  reset-only disabled Cancel All. `k` progress width uses the source 300ms
  linear change; a decrease finishes the preceding item at 100, resets over
  1ms, then advances to the new observed value. This is display interpolation,
  never a change to the reported progress.
- `ux` opens the language readability confirmation. CANCEL emits
  `ON_CANCEL_OLED_LANGUAGE_UPDATE` and closes only that confirmation. CONTINUE
  UPDATE closes the confirmation without a service message. Neither invents
  completion or clears download progress.
- `vx` opens observed error payloads. Ordinary RETRY/REVERT emit the exact
  source message with that payload and clear the local error as module 79826
  specifies. During reset, RETRY keeps the error payload but hides the warning
  and emits `ON_RESET_OLED` with the source payload shape. Reset REVERT first
  performs the source cancel-download state transition, then emits REVERT.
- `Dv/Hp/Bp`: progress applies the source disabled opacity .3 to home controls;
  BLE disables hover for Media/System Info and dims those cards to .5. Their
  source attribute tooltip stays at widget left20/top185, opacity transition
  300ms, hovered priority107. The Requires Synapse icon does not trigger its
  tooltip while these cards are disabled.
- BLE artwork/banner EDIT is gray **and remains clickable in this current
  source**: `DisplayWidget_disabled` sets color only, and `Bp` preserves its
  handler. There is no `ble-edit:hover` CSS in this manifest. The declared
  `ble-edit-tip` therefore remains hidden; no hover behavior is invented.
- `Bv` disables the entire language widget during BLE/progress, distinguishes
  raw/staged/decoded language (`raw < 127 ? raw : 255 & ~raw`), and enables Apply
  according to both source equality checks. SET locally writes the reducer
  value and emits `ON_SET_OLED_LANGUAGE`; only observed MW updates advance the
  changed counter. The selected System Info slides' device labels are remapped
  only for a changed observation with counter >1, matching the mounted effect.

## Integration and limits

`AudioProductWorkspace::observe_oled_runtime` accepts typed Connection, Loading,
Error and Language observations. `OledRuntimeRequested` carries the source
message name and payload. `request_oled_runtime_data` emits
`ON_SET_OLED_DATA_TO_UI`, the source GET action's actual broadcast. The parent
workspace owns transport routing and general Select synchronization. Runtime
state survives local profile-draft reinitialization; all existing editors and
their local data remain in place behind source-required overlays.

`audio_products` now synchronizes the decoded language Select and requests data
on entering TAB_OLED. The parent source-product and product workspaces forward
both the observation API and `OledRuntimeRequested`, whose payload and message
type have reader methods. The warning SVG is registered in the parent's
embedded resource table. This event path does not invent a transport provider;
an actual provider remains responsible for supplying observations.

These UI branches do not provide device discovery, firmware transfer, language
download service, upload acknowledgements or synthetic completion. Reset retry
and other requests are observable events, not a claim that firmware accepted
local artwork or a custom crop.

Verification is limited to static AST/CSS parsing, source/locale/resource
validation, formatting and the parent-owned `cargo check --locked --all-targets`.
No application, build, test, installer, downloaded JavaScript or DLL was run.

## Continuation review — 2026-10-06

Re-read the current `Bv`, `xx`, language reducer and GET action receipts, then
checked the Audio → SourceProduct → Product request/observation seams. Product
1383 begins on `TAB_SOUND`; entering `TAB_OLED` reaches `set_page` and emits
the GET broadcast. The SVG referenced by the runtime warning is embedded.

The review corrected language lifecycle differences. `Bv`'s `[t]` effect
replaces a staged choice only when the raw language changes; an MW observation
that increments the counter with the same raw value now preserves that choice.
The `[s]` effect uses the selection from its current render and runs only while
the OLED page is mounted. Leaving and reopening the page clears the component's
local staged choice, as the original unmount/remount does. Profile draft restore
preserves this mounted choice and obtains the device language from the retained
runtime reducer rather than from a saved profile. General Select synchronization
now uses its existing staged-value reader, removing that reader's unused warning.

Both maintained static commands below passed: 46 exact source receipts, 47 CSS
rules, current manifest/source hashes, locales, warning SVG and resource entry,
language lifecycle guards, tab-entry request, and both forwarding seams. These
checks establish static wiring and source agreement, not live transport behavior.
The parent agent owns the final compile check for this continuation.

```text
node tools/extract-audio-oled-runtime.cjs --check
python tools/validate-audio-oled-runtime.py
rustfmt --edition 2024 --check src/features/audio_oled_runtime.rs src/features/audio_oled_runtime_theme.rs
```
