# 691 OLED preset and home-card continuation (2026-10-05)

Evidence is the current product 691 manifest-owned OLED lazy module
`OLED.b7b95581.chunk.js` and `OLED.a636cf4a.chunk.css`. The mounted-root/base-CSS
chain is recorded in `keyboard-691-current-evidence.json`. The dedicated
`tools/audit-oled-preset-cards.cjs` statically resolves module 42553's
`Q/G/wt/Vt/jt/Ht/pi` bindings, module 54693's localization exports, and all
matching preset/card CSS rules. It checks the OLED file against the current
page inventory hash and verifies the English/Chinese localization keys.
Receipts are in `oled-preset-cards-current-evidence.json`.

The native animation/image dialogs now use the `pi` editor titles and
`wt/Vt` descriptions. Their preset content uses the source three-column grid,
20px gaps and 30px top margin. Each card keeps 232×64 preview content, a
1px `#5f5f5f` border and 1px margin, changing to a 2px border and zero margin
when selected/hovered. The selected rule follows hover for these presets;
disabled cards use the later `#5f5f5f4d` border rule and 10% image opacity.
Hover scales the image around the crop centre by 1.1 and reveals the black
50% toolbar inside the card. The enable switch is at the upper left,
Replace at the upper right, and Reset at the lower left for enabled custom
slots. Source `_e/De` inline SVGs are statically serialized as
`oled-691-replace.svg` and `oled-691-reset.svg`; their registration records
are the receipt's `assets` array.

The last enabled slot remains protected. Switch activation stops propagation
so enabling another slot does not also select it. Replace/Reset use the
existing import and isolated-draft flows; reset clears the imported payload,
size and crop metadata. Crop Apply and editor Apply retain their separate
commit boundaries. Home previews and the crop dialog retain scale 1; hover
scale only changes the preset presentation and never its saved geometry.
Animated image identity still depends on slot/content, so a hover redraw
does not restart the GIF.

The home card title now follows body hover. Unlike preset borders, its CSS
hover rule occurs after selected and therefore uses `#44d62c4d` even for the
selected card. Media/system requires-Synapse icons now show the source text
when their cards are available. BLE-disabled cards and BLE-disabled edit
actions show their respective source explanations. The native tooltip
component supplies presentation; its placement/delay is still a difference
from the source portal and is not counted as exact CSS parity.

Animation transfer-time estimates remain absent because the source formula
uses processed sizes. Native imports still retain original bytes and crop
geometry, so using those byte counts in the source formula would be false.
Processed crop encoding, language downloads/progress, service/device writes,
Emote/Banner/System editors and measured runtime rendering remain unfinished.

Verification is static only:

```powershell
$env:NODE_PATH='.work/audit-js/node_modules'
node tools/audit-oled-preset-cards.cjs --check
node tools/audit-oled-canvas.cjs --check
node tools/audit-oled-crop-language.cjs --check
rustfmt --edition 2024 --check src/features/source_controls/oled_presets.rs src/features/source_controls/oled_home_cards.rs src/features/source_controls/oled_page.rs
```

No application, build, test, vendor JavaScript, installer or DLL was run.
The parent task performs the permitted consolidated Cargo type check.
