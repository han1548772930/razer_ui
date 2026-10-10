# Current Analog Gamepad keyboard Customize and Lighting callers

Audit date: 2026-10-10. Products 678, 679 and 688 were inspected individually in their current product bundles. Shared behavior follows actual mounted callers and capability values. None of the inspected sections contains an explicit PID 679 condition; the Rust group must not be described as a vendor 679-only branch.

## Source acquisition and reverse-engineered semantics

All paths below are under `local-ui-reverse/source/official/apps.razer.com/synapse/products/<PID>/ui/static/`. Only text parsing and static media copying were performed; no vendor JavaScript was evaluated. Offsets are UTF-8 bytes, not JavaScript character indices.

| PID | JavaScript file / SHA-256 | CSS file / SHA-256 |
| --- | --- | --- |
| 678 | `js/main.f78f0dcf.js` / `8ddbd77a45fedbaad04379cd0d22855190908e1c5427c2629e269527f3a64180` | `css/main.28785c66.css` / `2c8359c87e11fadf087686fd6359e06da3b70ac4070ffda73d14de5958ccb2df` |
| 679 | `js/main.194d8a7b.js` / `3f9a9722c21dbd3b05d358218213fd4a4b57673c1ffe9da81a30128d298a3f20` | `css/main.1c5a651a.css` / `389a13a4f3419d3b2d566e5ac6a339c506aec7b5ae000735ecb5bc635d25fca8` |
| 688 | `js/main.e09fd702.js` / `61f64c6bad6309663f42b799c56ded8e4045ccb17a34a98b4ffbfb2792d31929` | `css/main.00729a36.css` / `4f3374f0a2784861241f822e29d17253d083b0177e894973e5b67366a40b243c` |

`node tools/audit_keyboard_analog_callers.cjs 678 679 688` statically regenerates source snippets, hashes, offsets, color callers and CSS rules in `.work/keyboard-analog-callers/receipts.json`.

The ten reused Quick Remapping, controller indicator, drawer, expand, trigger and Chroma sync SVGs were also hashed separately in each product's media directory. Each corresponding asset is byte-identical across all three sources; their retained `keyboard-679-*` asset filenames record acquisition provenance and do not restrict product applicability. Per-product paths and hashes are in the receipt's `resources` field.

| PID | Quick Remapping | Gamepad Tester | Drawer | Customize wrapper | Lighting factory branch |
| --- | ---: | ---: | ---: | ---: | ---: |
| 678 | 8084077 | 8119235 | 8122179 | 8127456 | 7993370 |
| 679 | 7979501 | 8001032 | 8003976 | 8009542 | 7888796 |
| 688 | 7742549 | 7747284 | 7749152 | 7751654 | 7635819 |

Each Customize caller mounts Gaming Mode and Analog Windows Properties on the left and Quick Remapping plus Gamepad Tester on the right. Product 688 mounts `Hl` (ModTap) first on the right, followed by `Ad` and `Dd`; this extra card is not polling. Products 678/679 use the wrist-rest config class; 688 instead passes `paddingTop:"2%"`, relative to the config container width. All three final cascades set the wrapper to 385px and config block to 387px with 6px margins and maximum height 100%.

All three declare `AnalogGenVersion:"analogV2"`. Quick remapping selects `[{x:0.1,y:0},{x:1.4,y:86},{x:2.8,y:176},{x:4,y:255}]`; the conditional's imported generation constant means analogV1, not a PID. Bind/reset edits mappings and submits `ON_SET_KEYMAPPING`; conflicting assignments are refused. Customize callers do not pass `keyHoverAndRemappedColor`; the black-remap class cannot be applied just because a product is in this group. Standard assignments use `#44d62c66`, Hypershift `#fd8611b3!important`, with green/orange 2px strokes and transparent Standard active/hover fill. Actuation-specific color callers require a separate exact audit.

Lighting mounts brightness/switch-off and Effects without a keyboard image. Effects uses Quick/Advanced tabs, a 150×27px dropdown, Chroma sync and effect-specific controls. Each bundle retains its own supported effect list.

### Product 688 ModTap

`Hl` starts at JavaScript byte 7662693. It reads `modTapReducer.isEnabled` and `deviceReducer.layoutId`, immediately dispatches enabled state locally, and posts `{"type":"ON_SET_MOD_TAP","payload":{"mappingList":<current customize mappings>,"isEnabled":<boolean>}}`. The default profile has `modTap:false` at byte 6574668; profile callback handling observes `modTap` at byte 8005315. Japanese layout enum value 12 uses Fn instead of Menu. The switch refuses input while a document key is held, clearing that state on keyup and window blur.

Help columns: PRIMARY Right Shift, Right Alt, Menu/Fn, Right Ctrl; SECONDARY Up, Left, Down, Right. CSS positions: gap20 at 363836; 141×74px SVG at 363881; description flex1 at 364013; black/white tooltip with #5d5d5d border and padding10 at 364066; column spacing20 at 364205; underlined headings at 364286; unstyled list at 364381; item margin5 at 364445.

Copied resource: `688/ui/static/media/mod_tap.37006906.svg` → `assets/synapse/keyboard-688-mod-tap.svg`; SHA-256 `4782f618530a4c2b225071f9504d2a9c7896f12b7747e04d3ebc9c4f4f7ee8f5`.

## Rust implementation and UI/backend connection

`keyboard_analog_gamepad.rs` renders the shared cards and 688 ModTap difference. `keyboard_products_data.json` records `source_layout:"analog_gamepad"` for only these three independently audited products, with `source_mod_tap:true` and 2% padding only for 688. `keyboard_products.rs`, `keyboard_snap_analog.rs`, `keyboard_snap_tap.rs` and `keyboard_actuation.rs` consume these source-backed conditions, replacing newly introduced 679-only restrictions. Ordinary-product behavior and separately audited Rapid Trigger capability lists remain distinct.

ModTap edits `/modTap` in the local draft, emits local persistence and sends the exact request through `KeyboardActuationRequested`. Real ModTap/layout observations refresh the UI and Snap Tap input interpretation. Native keydown/up and window activation handling reproduce the held-key guard and cleanup. Gamepad values remain observation-only; absent observations do not fabricate reads or centered sticks.

Per-product actuation parameters remain distinct: 678 max65535/minBreak256; 679 max65520/minBreak1638; 688 max65520/minBreak256. Slider bounds, quantization and defaults read each product's `Info`; no shared fixed device range was introduced. The common default make/release value19656 is source-backed. Separate custom-release constraints using minBreak are still incomplete.

Requests are submission intents, not device acknowledgements. The existing generation/error path reports unsupported until its real native service adapter is implemented. Local draft persistence is explicitly local and is not a successful vendor write.

## Remaining implementation and acceptance gaps

The real mapping blade, exact drawer positioning, original conflict confirmation backdrop, effect-specific controls beyond Wave direction, Advanced Effects module and real native write-back/gamepad adapter remain unfinished. Rich ModTap help contains source content and dimensions, but toolkit placement/delay has not received runtime visual acceptance. Actuation-specific remap colors and custom-release constraints remain gaps. Resource/source checks and rustfmt passed; this child did not run cargo or the application. No runtime/device acceptance was performed; these pages are not marked fully completed.
