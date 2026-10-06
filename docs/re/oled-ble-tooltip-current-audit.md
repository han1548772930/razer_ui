# OLED BLE-disabled card tooltip

This audit covers the product 691 OLED home-card disabled branch. Evidence is
the current lazy product bundle, not a historical snapshot:

- `.ref/devices/691/static/js/OLED.b7b95581.chunk.js` (SHA-256
  `ea0480e7c17180944b2aac3d4287abba9eeec3338c54e0327d55e370643deee6`),
  module `Q` (`oled-preset-cards-current-evidence.json`, binding offset
  6617–8077). Its `disable` branch adds the `turn-off-ble-tooltip` attribute
  to the outer card wrapper and sets the inner card opacity to `.5`.
- `.ref/devices/691/static/css/OLED.a636cf4a.chunk.css` (SHA-256
  `57fe855f0cf030f5d1b0fdedade193b676820a7951dbce50e30775cf48dfa8d`),
  selector `[turn-off-ble-tooltip]:before`:
  `background:#000`, `border:1px solid #5d5d5d`, `color:#ccc`, `font-size:14px`,
  `line-height:16px`, `left:20px`, `top:185px`, `padding:8px 10px`, and
  `transition:visibility 0s,opacity .3s linear`; hover changes opacity to `1`
  and visibility to `visible`.

The native consumer in `src/features/source_controls/oled_home_cards.rs` now
uses `SourceTooltipKind::OledBleDisabled` for the BLE-disabled media and system
cards. `src/ui/source_tooltip.rs` measures this path as max-content (the source
has no width cap), anchors the portal at the card-relative 20px/185px offset,
and retains the 300ms linear opacity lifecycle. The card's `.5` opacity remains
on the card content only, so the tooltip keeps the source's full contrast.

This is a static source alignment. Application execution and rendered tooltip
pixel/focus verification remain prohibited by the workspace requirements.

## 2026-10-06 编辑控件也接入同一机制

源里 `turn-off-ble-tooltip` 是**卡片级**属性（`w ? {"turn-off-ble-tooltip": Q} : {}`，`Q = getTextItem(v.XVF)`），
编辑按钮本身就活在这张卡片里，所以本地把编辑按钮的提示也从 Kit `tooltip::Tooltip::new(...)` 换成
`SourceTooltipKind::OledBleDisabled`（`oled-edit-tooltip-{name}`，触发器仍是那个编辑按钮），
`oled-require-synapse-tip` 之外不再有 Kit 提示：`tools/audit-oled-preset-cards.cjs` 现在断言
`oled_home_cards.rs` 里不存在 `tooltip::Tooltip::new(`，并按源 CSS/JSX 断言
`[turn-off-ble-tooltip]:before{…left:20px…top:185px…transition:visibility 0s,opacity .3s linear}`
与 `"turn-off-ble-tooltip"` 字面量。
