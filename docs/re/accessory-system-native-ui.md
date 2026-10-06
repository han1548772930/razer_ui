# Current monitor and cooling accessory controls

Status: native local controls implemented for five products and nine non-lighting
pages; hardware integration and full rendered parity remain incomplete.

Only the current `.ref/devices/<pid>/` sources are implementation evidence.
`tools/extract-accessory-system-products.cjs` resolves webpack exports and mounted
components by static AST traversal. It does not execute the downloaded program.
`tools/prepare-accessory-system-products.py` verifies every source file hash and
prepares the native data. Source paths, hashes, offsets and extracted JSX are in
`accessory-system-source.json`; the compact receipt is
`accessory-system-native-audit.json`.

| Product | Native pages | Implemented local settings |
| --- | --- | --- |
| 3858 Raptor 27 | Gaming, Color, Display | Six gaming presets, brightness/contrast/overdrive, Gamma 1.4/1.8/2.2, color temperature and custom RGB, input source, PIP/PBP, adaptive sync, HDR and refresh-rate-counter position |
| 3880 Raptor 27 165Hz | Gaming, Color, Display | Above plus Gamma 2.4, Native/Rec.709/DCI-P3 gamut and THX Cinema; installed ICC profiles and supported refresh rates show unavailable data |
| 3893 Hanbo | Performance | Separate fan and pump mode requests; actual modes, RPM and curves remain unknown until hardware supplies them |
| 3900 PWM controller | Performance | Eight profile port settings, linked fan mode, Quiet/Normal/Performance/Manual/Advanced, manual speed and monotonic nine-point curve |
| 3907 Laptop Cooling Pad | Performance | Enable, Fixed/Smart modes, separate fixed preset values and limits, independent CPU/GPU smart presets, Celsius/Fahrenheit, percent/RPM display and selected preset reset |

Lighting remains in the supplemental source-controls workspace. The generator
now supplies 3893/3907 lighting values from their own reducer seeds because their
`DEFAULTPROFILE` objects contain only profile identity. The provenance is recorded
in `accessory-controls-audit.json.initial_sources`. Hanbo's current `hideLightingIdle`
is respected. Product 3929 mounts its lighting within its sole Customize page and
does not gain a fabricated Lighting tab.

## Behavior verified against current source

- Monitor Gaming displays the selected preset's data. The first manual change
  copies that preset into `customData` and selects Custom, matching the reducer;
  the unused `DEFAULTPROFILE` custom values are not shown as an active preset.
- Overdrive values are Off 0, Weak 1, Strong 2. Gamma 3 is exposed only on 3880.
  Only 3880 mounts the color-gamut control. Non-Native gamut disables contrast
  and gamma; a selected Custom non-Native gamut also disables color controls.
- PIP sizes are Small 1, Medium 2, Large 3, with corner positions 0–3. The refresh
  rate counter uses its distinct position values 1–4. Secondary input choices
  are DP 15, HDMI 17 and USB-C 19; Auto 0 belongs only to the primary source.
- PIP disables HDR and Adaptive Sync. On 3880, enabled THX Cinema disables
  Gaming/Color; sRGB color temperature disables THX Cinema. Windows HDR and
  duplicate-display state cannot be inferred from local requested values.
- PWM manual fan speed is 33–100% step 1. Its advanced chart clamps to 25–100%
  and between adjacent points. Enabling linking copies the selected mode;
  Manual copies its speed, Advanced its curve. This is local profile intent,
  with no fabricated active-port detection or 1250-RPM demonstration values.
- Cooling Pad fixed modes use Low 500–2000, Medium 1500–2500 and High
  1900–3200 RPM, step 50. Smart uses the source's independent CPU/GPU presets,
  500–3200 RPM and monotonic adjacent-point constraints. Display percentage is
  RPM/3200, matching the source's 15.625% lower endpoint. The seeded `cpu_gpu`
  data is retained but no extra sensor tab is invented.
- Slider entities and subscriptions are retained outside rendering. Restoring
  a snapshot resets against the current source schema, rejects unknown fields,
  validates choice values, fixes port IDs and temperature coordinates, preserves
  fixed array lengths, clamps numbers and does not emit a user-change event.
- Page selection always accepts the shell's requested source key. Unsupported
  pages show an explicit incomplete state, never the previous page's controls.

## Remaining work

- Monitor source product images, input-source confirmation overlay, source PIP
  placement diagram, ICC install/select commands, detected refresh rates,
  Windows HDR integration and complete backend-supplied UI constraints.
- Hanbo detected fan/pump curves, hardware limits and update-required states.
  The current source starts with `ports: []`; no usable curve default exists.
- PWM port artwork, renaming, detection and active-port-only hardware routing.
- Cooling Pad smart curve add/remove node interactions (source supports 2–20
  nodes), sensor/system status panels, compatible Blade performance navigation,
  hardware capability gates, HyperBoost and mapped-button controls.
- Interactive dragging on the source chart is currently represented by native
  retained sliders and a line plot. Exact source geometry and measured pixel
  alignment are not verified.
- These local drafts do not invoke hardware commands or present success as a
  device acknowledgement. Persisted local settings are separate from observed
  device state.

`cargo check --locked --all-targets` passed after workspace integration on
2026-10-03. No application, build, test, installer, downloaded JavaScript or DLL
was executed. Rendered appearance and input behavior remain unverified under
the user's no-run restriction.

## 2026-10-04 PIP 组件（`eSA`）按原代码重做

原先的 PIP 面板用了三组自造按钮（模式按钮、输入源按钮、位置/尺寸文字按钮行），与当前
3858/3880 源码不符。现在按原组件重建：

- **屏幕模型 `qAA`/`ZAA`**：`.Screen_screen_wrapper` 为 `inline-flex;flex-direction:column`；
  `.Screen_screen` `330x196`、`border:1px solid #5d5d5d`、`border-radius:5px`；
  `.Screen_main_area` `height:186px;position:relative`（子内容的定位上下文）；
  `.Screen_footer` `background:#5d5d5d;height:calc(100% - 186px)`（=10px，下方 2px 圆角）；
  `.Screen_foot` `130x30`、`background:#222`、`margin:2px auto 0`。刷新率计数器的 2×2 网格与
  PIP 选择器都挂在这个屏幕模型里。
- **PIP 选择器 `JAA`**：`QAA` 给出 12 个预设（3 尺寸 × 4 角，`id = "<size>-<position>"`）：
  SMALL `90x49.5`（z 3）、MEDIUM `120x66`（z 2）、LARGE `151x83`（z 1），角位由
  `left/right` + `top/bottom` 决定；`.PIP_item` 是 `1px dashed #cccccc1a` 的绝对定位框，
  `:hover` 变 `#ffffff1a` + `#44d62c`；指针进入选择器后 wrapper 进入 `PIP_item_hover` 状态，
  所有 item 边框变 `#ccc`、悬停项与 `.PIP_source` 变 `#44d62c`，并且
  `.PIP_source span{display:none}` 把输入源名字隐藏。选中项的 `.PIP_source` 用该预设的
  内联几何（`#222` 底 + `1px solid #5d5d5d` 边框，文字 Roboto 14px/17px `#999`）。本地按
  源码的 z 序绘制（1→2→3，同 z 时源框先于 item），点击写入
  `/secondDisplay/pipSetting/{position,size}`（`Ss` 0–3；尺寸 1–3）。
- **右侧栏 `.pip_display` / `.action_wrapper`**：`margin-left:20px` 的竖列；`MODE` 标题
  （`#ccc`、Roboto 14px/17px、`margin-bottom:8px`）；`.action_button_group` 是
  `width:290px`、`flex-direction:column`、带 `mb20` 的按钮组，两个 `.btn_custom`
  各 `144x80`、`margin:10px 10px 0 0`，继承共享 `.btn`
  （`color:#fff;font-size:12px;line-height:14px;padding:6px 0 7px;transition:opacity .3s`、
  `:active{opacity:.6}`）；PBP 追加 `margin-top:10px`。微型屏幕图形：`child_pip`
  `51x28`、右下、去掉右/下边框；`child_pbp` 右半 `50%` 高 100%、左边框；PBP 文字
  `top:40%`、居中、`z-index:2`。选中态 `background:#222;border-color:#44d62c`。
- **输入源改为下拉**：`C6O` 接收 `$AA`（本地 `PIP_SOURCES`，顺序 DP_1 15、HDMI_1 17、
  USB_C 19，标签键 `DP_1`/`HDMI_1`/`USB_C`），本地用共享 `Select` 组件，值来自本地意图
  `/secondDisplay/source`，不是设备观测值。
- **标题键按源码更正**：`nc3` → `MODE`，`dso` → `SOURCE`（不是 `SCARLETT_SOURCE`），
  `JXS`/`IMU`/`NIR` → `PIP_HEADER`/`PBP`/`PIP_TOOLTIP`（在 3858 的语言别名表里逐一核对；
  表内还可见 `SCARLETT_INPUT_SOURCE_TOOLTIP`、`FREE_SYNC_TOOLTIP`、`HDR_TOOLTIP[_WINDOWS_11]`、
  `FPS_COUNTER_TOOLTIP` 等，与上一轮使用的键一致）。
- **一处按源码保留的溢出**：`.body-widgets .widget` 固定 `600px`、内边距 `30px 40px`
  （内容宽 520px），而 `.pip_display` 一行是 `330 + 20 + 290 = 640px`，原版本身就会横向
  溢出组件框。本地保持同样的固定尺寸（不做收缩），以免"看起来更整齐"却偏离原版。
- 仍**未**接入：`.btn` 的 `transition:opacity .3s` 目前是即时切换（与仓库内其它
  `.btn:active{opacity:.6}` 的实现一致）；屏幕模型的 1px 边框在本地按 GPUI 的盒模型计入，
  与原版 content-box 有 2px 级别差异，未逐像素验证。

### 下一步已取得的证据：输入源确认浮层

主输入源组件 `SSA` 的 4 个按钮（`btn btn_custom`，带 `TSA.A`/`sSA`/`ISA`/`rSA` 图标与
`SCARLETT_AUTO`/`HDMI_1`/`DP_1`/`USB_C` 标签）点击后走 `confirmInputSourceChange`：
若 `shouldAskAgainValue` 为真则先弹出确认框，否则直接切换。确认框是
`.alert_wrap > ASA → OSA`：

```jsx
<div id="confirmationAlert" className={"flex alert profile-del " + (active ? "show" : "")}>
  <div className="del-title">{title}</div>
  <div className="body-text t-center">{confirmationDescriptionMsg}</div>
  <znA id="shouldAskAgain" onCheck={...} name={shouldAskAgainMsg} active={...} extraClass="checkBoxAlignment" />
  <div id="confirmationButton" className="thx-btn" onClick={confirmAction}>{confirmationMsg}</div>
</div>
```

行为与键：`confirmationMsg = SCARLETT_CONFIRM`（英文表值 `CONFIRM`）、
`confirmationDescriptionMsg = SCARLETT_CONFIRMATION_DESC`（`CONFIRMATION_DESC` =
"This might switch you away from the PC running Synapse."）、
`shouldAskAgainMsg = SCARLETT_CONFIRMAION_TEXT`（本地键即为带拼写错误的 `CONFIRMAION_TEXT`
= "Don't ask me again"）；本地 `locales/*.json` 缺 `SCARLETT_*` 三个键，需要用对应的基础键；
点确认框外部任意处会以 `confirmAction(false)` 取消。

## 2026-10-04 色彩页按源码重做（`wAA`/`nSA`/`NSA`）

3858 的 `TAB_COLOR` 是 `xAA → zAA → wAA`，3880 是 `NSA`，两者共用色彩温度组件：

- **组件外壳**：`wrA` 带 `title: COLOR_TEMPERATURE_HEADER`、`tips: COLOR_PROFILE_TOOLTIP`
  （键由别名表核对：`Lyf`/`iQK`）、`hasSwitch:false`；本地此前没有提示按钮，现已补上。
- **预设按钮**：`.btn_group`（`gap:10px`）里的 `btn btn_custom`，顺序照 `Object.entries(GM)`
  （`NORMAL:5`、`LOWBLUELIGHT:12`、`WARM:4`、`COOL:8`、`SRGB:1`、`CUSTOM:11`），标签是
  `KAA["SCARLETT_"+name]` 解析出的基础键 `NORMAL`/`LOW_BLUE_LIGHT`/`WARM`/`COOL`/`SRGB`/
  `SCARLETT_CUSTOM`（十种语言齐备）。此前本地用的是 outline 按钮，且标签走 `t_or` 兜底。
- **红/绿/蓝三行**：包在 `displayClasses = "slide-off"` 的块里，选中 Custom 时追加
  `slide-on`；CSS 是 `.slide-off{display:none}` ↔ `.slide-off.slide-on{display:block}`
  （`display` 不参与过渡，所以原版实际是立即显隐，本地按需挂载即等价）。三行是 `STA`，
  只传 `featureNameText`（`JJv`/`Nls`/`uiz` → `RED`/`GREEN`/`BLUE`）与 `value`：
  **没有** `tooltipText`、**没有** `enableSliderRange`（`.foot` 灰标因此都渲染为空字符串），
  但 `maxStep` 默认 100，所以带 50×27 的数值输入框（本地已为该三行注册 `add_percent_input`）。
- **3880 是两列网格**：`NSA` 是 `div.body-widgets.flex` + 两个 `tSA`（`_SA` 渲染为
  `div.widget-col.col-left|col-right`，CSS `.widget-col{flex-direction:column;
  height:fit-content;width:600px}`、`.body-widgets{flex-direction:row;flex-wrap:wrap;
  justify-content:center;margin:auto;max-width:1240px}`）。左列 `rSA`（THX Cinema）+
  `dSA`（Color Profile），右列 `ISA`（HDR）+ `sSA`（色彩温度组件）。本地此前是单列堆叠，
  且顺序不同，现按 `COLOR_PAGE_COLUMNS` 常量驱动两列渲染（有测试固定列内顺序与唯一性）。
- **THX Cinema `rSA`**：`tTA` 外壳 `title: THX_CINEMA_HEADER`、`tips: THX_CINEMA_TOOLTIP`、
  `hasSwitch:!disabledReason`、`active: !disabledReason && isEnabled`、
  `customStyle:{zIndex:3}`；组件体是 `THX_CINEMA_DESC`（有禁用原因时挂 `.featureDisabled`）。
  本地改成把 `surface::SynapseSwitch` 放进标题行（见下）并切换 `/thxCinema/isEnabled`。
- **HDR `ISA`**：`title: HDR_HEADER`、`tips:` Windows 11 用 `HDR_TOOLTIP_WINDOWS_11`
  否则 `HDR_TOOLTIP`（本地 `hdr_tooltip_key` 已按 `system::is_windows_11()` 选择）、
  `hasSwitch:!disabledReason`、组件体 `HDR_MSG`。本地此前用带标签的复选框行，现改为标题行开关。
- **组件外壳 `_TA`/`tTA`（新证据）**：`div.widget-container > div.widget[.hyperPolling]
  [extraClass][.increase-zindex][.disabled]`，内部依次是
  ① `tips` 存在时的 `div.help`（`role=button`，点击区域固定右上角 `.widget .help{right:10px;top:10px}`，
  悬停经 portal 显示 `.tip`）；② `hasSwitch` 时的 `div.widget-switch > div.switch[.on][.disabled]
  > div.handle`，**放在 `.titleRow > .title`（`display:flex`）里、紧跟标题文本**；
  ③ `title !== ""` 时的 `div.titleRow > div.title{color:#44d62c;font-family:RazerF5;font-size:16px;
  text-transform:uppercase}`；④ 子内容；⑤ `centerTips` 的 `.widget-body-tooltip`。
  为此在 `ui/surface.rs` 新增 `panel_with_title_switch(title, title_control, corner_control, cx)`，
  把开关放进标题文本之后，帮助按钮仍留在右上角。
- **Color Profile `RSA`**：`title: PERFORMANCE_MODE_SCREEN_COLOR_PROFILE_HEADER`、
  `tips: PERFORMANCE_MODE_SCREEN_COLOR_PROFILE_TOOLTIP`（`ovc`/`o5Y` → `ha`/`Ga`），
  组件体是 `.screen-refresh-container[.featureDisabled] > .laptop-screen > .content
  {padding-left:0}` 里的配置文件下拉 `k6O`（`z6O`：数据来自扩展上报的 `colorProfiles`，
  选项 ≤1 时禁用，且数据为空时用一条 `{content:""}` 兜底），下面是
  `.img-text .external` 外链：`color:#ccc;font-size:14px;line-height:44px;
  text-decoration:underline;text-transform:capitalize`、`hover{color:#44d62c}`、
  `active{opacity:.7}`，点击走 `openColorManagement`（`simpleLaunchUserAppProcess(...,
  "colorcpl.exe")`，本地 `backend::system::open_color_management()` 启动同一程序）。
  本地**删掉了此前自造的“等待系统提供显示器色彩配置文件。”提示**：没有系统配置文件数据时
  就按原版自己的空数据分支渲染一条空选项的禁用下拉。`.laptop-screen` 的样式不在已抓取的
  CSS 里（`.screen-refresh-container` 只留下 `.dropdown-area{margin-left:0}`），
  因此不臆造它的外观，只保留结构与可核对的外链样式。
- **`uiRestraint` 设备约束**：当前源把 `monitor.uiRestraint` 的 truthy 条目直接作为
  `disabledReason`，传给 `THX Cinema`、`HDR`、`Color Profile`、`PIP`、`Adaptive Sync`、
  `Gaming`、`Color` 与刷新率计数器。`accessory_system_products.rs` 现在保留源字段名并在
  所有对应 `change` 路径、开关、按钮、滑条和 PIP 选择器上拒绝本地草稿写入，缺少服务
  字段时保持可编辑预览，不伪造设备限制。仍未接入的是真实宿主/设备服务对 `uiRestraint`
  的填充、系统色彩配置文件枚举，以及显示器产品图。
- **原因行 `zrA`/`iTA`（2026-10-06 重做）**：原因段落是每个组件体（`wrA`/`tTA` 之后）
  **第一条**子元素，源码里就是 `children:[<zrA text={disabledReason}/>, <div className=
  {featureDisabled}>…]`；本地此前把它渲染在控件**下方**，且缺 disc。现在
  `exclamation_line(text, margin_bottom, cx)` 按源码标记绘制 `p.exclamationText[ mb20]`：
  `p.exclamationText span{width:14px;height:17px;margin-right:5px}` 的行内盒，`:before` 是
  `#5d5d5d` 的 14px 圆点加 `tooltip_exclamationmark.cc8fb226.svg`（本地资源
  `synapse/accessory-exclamation.svg`），文字 14px/17px、颜色继承；`className` 默认
  `exclamationText mb20`（`.mb20{margin-bottom:20px}`），色域警告 `xrA` 覆盖成不带 `mb20`
  的 `exclamationText`，因此传 `0.`。`restriction_line(feature, cx)` 用它渲染
  `/uiRestraint/<feature>` 的原因，并按源码次序放回八个组件的首个子元素：Gaming（`RTA`/
  游戏模式页，原本完全缺失）、Color（`wAA`，原本完全缺失）、THX Cinema、HDR、Color
  Profile、PIP/second display、Adaptive Sync、FPS 计数器。3858 与 3880 的 `zrA`/`iTA`
  用点数量（6 / 9）、默认类名、首个子的位置、色域覆盖类名与 `GM` 预设枚举都已写进
  `tools/prepare-accessory-system-products.py` 的断言；同一次重建还补上了此前从
  `initial` 漏掉的 `uiRestraint`（源 reducer 种子里的空对象，见该工具的注释），因此
  `accessory_system_products_data.json` 与重建结果逐字节一致。
- **组件体 `.widgetContent` 的 20px 列间距（2026-10-06 接入）**：源把组件体包在
  `.widgetContent{display:flex;flex-direction:column;gap:20px}` 里，组件标题自己的
  `margin-bottom:20px`（`.widget .titleRow .title`）负责标题到第一条子元素的距离。
  `tools/audit-monitor-widget-layout.cjs` 逐包枚举了全部 `.widgetContent` 出现点：**只有
  3858/3880 两个显示器包用到它**（3893/3900/3907/3921、1383、164/241 均为 0 处），共 9 组：

  | 产品 | 标题符号（键） | 顶层子元素 | 本地 |
  |---|---|---|---|
  | 3858/3880 | `U$D`（SCARLETT_GAME_MODE_HEADER） | 6：预设组、亮度、对比度、超频、gamma、色域控件 | `surface::widget_content` |
  | 3858/3880 | `Lyf`（COLOR_TEMPERATURE_HEADER） | 2：预设组、`.slide-off` 自定义组 | `surface::widget_content` |
  | 3858/3880 | `JXS`（PIP_HEADER） | 1（内层 `div` 再分三块） | 间距不适用，未改 |
  | 3858/3880 | `mGs`（SCARLETT_FREE_SYNC_HEADER） | 1 | 间距不适用，未改 |
  | 3880 | `$T1`（REFRESH_RATE_HEADER） | 2（说明段 + 透明 Fragment：胶囊、链接段） | `surface::widget_content` |

  本地新增 `surface::widget_content(children)`（`v_flex().gap(css(20.)).children(…)`），
  只在源确实是 `.widgetContent` 的组件体里收集子元素：Game Mode 现在是「预设组 + 四行
  `STA` + 整个 `xrA` 色域控件」五个子元素各占一格（此前 `xrA` 的标题行/按钮组/警告被摊平成
  三个同级子元素，且没有任何间距），色彩温度页是「预设组 + `.slide-off` 组」两个子元素
  （`.slide-off` 是普通块容器，内部三行 `STA` 保持紧贴）。`panel`/`panel_with_control`
  本身不加 gap，因为不是每个组件体都是 `.widgetContent`（例如 `nSA` 的组件体是
  `div.mb20.flex`，THX/HDR/Color Profile 是单个 `div`），标题的 `mb(20px)` 保持不动。
  收据见 [monitor-widget-layout-current-evidence.json](monitor-widget-layout-current-evidence.json)，
  机检 `node tools/audit-monitor-widget-layout.cjs --check`。
### 2026-10-06 3880 显示页的刷新率组件（`jSA`）落地

3858 的 `TAB_DISPLAY` 右列第二块是 `NSA`（`title:Y0s`＝HDR），3880 同一位置换成 `jSA`
（`title:$T1`＝REFRESH_RATE_HEADER），而两列的 FPS 计数器（`nSA`/`HSA`）都在（见下面 2026-10-06
的显示页结构一节）；此前本地给这一块渲染的是自写的占位面板
（`surface::note("等待显示器提供支持的刷新率。")`，文案不是源文，已删除）。源的结构是：

```jsx
<tTA title={$T1} tips={ZYW}>                      // 没有开关，也没有 customStyle
  <iTA text={disabledReason}/>                    // 组件体第一条：zrA 限制说明
  <div className={"widgetContent " + (disabledReason ? "featureDisabled" : "")}>
    <p><Text text={M7n}/></p>
    {disabledReason ? null : <>
      <xSA activeId={selected} options={rates} onOptionClick={click}/>
      <p><Text text={XSA(Muv, {displaySettings: <span onClick={() => msSettings("display")}
         style={{textDecoration:"underline", cursor:"pointer"}}>{W_M}</span>})}/></p>
    </></div>
</tTA>
```

- **按键位解析的文案**（`H6O=a(4693)`，即显示页所在的 locale 模块）：`$T1` =
  `REFRESH_RATE_HEADER`、`ZYW` = `PERFORMANCE_MODE_SCREEN_REFRESH_RATE_TOOLTIP`、`M7n` =
  `PERFORMANCE_LAPTOP_SCREEN_GUIDE`、`Muv` = `ADJUST_REFRESH_RATE_DIALOG`（含
  `{{displaySettings}}` 占位符）、`W_M` = `WINDOW_DISPLAY_SETTINGS`；五个键在本地 10 个
  `locales/*.json` 里都已存在，因此不需要新增译文。
- **胶囊是 `PillsSelectBox` 组件**（`.PillsSelectBox_pillsContainer__E5ZcB` +
  `.PillsSelectBox_pillButton__-CgIZ` + `.PillsSelectBox_active__kObeU`）：容器
  `background-color:#111;border:1px solid #5d5d5d;border-radius:18px;display:flex;gap:5px;
  height:36px;padding:5px;width:fit-content`，`:hover{border-color:#44d62c}`；胶囊
  `background-color:#0000;border:0;border-radius:13px;color:#ccc;font-size:14px;height:26px;
  line-height:16px;padding:5px 10px;text-align:center`，`.active` 时
  `background-color:#44d62c;color:#111`，**没有 hover/active 规则**（本地相应地不给
  胶囊加 hover 态）。选项文本是 `"<n> Hz"`，来自 `supportedRefreshRate`（源先用
  `useState` 的 60/120/144/165 兜底，收到 MW 事件后替换成升序列表），选中项来自
  `selectedRefreshRate`；点击先乐观 `setSelected(id)` 再 dispatch
  `changeMonitorRefreshRate(value)`——本地同样立刻更新运行时观测值，服务动作留在边界外。
- **版本差异**：`.widgetContent` 的 React 子元素是「说明段 + 透明 Fragment」，Fragment 不
  产生 DOM 节点，所以实际是三层元素各占 20px；本地用上一轮加的
  `surface::widget_content([说明段, 胶囊, 链接段])` 渲染，并用
  `.opacity(0.3)`（对应 `.featureDisabled{opacity:.3;pointer-events:none}`）与点击守卫表达
  禁用态。
- **`displaySettings` 链接的动作链**：源 `RiA.A.msSettings("display")` → Electron 宿主动作
  `msSettings`（`payload:{actionArgs:"display"}`）→ 宿主 `sysutil/win` 原生模块
  `msSettings:["void",["string"]]`。该模块是 DLL、静态不可读；应用内可读的同族写法是
  Dashboard 动作表的 `case"PowerUserMenu":return"cmd /c start ms-settings:"`，因此本地
  `backend::system::open_display_settings()` 用同一种 `cmd /c start ms-settings:display`
  打开源参数指定的 `display` 页（页面名来自源，方案来自应用自身的同类动作）。
- 机检：`tools/audit-monitor-widget-layout.cjs` 通过 `H6O=a(4693)` 定位 locale 模块、逐个
  断言 5 个符号→键的映射、检查这些键在 10 个语言包里都存在、逐条比对 4 条胶囊 CSS 声明、
  确认 `msSettings("display")` 与本地全部标记，并断言那段自写占位文案不再出现。

### 2026-10-06 显示页的两列结构与标题行开关

`TAB_DISPLAY` 的源码结构是 `.body-widgets.flex`（`flex-direction:row;flex-wrap:wrap;
justify-content:center;margin:auto;max-width:1240px`）里两个 `.widget-col`（`flex-direction:column;
height:fit-content;width:600px`），每个组件本身是 `.body-widgets .widget`（`min/max-width:600px;
margin:10px auto;padding:30px 40px`）。两个产品的列内顺序不同：

| | 左列 | 右列 |
|---|---|---|
| 3858 | `lSA` 来源、`eSA` PIP | `RSA` 自适应同步、`NSA` HDR、`nSA` FPS 计数器 |
| 3880 | `WSA` 来源、`mSA` PIP | `KSA` 自适应同步、`jSA` 刷新率、`HSA` FPS 计数器 |

本地此前把 5 个组件串在一个 `v_flex` 单列里，且 3880 的刷新率组件排在 FPS 计数器之后；本轮改成
同样的两列 600px 结构并按上表排序（`.child(input_source).child(pip)` 与
`.child(free_sync).child(third).child(fps_counter)`，`third` 为 3858 的 `hdr_widget` / 3880 的
`refresh_rate_widget`）。

`wrA`/`tTA` 外壳把开关渲染在 `.titleRow > .title` 里（`display:flex` 的标题文本之后），
**只有 `hasSwitch` 为真才有 `.widget-switch` 节点**：

```jsx
H = s ? <div className="widget-switch"><Switch disable={this.props.disable} active={U} …/></div> : null
v = <div className="titleRow"><div className="title"><Text/>{H}{A}</div><div>{O}</div></div>
```

显示页各组件自己的声明因此各不相同：`JXS`（PIP）`hasSwitch:!a`（有禁用原因时**不渲染**开关，
`active:isEnabled`）、`mGs`（自适应同步）`hasSwitch:!e.disabledReason` 且
`active:!disabledReason&&isEnabled`、`jXh`（FPS 计数器）`hasSwitch:!0`、`Y0s`（HDR）
`hasSwitch:!e.disabledReason`、`Q$9`（来源）`hasSwitch:!1`；3880 的 `lAx`（THX）同样是
`hasSwitch:!e.disabledReason`。本轮新增
`surface::panel_with_title_switch_opt`（`None` 时走不带开关的 `panel_with_control`，即
`hasSwitch:false` 的等价物），PIP / 自适应同步 / FPS 计数器 / HDR / THX 五个组件改用它，删掉了
此前身体里的 `self.toggle(...)` 行（原版没有那种带文字标签的复选框行），并补上自适应同步组件体
里缺失的 `FREE_SYNC_MSG`（`.widgetContent > div[.featureDisabled]`，只有正文文字
`opacity:.3`）。FPS 计数器里此前还有一条 `restriction_line("refreshRate")`，但 `nSA`/`HSA` 根本
不读 `uiRestraint`，禁用原因属于 `$T1` 刷新率组件，本轮删除。

`node tools/audit-monitor-widget-layout.cjs --check` 现在同时校验：两页的列组成字符串、五个
组件的 `hasSwitch` 声明、本地 `panel_with_title_switch_opt` 与五个开关标记、`FREE_SYNC_MSG`、
两列的本地顺序标记，以及被替换掉的 `self.toggle(...)` 行不再出现。

## 2026-10-04 游戏模式页（`RTA`）按源码重做

3858 的 `TAB_GAMING` 是 `NTA → dTA → RTA`，3880 是 `FTA`，两者都走同一个 `RTA` 结构；
此前本地用的是「outline 按钮 + 文字标签 + 滑条」的混合形态，本轮按源码重建：

- **组件外壳**：`wrA{title: SCARLETT_GAME_MODE_HEADER, tips: SCARLETT_GAME_MODE_TOOLTIP,
  hasSwitch:false, active:!disabledReason, customStyle:{marginBottom:"30px"}}`；本地用
  `panel_with_control` + `help_control` 挂提示，并加 `.mb(30px)`。
- **预设按钮**：`.btn_group`（`gap:10px`）里的 `btn btn_custom`（与输入源按钮同一套
  `.btn_group .btn_custom` 度量），顺序照 `zH` 枚举 DEFAULT 0、FPS 1、MMO 3、RACING 2、
  STREAMING 4、CUSTOM 5，标签 `lTA["SCARLETT_"+name]` 与本地键一一对应。
- **`STA` 行**：`.slider_header.mb10`（`display:flex;align-items:center;position:relative`）
  含功能名、内联 `.help`（`.slider_header .help{margin-left:5px;position:relative;
  right:auto;top:auto}`）与——仅当 `maxStep===100`——`.slider_header input`
  （`50x27`、`#111` 底、`1px solid #5d5d5d`、Roboto 14px、`padding:5px 6px`、
  `margin-left:10px`，原版是 `<input maxLength=3 pattern="(100)|(0*\d{1,2})">`）。
  本地新增 `valid_percent_draft`（≤3 位数字且 ≤100，测试覆盖 0/5/05/42/99/100 与
  空串、101、1000、1e2、-1、12a、"100 " 的拒绝）与真实 `InputState`（Enter/失焦提交，
  经 `sync_sliders` 双向同步；原版是每次按键即提交，这一点见下方偏差）。
- **离散滑条与标签**：overdrive 与 gamma 在原版里是**滑条**（`maxStep` 2 / 2 或 3，
  `enableSliderRange`），不是按钮行；灰标 `.foot` 的位置照 CSS：
  `.foot{bottom:-2px;position:absolute;text-transform:uppercase}`、`.foot.min{left:0}`、
  `.foot.mid{left:0;width:100%;text-align:center}`、`.foot.mid1{left:0;width:66%}`、
  `.foot.mid2{left:0;width:133%}`、`.foot.max{right:0}`（即 0 / 33% / 66.5% / 100% 四个锚点）。
  overdrive 标签取语言键 `OFF`/`WEAK`/`STRONG`（`LpY`/`CAI`/`FzV`）；gamma 标签是字面量
  1.4 / 1.8 / 2.2，3880 的扩展滑条再加 boost 标 2.4（`gaming_gamma_steps`/`gaming_gamma_tags`
  承载并有测试）。
- **提示键核对**（用包内别名表逐一解析）：`kwu`=SCREEN_BRIGHTNESS_TOOLTIP、
  `Zli`=CONTRAST_TOOLTIP、`iT`=OVERDRIVE_TOOLTIP、`bZl`=GAMMA_TOOLTIP、
  `ybq`=SCREEN_BRIGHTNESS_HEADER、`YmK`=CONTRAST_HEADER、`wyX`=OVERDRIVE_HEADER、
  `tbO`=GAMMA_HEADER、`U$D`=SCARLETT_GAME_MODE_HEADER、`qek`=SCARLETT_GAME_MODE_TOOLTIP，
  与本地此前的键一致。
- **色域控件 `xrA`**：`slider_header mb10`（标题 `COLOR_GAMUT` + `COLOR_GAMUT_TOOLTIP`）+
  `.btn_group mb10` 的三个 `btn btn_custom`（`krA`：NATIVE / REC 709 / DCI-P3，枚举
  `B4={NATIVE:0,DCI_P3:1,REC_709:2}`）+ **仅当不是 Native 时**才渲染的 `zrA` 警告
  （文案键 `COLOR_GAMUT_WARNING`）。`p.exclamationText span:before` 是
  `background:#5d5d5d` 的 14px 圆点加 `tooltip_exclamationmark.cc8fb226.svg`，
  该文件在 3858 抓取里缺失（只抓了 js/css），但 Macro/Profiles 应用里有字节相同的副本，
  已作为 `synapse/accessory-exclamation.svg` 纳入资源清单（`tools/prepare-resources.py`
  记录来源与哈希，`validate-resources.py` 现校验 1085 条哈希）。
- **`.slider-container` 滑条外观（2026-10-06 接入）**：源 `OTA` 的声明
  （`.slider-container{height:64px;opacity:.3;pointer-events:none}`、`.on{opacity:1;pointer-events:auto}`、
  `.no-tip{height:36px}`、`.slider{bottom:25px;height:6px;width:100%}`、
  `.slider::-webkit-slider-thumb{background:#44d62c;border-radius:8px;height:16px;width:16px}`、
  `.on …:hover{background:#5d5d5d;border:2px solid #44d62c}`、`.on …:active{background:#383838;border:2px solid #44d62c}`、
  `.left,.right{background:#44d62c;border-radius:3px;bottom:25px;height:6px}`、
  `.track{background:#44d62c4d;border-radius:3px;bottom:25px;height:6px}`、
  `.slider-tip{background-color:#44d62c;border-radius:3px;bottom:42px;line-height:14px;padding:4px 8px}` +
  `.slider-tip,.thumb-tag{color:#212121;font-size:12px;position:absolute}`）
  现在由共享的 `src/ui/source_slider.rs`（`SourceSlider`）绘制，颜色集中在 `theme::SliderColors`；
  指针拖动与焦点仍由 gpui-kit 的 base slider 承担，`featureDisabled` 保持行级 `opacity(0.3)`
  并禁用交互。填充宽度按源 `updateValue()` 的 `calc(8px + p*(100% - 16px))`（两端 8px 缩进），
  数值气泡按 `p*(W-16) - tipW/2 + 8` 居中于拇指中心；`STA` 传 `noTip`，所以它的容器是 36px 且
  不渲染气泡。1383 OLED 亮度行同一轮改用该共享层。收据见
  [source-slider-current-evidence.json](source-slider-current-evidence.json)，机检
  `node tools/audit-source-slider.cjs --check`（7 份样式表 × 12 条声明 + 本地指纹）。
- **仍未接入**：风扇转速行（`range()`）对应的源控件结构尚未核对，暂不按 `OTA` 改造；
  `.thumb-tag` 与 `.slider-tip` 的悬停互换只在源传 `thumbTag` 时挂载（当前无页面使用）；
  界面文字默认色由应用层决定，本地用主题前景色；显示器色彩页（`xAA`）仍用旧的行渲染，
  下一轮按同一 `STA` 结构替换；`uiRestraint` 真实填充与显示器产品图见上。

## 2026-10-04 输入源按钮与确认浮层（`SSA` + `OSA`）

- **按钮组**：`.btn_group{display:flex;flex-wrap:wrap;gap:10px}` +
  `.btn_group.inputSource{gap:20px}`；`.btn_group .btn_custom` 是
  `background-color:#0000;border:1px solid #5d5d5d;display:flex;align-items:center;
  font-family:Roboto;font-size:12px;margin:0;padding:7px 16px 6px;text-transform:uppercase;
  width:fit-content`，`.active{background-color:#222;border-color:#44d62c}`。本地按
  `INPUT_SOURCES`（AUTO 0、HDMI_1 17、DP_1 15、USB_C 19，与 `SSA` 的渲染顺序一致）渲染，
  文字照 `text-transform:uppercase` 转大写，`:active{opacity:.6}` 与共享 `.btn` 的
  `color:#fff;font-size:12px;line-height:14px` 一并沿用。
- **缺图标（不猜）**：每个按钮在原版里带 `<img>`（`.btn_group.inputSource .btn_custom img{
  height:20px;width:40px}`），资源是 `icon_hdmi.7df743db.svg`、
  `icon_displayport.c7f9208d.svg`、`icon_usb_typec.169c5217.svg` 与模块 6370 的 Auto 图标。
  已抓取的 `.ref/devices/3858/static/` 只有 `css/`、`js/` 两个目录，整个 `.ref` 树里也没有
  任何同名或同义文件（按文件名与 SVG 内容都搜过），因此本地按钮**只渲染文字**，不画占位
  图形；补齐需要先取回这四个文件。
- **确认浮层**：`.alert_wrap .profile-del{left:40px!important;top:165px!important}` 相对
  `position:relative` 的 `.widget` 定位；`.profile-del` 是
  `background:#111;border:1px solid #fd4949;border-radius:3px;min-width:300px;padding:20px;
  box-shadow:0 6px 10px 0 #0003;display:flex;flex-direction:column;align-items:center;
  position:absolute;opacity:0;visibility:hidden;transition:visibility 0s,opacity .3s linear`，
  `.show{opacity:1;visibility:visible}`；`.del-title`（此处 `title=""`）本地不渲染；
  `.body-text` 是 `color:#ccc;font-size:14px;line-height:17px`、`.profile-del .body-text
  {margin-bottom:10px}` 加 `t-center`；复选框容器 `.checkBoxAlignment{align-self:flex-start}`；
  确认键 `.profile-del div.thx-btn{background-color:#fd4949;border:1px solid #0000004d;
  color:#111;font-size:12px;height:27px;line-height:14px;min-width:90px;padding:4px 5px;
  white-space:nowrap;width:auto}` 另有 `.thx-btn:hover{opacity:.8}`、`:active{opacity:.6}`、
  `transition:opacity .3s`。颜色取自仓库既有的 `ProfileAlertColors::danger()`（`#fd4949`）。
- **显示过渡**：原版是 `.show` 切换 `opacity`（`.3s linear`）+ `visibility`（0s）。本地按
  同一 DOM 结构常驻渲染浮层，用 `motion::transition`（300ms、`Easing::Linear`）在
  `active` 之间插值透明度，非激活时 `invisible()`（等价 `visibility:hidden`，同时不接收
  事件），因此淡入淡出都有。
- **层级**：`SSA` 给组件加了 `extraClass:"widgetZIndex"`（`.widgetZIndex{z-index:6}`），
  让浮层盖住相邻组件。本地用 `deferred(...).with_priority(6)` 在后续绘制层绘制，同时避免
  被后面的组件裁剪。
- **交互分支**：`confirmInputSourceChange` 忽略「当前已选」的源；`shouldAskAgainValue`
  初始为真（先询问），复选框「不再询问」勾选后置假（直接切换）。本地由
  `input_source_step(current, requested, ask_again) -> {Ignore, Prompt, Apply}` 承载并有测试；
  点浮层外部（本地挂在页面根节点的 `on_mouse_down`，浮层自身 `stop_propagation`）等价于原版
  文档级 mousedown → `confirmAction(false)` 取消；确认键 → `confirmAction(true)` 应用；
  切换页面会丢弃未确认的请求（原版组件卸载即丢状态）。
- **仍未接入**：四个输入源图标；复选框的 `.check-box.checked:before/:after` 刻度动画
  （本地沿用组件自带的勾选动画）；`.btn`/`.thx-btn` 的 `transition:opacity .3s` 在本地是
  即时切换；`uiRestraint` 设备约束（HDR/Adaptive Sync/PIP 的禁用原因，属于设备上报状态）
  与显示器产品图。

## 2026-10-04 页面可达性与控件细节

- **渲染分支顺序**：`AccessorySystemProductWorkspace::render` 原先把
  `_ if !supports_page(...)` 描述符守卫放在实现分支之前，而 3858/3880 的
  Gaming/Color/Display 与 3893/3900/3907 的 Performance、3921 的 Customize 都没有生成
  描述符表（描述符只覆盖 Lighting），因此这些**已实现页面全部被守卫挡在一个乱码占位
  （旧提示文字已损坏，不作还原）之后**。现已改为实现分支优先、守卫只兜底没有描述符且没有渲染器的页，
  并删掉该乱码字符串（另一处同样的乱码在 `shell.rs` 的深链失败提示里，已换成明确中文）。
  机检 `python tools/audit-page-coverage.py` 会报「实现分支出现在描述符守卫之后」的错误，
  该检查自带 `--self-test` 的四个合成用例（其中「守卫在前」的用例必须被报出来），
  因此不需要改写源码就能证明它能失败。
- **监视器控件提示**：`.widget .help` 为 14px 圆角控件（`background:#4a4a4a`，悬停
  `#ffffff4d`，`transition:background-color .3s`，位置 `right:10px;top:10px`），图标是
  `tooltip_questionmark.96138d2f.svg`（11 份已抓取副本字节一致，本地复用同一份资源）。
  悬停显示 `.widget .tip`：`max-width:300px`、`font-size:14px;line-height:18px`、
  `padding:8px 10px`、黑底 `1px #5d5d5d` 边框、`#ccc` 文字、`opacity .3s linear`，
  锚点为组件右上 `right:14px;top:34px`。`SourceTooltip` 新增 `WidgetTip` 分支实现该几何
  （相对 14px 帮助控件即「右边缘内收 4px、上边缘下移 24px」），并有几何测试
  `widget_tip_anchors_below_the_help_control_and_caps_at_300px`。按用户的禁止运行约束，
  该测试只通过 `cargo check --locked --all-targets` 编译检查、未被执行，因此断言值本身
  仍未运行验证。
  文本键：`SCARLETT_INPUT_SOURCE_TOOLTIP`、`PIP_TOOLTIP`、`FREE_SYNC_TOOLTIP`、
  `FPS_COUNTER_TOOLTIP`、`HDR_TOOLTIP[_WINDOWS_11]`（十种语言全部存在）。
- **HDR 条件分支**：原版按 `osName===windows && osVersion==="11"` 选择
  `r6O.xbh` / `r6O.UEY`，对应两点只在措辞上不同的提示（“Brightness & Color” 与
  “Windows HD Color”）。新增只读的 `backend::system::is_windows_11()`（`CurrentBuildNumber`
  ≥ 22000）承载该判断，键选择由 `hdr_tooltip_key` 固定并有测试。
- **刷新率计数器位置**：`RefreshRateCounter_main/row/btn_custom` 是两行 27px、按钮
  `min-width:90px`、透明底 `1px dashed #ccc` 边框、选中显示 `#`；选中态 `btn-green`
  （`#44d62c` 底 + 黑字），悬停 `#fffafa1a`，按下 `#0000004d` 且边框转 `#44d62c`。
  本地 `corner_grid` 按 `e3` 枚举 1–4 渲染（测试 `monitor_corner_grid_uses_the_e3_enum_order`
  固定枚举顺序，同样只编译未运行），替换了原先的文字标签行
  （原文字标签还用了不存在的键 `BOTTOMRIGHT`，语言表里的实际键是 `BottomRight`）。
- **刷新率计数器的挂载（同一轮补正）**：`nSA` 把网格放进 `qAA` 屏幕模型
  （`main{display:flex;flex-direction:column;height:100%;padding:10px}`），中间还有
  `.RefreshRateCounter_middle`（`color:#999`、Roboto 14px、`flex-grow:2`、居中）显示
  `r6O.cui` → 键 `DISPLAY`；计数器关闭时整块 body 带 `.featureDisabled`
  （`opacity:.3;pointer-events:none`），本地以 `opacity(0.3)` + 交互门控实现。PIP 组件本身
  也挂了同一个 `.featureDisabled`，本地已按此加在 `.pip_display` 整行上。
- **仍未接入**：显示器输入源确认浮层已在下一节接入；`uiRestraint` 设备约束
  （HDR/Adaptive Sync/PIP 的禁用原因，属于设备上报状态）与显示器产品图未接入。
  以上都不影响本轮已经可达的页面本身。
