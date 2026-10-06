# Current wireless ARGB customize pages (3884 / 3886)

Audited on 2026-10-03 using only the current product bundles recorded by
`pending-product-3884-source.json` and `pending-product-3886-source.json`.
`tools/extract-wireless-argb.cjs` parses these bundles with Acorn. Vendor
JavaScript is never evaluated. Component offsets, product metadata, label
exports, ten locale tables, CSS, reducer excerpts and resource declarations
are retained in `wireless-argb-current-evidence.json`.

The native module renders the 260 × 260 product artwork, power/automatic
detection/refresh controls, standby, Bluetooth/mobile sync, detection, missing
devices, overcurrent protection and LED-limit messages. The 3886 source also
has a DC-power-required message. Requests never fabricate hardware responses.
Without a device service the live page shows status unavailable; development
fixtures are confined to the explicit preview dialog.

The port editor preserves strip/fan choice, editable port names, LED steppers,
fan LED choices, up to four strip segments, bend redistribution, second/fourth
side constraint, removable additional segments, totals, Chroma-layout notices
and warning copy. The 3884 editor additionally permits multiple fans, adding
20 LEDs per fan, and sums all active ports against the 240-LED device limit.
It uses the source `minimumLedValue: 1`; its per-port maximum comes from a
device capability (the isolated fixture uses the source's 80-LED fan-capable
case). The 3886 source minimum is 4, its numeric editor maximum is
`max(detectedLedCount, 40)`, and its per-port warning threshold is 120 despite
the warning string naming 80. That disagreement is preserved, not normalized.

## Actual mounted-source limitations

3886 `TopViewComponent` includes
`!r && i && !u.type === k_.BLE_MOBIL && this.renderPorts(a)`.
`k_.BLE_MOBIL` is the number 3; strict comparison of a Boolean with 3 is always
false. The normal 3886 page therefore does not mount its port cards. Native
rendering preserves this. The preview contains an explicitly labeled switch
to inspect the defined but unreachable editor; that switch is never counted
as live-page coverage or evidence that the official page mounts it.

There is no discovery/power/connection/Chroma service adapter in this module.
The preview records requests and requires explicit state selection for their
results. It neither emits profile updates nor saves fixture observations.
`snapshot`/`restore` contain only port layout preferences. Detection counts,
active ports, power, warning, connection and the per-port notice dismissal
remain transient. Restore validates known ports and values, limits input size,
and assigns fresh stable local segment identities.

The parent stores the port-layout snapshot in `source_device_settings`, outside
lighting profiles. Both current roots' `loadActiveProfileSettings` load
`isPowerOn`, `isAutoDetectionEnable`, brightness, quick effects and lighting
timeouts from the selected profile; neither loads a ports field. Ports instead
flow through `portsReducer` and `ON_SET_PORT_VALUES` (3884 also sends
`portsConfig`). These exact paths are retained in evidence `persistence`.
The downloaded UI does not establish the service's on-disk port schema; this
audit makes no claim about that absent middleware storage implementation.

Native interaction uses retained Kit input/select state, semantic buttons,
and the framework dialog for Escape/focus restoration. The source SVG shapes
and source CSS artwork colors are statically extracted; rem-relative geometry
follows the source 16px reference scale. Runtime geometry, focus behavior and
pixel equality have not been measured because running the application/tests
is prohibited. The current native editor uses integer LEDs; 3886's historical
numeric widget technically accepts decimals despite LED-count semantics.

The latest source review moved distinct card/border/warning colors into
`wireless_argb/theme.rs`, and explicitly set the Roboto body and RazerF5 port
heading fonts. The icon `:hover`/`:active` recoloring and the auto-detection
keyframes are implemented from the audited stylesheet (next section). Native
tooltip timing, numeric decimal entry for 3886 and rendered pixel geometry
remain unverified fidelity gaps: nothing here was measured in a live window, and
a static SVG receipt never by itself proves the animation.

Profile header behavior is separately traced in evidence `profile_bar`:
3884 root dispatches `setProfileDropdownState(active_view===TAB_LIGHTING)` on
mount and update, so its non-Lighting dropdown is disabled. 3886 has no such
root dispatch and starts `enableSwitchProfile:true`; its `isEnableProfileBar`
prop only disables the synchronization icon outside Lighting. Both keep the
profile bar mounted. Parent SourceWorkspace owns these conditions.

## Icon interaction layers and auto-detection keyframes

The extracted stylesheet recolors one layer per icon and animates the
auto-detection glyph; GPUI runs neither, so `prepare-wireless-argb.py` splits
each interactive icon into the layer the source paints and the layer its
`:hover`/`:active` rules target. Every derived layer keeps the audited geometry
and viewport attributes verbatim and replaces only the recolored paint with
`currentColor`, and the native page supplies the colors from the same
declarations:

| Source declaration | Native color |
| --- | --- |
| `.icon-detection:hover .detect-b{fill:#44d62c}` | `Colors::primary()` |
| `.icon-detection:active .detect-b{fill:#39a029}` | `Colors::icon_pressed()` |
| `.icon-detection--active:hover .detect-a{stroke:#96ef89}` | `Colors::ring_hover()` |
| `.icon-refreshing:hover path{fill:#44d62c}` / `:active` `#39a029` | `Colors::primary()` / `icon_pressed()` |
| `.icon-power:hover rect{fill:#7de36c}` | `Colors::power_hover()` |
| `.icon-power--power-off rect{fill:#c8323c}` / `:hover` `#d97077` | `Colors::power_off()` / `power_off_hover()` |
| `.icon-warning:hover path{fill:#feab59}` / `:active` `#b15e0c` | `Colors::warning_hover()` / `warning_pressed()` |
| `.icon-close-glitter:hover path{fill:#c8323c}` | `Colors::remove_hover()` |

`.common-transition,… .icon-detection,…{transition:all .3s}` is declared on the
icon roots (and on the `:hover` state of the refreshing/warning paths), not on
the recolored glyph, so the source's hover/active recoloring is immediate. The
earlier local 300ms hover opacity fade was an approximation and has been
replaced by those colors; `.port-name-container .port-edit-icon` declares no
hover color, so the pencil keeps its extracted `#44d62c` paint.

The auto-detection icon keeps the source keyframes: `zoomout` scales `detect-b`
from .8 over 50ms, `zoomoutc` expands the `#44d62c` `detect-c` square from .4 to
2 while its paint fades, `zoomoutf` expands the `#666` `detect-f` square from .2
to 1.4 (both 700ms), and the enabled icon plays `zoomin` on the glyph (100ms,
opacity 0 -> 1, scale 1.8 -> 1). `transform:scale()` around
`transform-origin:center` is reproduced by animating each centered layer's size,
and `ease-out` (cubic-bezier(0, 0, .58, 1)) is sampled through
`Easing::EaseOut`. The inactive expansion only plays after a detection request,
matching `isFirstRender`; `reduce_motion()` keeps the static layer. The enabled
icon's ring, not its glyph, carries the hover/pressed stroke.

## Port rows: hover-revealed remove glyph and `.port-add-bend`

`MU` renders one `.port-item` per strip/bend and mounts `pU` (the
`.icon-close-glitter` svg, `20x20`, `fill:#ccc`) only when `index !== 0`. The
stylesheet hides it until the row itself is hovered:

```css
.port-item .icon-close-glitter{display:none;margin-left:-10px}
.port-item:hover .icon-close-glitter{display:inline-block}
.port-item:hover .icon-close-glitter:hover path{fill:#c8323c}
```

`display:none` means the glyph also occupies no space until its row is hovered,
so the native row tracks its own hover (`hovered_segment`) and only then mounts
the remove control, pulled 10px left exactly as declared; the glyph's own hover
color is the already extracted `currentColor` layer (`Colors::remove_hover()`).

The add-bend/add-fan control differs between the two products' own stylesheets,
so the native row branches on the product:

| | 3884 | 3886 |
|---|---|---|
| `.port-add-bend` | `color:#707070;cursor:pointer;display:inline-flex;height:27px;margin-bottom:5px;margin-top:5px` | the same **plus** `text-decoration:underline` |
| underline | `.underline{position:relative}` with `:after{background-color:#707070;bottom:.6px;content:"";height:1px;left:0;position:absolute;width:100%}` | none declared (the row's own `text-decoration`) |
| hover | `.underline:hover{color:#44d62c}` and `.underline:hover:after{background-color:#44d62c}` | no hover color declared |

Both selectors sit in the `.common-font-styles` group, i.e. Roboto 14px. The
native control therefore sets Roboto 14px, `#707070` and 27px with `margin:5px 0`;
3884 renders the 1px line as a bottom border that turns `#44d62c` together with
the text (GPUI has no `::after` pseudo-element, so that rule maps to the
border), while 3886 uses `text-decoration:underline` and keeps `#707070` on
hover. The strip/fan mount condition and the `S() >= t` disabled case were
already audited and are unchanged.

`python tools/validate-wireless-argb.py` asserts both products' declarations and
the native markers for both rules.

## Assets and integration

`assets/synapse/wireless-argb-manifest.json` records 54 source/output SHA-256
receipts for both products: 30 extracted assets (product art, fan/strip
diagrams, detection art and source inline SVG icons) and 24 derived interaction
layers (the auto-detection glyph/green/gray/ring layers, the refresh, remove,
warning, power and power-off recolored layers, and the two power bases that drop
the recolored rect). `prepare-wireless-argb.py` fetches only exact current
manifest URLs when missing, converts bitmap data with Pillow, copies safe SVG
data, and asserts the audited hover/active declarations and keyframe numbers
before deriving a layer. Main resource embedding is integrated by the root task.

Every resource receipt uses the shared `source`, `source_sha256`, and
`output_sha256` schema. For inline artwork, `source` is its current JavaScript
bundle and `source_sha256` hashes that bundle; `embedded_sha256` separately
hashes the statically decoded image data. The validator checks both hashes,
the extraction declaration and the output, so the global resource preparation
can validate inline assets without skipping their source provenance.

Public seam: `WirelessArgb::new(&Device, window, cx)`, `supports_page(pid,key)`,
`snapshot()`, `restore(saved,window,cx)`, `dismiss(window,cx)`,
`WirelessArgbChanged`, and `open_preview(window,cx)`.

Permitted verification:

```text
node tools/extract-wireless-argb.cjs --check
.work/resource-env/Scripts/python.exe tools/prepare-wireless-argb.py
python tools/validate-wireless-argb.py
rustfmt --edition 2024 src/features/wireless_argb.rs src/features/wireless_argb/state.rs src/features/wireless_argb/preview.rs
cargo check --locked --all-targets
```

No application, builds, tests, downloaded JavaScript, installers or DLLs ran.

## 2026-10-06 检测/刷新图标的提示是即时挂载（尚未接入本地）

源 3884（`.ref/devices/3884/static/js/main.607cfa0c.js`，3886 是同一组件的 ES5 转译）：

```jsx
<div id="icon-detection-wrapper" className="icon-detection-wrapper"
     onMouseEnter={() => this.toggleTooltipDetection(true)}
     onMouseLeave={() => this.toggleTooltipDetection(false)}
     onClick={this.toggleAutoDetection}>…</div>
<Gu isMounted={this.state.toggleTooltipDetection} position="bottom-left"
    target="icon-detection-wrapper">{getTextItem(OT.iyT)}</Gu>
```

`Gu`（类 `hu`）的行为：`showTooltip` 里 `setState({shouldRender:!0})` 立刻挂载，回调里只用一个
**0ms** `setTimeout` 加 `.show`，由 `.tooltip-razer .main{transition:opacity .1s linear}` 完成
100ms 淡入；`hideTooltip` 立刻去掉 `.show`（淡出 100ms）并在另一个 0ms 定时器后卸载——**全程没有
展示延迟**。位置 `position:"bottom-left"` 由 `Uu(target,"bottom-left")` 算出
`{x: target.left+320-target.width, y: target.top}`，容器 `position:fixed` 后内层
`.main{right:0;top:100%;margin-top:5px}`，即**右缘对齐目标、位于其下方 5px**。同一组件还有
`bottom-right`/`bottom-left-edge`/`bottom-right-edge`/`top` 等位置。皮肤沿用 `.tooltip-razer`
（`#000`/`#5d5d5d`/14px/`8px 10px`），与全局 `[tooltip]` 一致。

本地实现（2026-10-06 第三轮起共用 `crate::ui::hover_tip`）：提示本体已经抽到共享模块
`src/ui/hover_tip.rs`——`SourceTipPlacement{BottomLeft,BottomRight,Bottom,Top}` 与
`source_hover_tip(id, text, placement)`，按当前源的 `.tooltip-razer` 规则实现：300px 主框、
`justify-content` 决定左右对齐、`bottom-*` 用 `top:100%;margin-top:5px`、`top` 用
`bottom:100%;margin-bottom:5px`、`bottom`/`top` 再用 `left:50%;margin-left:-150px` 居中，皮肤
`#000`/`1px #5d5d5d`/`#ccc`/Roboto 14px/16px/`8px 10px`/`text-align:left`，并只保留源里那
100ms 的 `transition:opacity .1s linear` 淡入。3884/3886 与 778/3871 共用它。

原始实现（同日第二轮）：`src/features/wireless_argb.rs` 的检测/刷新按钮不再用
Kit 的 `Button::tooltip(text)`（该 API 只有 `tooltip_placement(Top/Bottom/Left/Right)`，没有延迟
设置，也无法表达右缘对齐 + 下方 5px），而是按源自绘：

- 两个带 id 的包装元素 `#icon-detection-wrapper` / `#icon-refreshing-wrapper`（`div().id(..)
  .relative()`）各挂 `.on_hover(cx.listener(|this, hovered, _, cx| …))`，直接把
  `hovered_icon = Some(ArgbIcon::Detection|Refresh)` 或 `None` 写回，对应源的
  `toggleTooltipDetection` / `toggleTooltipRefresh`——**即时挂载，没有展示延迟**；
- 悬停时在包装元素内挂 `argb_icon_tip(...)`：`.absolute().top_full().right_0()
  .mt(5px)`（右缘对齐、下方 5px，等价 `.tooltip-razer.bottom-left`），皮肤取
  `TooltipColors::{background,border,foreground}`（`#000`/`#5d5d5d`/`#ccc`）+ Roboto 14px/16px +
  `8px 10px` 内边距 + `whitespace_nowrap()`，并用
  `with_animation(.., Animation::new(Duration::from_millis(100)), |tip, delta| tip.opacity(delta))`
  复现源里那 100ms 的 `.show` 淡入。

上一轮（同日早些时候）失败的原因已查明并记录：gpui 的 `StatefulInteractiveElement`（`.on_hover`
的来源，路径正是 `gpui_kit::StatefulInteractiveElement`）只对**带 `id` 的元素**成立，当时的内联写法
`div().relative().on_hover(...)` 没有 `.id(..)`，所以只报 “no method named on_hover”。源里这两个
提示的目标本身就是带 id 的 `#icon-detection-wrapper` / `#icon-refreshing-wrapper`，因此补 id 既
修好了编译也与源结构一致。

`tools/validate-wireless-argb.py` 现在同时钉住源（现代/ES5 两套片段）与本地实现（enum、状态字段、
两个 id、翻转语句、提示函数、`.tooltip-razer` 皮肤、100ms 动画、`.absolute().top_full().right_0()
.mt(5px)`，以及两个图标不再使用 `Button::tooltip`），任何漂移都会失败。

## 2026-10-06 LED 数量提示（`bottom-right`）

源 3884/3886 的端口行里，检测结果是一个 `Gu position:"bottom-right"` 提示，内容由
`getTextItem(OT.vml, {ledCount: '<span style="color:#44d62c">N</span>'})` 生成——**只有数字是主题绿**，
其余是普通提示文本；提示容器（`.tooltip-razer{position:absolute;height:100%;width:100%;top:0;left:0;
z-index:1060}`）自己覆盖目标并监听鼠标，所以没有额外延迟，仍然是即时挂载 + 100ms 淡入。
`bottom-right` 意味着 `.main{justify-content:flex-start;left:0}` 且 `top:100%;margin-top:5px`，
即贴目标下沿、**左缘对齐**。

本地改动（`src/features/wireless_argb.rs`）：检测结果按钮包进
`div().id(("argb-detected-wrapper", id)).relative()`，`.on_hover` 维护
`hovered_detected: Option<u32>`（按端口即时挂载），悬停时挂
`source_hover_tip_element(("argb-detected-tip", id), SourceTipPlacement::BottomRight, …)`，内容按
`{{ledCount}}` 拆成前缀/绿色数字/后缀三段。`tools/validate-wireless-argb.py` 同时断言源的
`position:"bottom-right"`/`dangerouslySetInnerHTML`/绿色 `ledCount` 片段与本地标记。

## 2026-10-06 端口帮助控件（`.widget .help` + `.tip`）

源 3884/3886 的端口卡片是 `.port-container.widget`，帮助入口是它的直接子节点：

```jsx
<div className="port-container widget">
  <div className="help"/>
  <div className="tip">{getTextItem(OT.gt9 /* GLITTER_TIP_HELP_PORT_MESSAGE */)}</div>
  …
```

即**不是**普通的按钮 + tooltip，而是 `.widget .help`（
`.widget .help{background-color:#4a4a4a;border-radius:50%;height:14px;position:absolute;
right:10px;top:10px;width:14px;transition:background-color .3s}`、`:hover{background-color:#ffffff4d}`）
加 `.widget .tip`（`max-width:300px`、14px/18px、`padding:8px 10px`、黑底 `1px #5d5d5d`、`#ccc`）。

本地改动（`src/features/wireless_argb.rs`）：不再自绘 20px 的 `help-default.svg` 按钮，改用本仓库
已按源实现的 `surface::help_control`（它本身就是这个 14px 圆形控件 + `.widget .tip`，内部走
`SourceTooltipKind::WidgetTip`），并用 `.absolute().right(10px).top(10px)` 的包装补回源里的绝对定位；
卡片容器补 `.relative()`。`tools/validate-wireless-argb.py` 新增源标记
（`className:"port-containerwidget",children:[`、`className:"help"`、`className:"tip"`）与本地标记，
并要求旧的 `.tooltip(self.spec.text("GLITTER_TIP_HELP_PORT_MESSAGE"))` 与 `help-default.svg` 消失。
