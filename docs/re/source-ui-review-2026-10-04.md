# 当前源逐项 UI 复核：设备顶栏、主导航、托盘

日期：2026-10-04。此报告由独立子任务静态复核；没有运行应用、构建、测试、安装器、下载的 JavaScript 或 DLL。只修改本报告。Rust 行号为本轮读取时的位置；主任务同步修改了若干已确认问题，以下明确区分已修正与待修正。

## 方法和边界

- JavaScript 只作为 Acorn AST 数据读取。应用包通过 `tools/webpack-source.cjs` 的 manifest、webpack 模块、模块顶层 binding 和 export getter 逐级定位；没有 `require` / `eval` 下载代码。
- 182 设备单包的业务代码是闭包，不冒充 webpack 模块。记录其闭包、类、方法和 UTF-16 字符偏移。
- CSS 按源文件顺序收集所有匹配规则，区分同一选择器的多次声明和具体选择器覆盖；CSS 存在不等于当前组件命中。
- 本轮实际打开的源仅为当前 `.ref/devices/182/`、Dashboard、systrayv2；未访问四个禁止目录，也未使用 `.ref/tools/`。
- 本轮不能宣称全部产品、托盘所有分支或最终屏幕像素已验收。剩余分支需要继续按实际挂载方及状态输入核对。

## 1. 设备顶栏的实际组件树

当前 182：`.ref/devices/182/static/js/main.db20a7c4.js`，业务闭包 `@4201771..5035560`，类 `tn @4442702..4450431`，`render @4447283..4450430`。

```text
tn.render
  div.nav-tabs
    div.profile-wrapper [ref: profileEl]
      renderProfileBar && renderProfileBar()
      hasDynamicMode && activeDynamicMode -> ui
      renderKeyMapBar -> nested profile-wrapper
    div.navs-wrapper [role: tabs]
      visibleNavs.map(li)
      hidden navs exist -> an [NavBarDropdown]
    div.right [ref: rightEl, role: tablist]
      displayMode == disabled -> jo
      hasTutorial -> lazy tutorial icon
      renderRestartWarning()
      renderStereoWarning()
      renderFilteringBar && renderFilteringBar()
      hasBattery && batteryState !== undefined -> div.battery
      extendedOptionComponent
      showHelp || navs contains HELP -> div.help > div.help-icon
  Mi
  hasTutorial && isShowTutorial -> lazy tutorial popup
```

### 已确认并由主任务修正

1. **重复前进/后退箭头。** 本轮初始 `src/features/workspace.rs:1157` 的 `device-tab-back` / `device-tab-forward` 不在上述 `navs-wrapper` 子树中。真实历史箭头在另一层 toolbar。不能用 `.nav.back` / `.nav.forward` 的样式声明证明它们应挂在此处。主任务已移除重复按钮，并接回 toolbar 内产品历史。
2. **电量与帮助分组错误。** 初始 `workspace.rs:1208` / `source_workspace.rs:700` 将电量独立放在居中标签之后、右侧 `flex_1` 帮助之前。这使电量贴近导航而非右端。主任务已将电量与帮助放入同一个右侧容器；当前分别在 `workspace.rs:1184` 和 `source_workspace.rs:723` 附近。

### 待继续修正：三列约束与溢出计算

182 `main.48c20423.css` 的有效规则：

| 偏移 | 选择器 | 声明 |
| --- | --- | --- |
| 20374 | `div.nav-tabs` | `min-height:48px; border-bottom:2px solid #000; width:100%` |
| 20611 | `.nav-tabs .profile-wrapper` | `display:flex; flex:1 0 25%` |
| 20714 | `.nav-tabs .right` | `flex:1 1 25%` |
| 20744 | `.nav-tabs .navs-wrapper` | `flex:1 0 max-content` |
| 22026 | `.nav-tabs .navs-wrapper` | `display:flex; font-family:Roboto,sans-serif; font-size:12px; justify-content:center` |
| 25015 | `.nav-tabs .right` | `align-items:center; display:flex; justify-content:flex-end` |

`workspace.rs:1145` / `source_workspace.rs:689` 附近左右列仍使用 `flex_1()`、中心使用自然宽度，未编码上述 basis/shrink 差异。应按实际三列的源规则实现，不能只因常见宽度下看起来居中就认定一致。

`tn.renderNavs @4443860..4444525` 使用：

```js
window.innerWidth
  - getElWidth(profileEl.current)
  - getElWidth(rightEl.current)
  - getElWidth(document.querySelector(".navs-wrapper .dots3"))
  - 10
```

`getElWidth` 是实际子节点 `clientWidth` 的和。`src/ui/surface.rs:80` 却将右区固定为 `46 + 24 + 10`；该常量不含变化的电量百分比文本，还把 icon 两边空白误注成各 10px。源 `.batt` 后续规则已覆盖为各 5px。`workspace.rs:1127` 还将物理 `viewport_size` / 已随 rem 缩放的文字测量，与未缩放的源像素常量相减，非 16px rem 时会进一步改变折叠阈值。

`source_workspace.rs:691` 主导航仍直接循环全部 primary pages，没有 `visibleNavs` / `NavBarDropdown` 的宽度计算与菜单入口。为共享页补齐时必须继续确认每个产品实际使用的 header/root props；不要直接把 182 的 profile 显隐条件复制到其他产品。

## 2. 电量内部、状态与提示

### 已确认匹配的内部尺寸

182 CSS：`@23278 .nav-tabs .batt,.nav-tabs .warn,.nav-tabs .warnRestart` 为 `26×26`、`background-size:20px`、`margin:0 10px`；随后 `@23849 .nav-tabs .batt` 覆盖为 `margin:0 5px`。`@24319 .right .battery` 为 `height:46px; font-size:14px; color:#ccc; display:flex; align-items:center; justify-content:center`。`@27094 .battery .low-batt` 为 `#c8323c`。

`src/ui/battery.rs:182`、`:207` 的 46px、14px、26px 外盒、20px 图像、两侧 5px 匹配该级联结果；百分比在前、图像在后的顺序也与 `tn.render` 一致。电量位置问题主要来自上一节的父层结构，不能通过改这些已正确的内部数字补偿。

### 待修正：实际 tooltip 被误认成旧属性 tooltip

`tn.render @4447283..4450430` 的图标节点只有 `className:this.getBatteryState()`，**没有 `tooltip` 属性**。真实第三个子节点是：

```js
!hideBattValue && jsx($o.A, {
  position: "bottom-left",
  isMounted: showBatteryTooltip && tip !== undefined,
  target: "battery-level-tips",
  children: tip
})
```

同一组件的当前 Dashboard 版本为 `App.72827d47.chunk.js` 模块 `97203`，类 `de @230498..238131`；其 `ie=a(58837)` 位于 `@226345`。模块 `58837` 的导出 `A -> m -> h` 定位到实际 tooltip 类 `h @80470..83764`，采用固定目标矩形、portal 和 viewport 水平边缘调整。

182 实际匹配的 CSS：

| 偏移 | 选择器 | 有效行为 |
| --- | --- | --- |
| 32411 | `.tooltip-razer.bottom-left>.main` 所在组 | `top:100%; margin-top:5px; bottom:auto; margin-bottom:0` |
| 32917 | `.tooltip-razer.bottom-left>.main` 所在组 | `right:0; left:auto; margin-left:0; justify-content:flex-end` |
| 33075 | `.tooltip-razer>.main` | `width:300px; opacity:0; transition:opacity .1s linear` |
| 33280 | `.tooltip-razer.show>.main` | `opacity:1` |
| 33316 | `.tooltip-razer>.main>.wrapper` | `padding:8px 10px; font:14px/16px Roboto; background:#000; border:1px solid #5d5d5d; color:#ccc` |

`battery.rs:187..202` 目前使用通用 `component::Tooltip`；warning 又强设 352px。352px 来源是 `@24104 .nav-tabs .batt.batt-warning[tooltip]:before`，当前图标根本不命中它。应使用实际 `tooltip-razer` 路径，保留其 300px 对齐容器与按内容宽度的 wrapper，而非把 wrapper 简化为固定 300px 或沿用未命中的 352px。

### 待修正：条件与数据表达

- `battery.rs:155` 只检查 `power_status`，未检查源的 `hasBattery`。已有 `Device.has_battery`，应作为渲染必要条件；单纯有缓存电量不等于应显示电池。
- 当前 `tn` 还明确处理 `hideBattValue`、`externalPowerConnected`、`earbudBatteries` / `earbudBatteryState`。Rust 此控件和 `model.rs:188` 的 `PowerStatus` 没有这些呈现输入。不能宣称耳塞左右电量、外接电源隐藏数值分支已完成；后续逐产品确认 root/Redux 的真实来源后接入，后端接口仍按主任务统一安排。
- `tn` 的数值小于 0 时显示 `-`、发出 `CHECK_BATTERY_STATUS`，tooltip 和低电红字也按该条件分支。`PowerStatus.level:u8` 无法表达负电量；当前 Rust 的 `level < 0` 呈现分支实际上不可到达。
- 当前 182 状态机函数 `@4249328..4249790` 的 default 仅将**图标**设为 `rE(100)`；tip key 仍是 `BATTERY_PERCENT`，`createBatteryTip` 使用真实 `batteryValue` 插入 level。`battery.rs:102` 的 `percent_tip(100)` 因此不对：未知 chargingStatus、level=37 时应图标 100，但提示 37%，不能提示 100%。其他已核对的 off / Charging>=99 / NoCharge_BatteryFull / PAUSED_CHARGING 图标选择与此当前函数一致。

## 3. 主导航：已定位并修正的翻译键

当前 Dashboard `App.72827d47.chunk.js` 模块 `35378` 的 `class x @15591..19231` 是 HomePage；构造器 `@17294..17369` 建立四个实际 nav，`@18578` 将 `this.state.nav` 传给 `C.A`。该模块内 `C=a(97203)`，后者 export `A -> ce`，`ce @238140..239395` 用 Redux connect 挂载前述 `de`，因此是实挂载链，不是单独查到常量。

| nav 顺序 | 模块局部引用 | 导入模块 54693 的实际字面量位置 | 当前中文原文 |
| --- | --- | --- | --- |
| 1 | `u.Rav` | `r @138840..138858 = DASHBOARD_HEADER` | 控制板 |
| 2 | `u.BSg` | `c @138950..138969 = GAMER_ROOM_HEADER` | GAMER ROOM |
| 3 | `u.iwS` | `s @138892..138920 = DEVICES_AND_MODULES_HEADER` | 设备和模块 |
| 4 | `u.fUK` | `a @138923..138947 = GLOBAL_SHORTCUT_HEADER` | 通用快捷键 |

初始 `src/nav.rs:64..67` 用 `DASHBOARD` / `GAMER_ROOM` / `DEVICES_AND_MODULES` / `GLOBAL_SHORTCUTS`，en、zh-CN、ja、de 本地语言包均不存在这些 key，因此一律回退到自写中文。主任务已改为上表当前 key。四个页面的顺序本来正确，无需增删主导航项。

`src/shell.rs` 的 main-navigation 仍是直接居中四个按钮，未走共享 header 的宽度与 overflow 行为；应连同设备页的源布局逐项补齐。其 48px 行高、20px 标签间距与当前 `55.4e8559cb.chunk.css` 对应规则一致。

## 4. 托盘：当前单启动器分支与遗漏

当前 systrayv2 `554.2573b048.chunk.js` 模块 **2554**，不是由 chunk 文件名推定的模块 554。实挂载链：

- `Re @36612..37707` 是 `.systray` 根，读取 `!!user.item.id`、`app.launchers.length`、`view.current`。
- `ae @27838..27959` 是 connected header：`user.id ? oe : se`；`oe @27960..28592` 为头像/用户名/查看在线，`se @28592..28777` 为 `.header-2` 登录入口。
- 有 user id 才挂 `he @31009..31839` navbar 与 `Oe @37707..39239` body；没有 user id 就不挂这两层。
- launchers.length>0 才挂 `me @31864..32837`；`me` 从 store 过滤 launcher，单项显示 title，多项以图标和 tooltip 表达。

### 已确认匹配的单项分支

`src/shell/tray.rs:129..197` 当前固定使用未登录 `header-2 + apps-1`。其配色 `#222/#111/#000/#ccc/#999/#eee`、Roboto 16px、line-height 1.22、60px header、60px apps、32px icon、icon 和文字间 10px、active 图标 opacity .3 与源匹配。apps 顶边 1px 包含在 60px 总高度内，源 li 高 59px；不能再额外减掉一行边界。

### 待修正：明确缺少的过渡

`554.7cdbd936.chunk.css`：

| 偏移 | 实际选择器 | 过渡 |
| --- | --- | --- |
| 2959 | `.systray>.header-2` | background-color 与 color，各 `.1s ease-in-out` |
| 6215 | `.systray>.apps>li` | background-color `.1s ease-in-out` |
| 6474 | `.systray>.apps>li>.icon` | opacity `.1s linear` |
| 6729 | `.systray>.apps>li>.title` | color `.1s ease-in-out` |

`tray.rs:160`、`:183` 仅立即 `.hover` 换色，`:192` 立即 `.group_active` 改透明度，没有上述 motion/transition。应按四个实际属性分别补齐 100ms 过渡，含移出/松开时的反向过渡。

### 待完成的实际分支，不应标记全部完成

当前 `TrayPopup` 没有 session、view、launchers 数据输入；因此无法进入 `oe` 登录/访客 header、`he` 通知/小组件导航、`Oe` 通知/小组件/空状态，也不能显示多应用 launcher。`DesktopTray::menu_entries()` 同样固定单 Synapse 和登录入口。应在主任务的后端统一接入之前先把对应 view/model 分支和 UI 做全，使用真实条件的本地可审查状态，不能伪造已登录或伪造通知。

`me` 的实际 title 子节点直接使用 `getAppTitle` 的解析结果，并无 `.toUpperCase()`；本地 `tray.rs:194` 强制大写。当前 CSS 的 launcher/title 规则没有 text-transform。此点应由应用标题原文决定，不应额外改变大小写；单 Synapse 当前 host title 可能本就全大写，不代表多应用均可大写。

## 5. 旧校验的修复要求

- `tools/audit-device-tabs.cjs` 初始版本要求 Rust 出现 `device-tab-back` / `device-tab-forward`，这与已证明的当前组件树相反。必须改成验证实际 toolbar/header 挂载关系。
- 该工具 `parseRules` 遇到同名选择器只保留第一次声明；当前 `.nav-tabs .navs-wrapper`、`.nav-tabs .right` 都分多处声明，不能用此实现声称完成 CSS 级联审计。
- `tools/audit-battery-indicator.cjs` 的图标/色值检查只能证明资产表和数字存在，不能证明父节点定位、hasBattery、hideBattValue 或真实 tooltip 渲染路径正确。
- 校验产物应记录当前文件 hash、模块/类、属性/JSX 节点偏移、条件输入与 Rust 实际消费者，不得继续只靠全文件 `includes` 来认定该控件已完成。

## 来源哈希

| 路径 | SHA-256 |
| --- | --- |
| `.ref/devices/182/static/js/main.db20a7c4.js` | `7626cc9c0a20cf491a481c701829429ac8f23c9c8c5b907736505f70d3b9bcc7` |
| `.ref/devices/182/static/css/main.48c20423.css` | `dec1b71e91cc5e261b9c84bfcdfe501d5c6aa7f16491e103561a5fd69e916a1e` |
| `.ref/applications/synapse/dashboard/static/js/App.72827d47.chunk.js` | `c962f6962441b1b053a2789be110123e9a90279f205129960b531fbd3cbda40a` |
| `.ref/applications/synapse/dashboard/static/js/main.01550b17.js` | `b3a39adaa6507541fbdb3b16e5a9cd716b30289113859e057ef24a3925322917` |
| `.ref/applications/synapse/dashboard/static/css/55.4e8559cb.chunk.css` | `aba3cdc0ae2b3f36197ee327cc9a681a5680cf9117b6ee1bf59a867ebb57daf8` |
| `.ref/applications/systray/systrayv2/static/js/554.2573b048.chunk.js` | `e492fee599908243b58597175bb101a8e44c2357fee6fbec341efac7653a439f` |
| `.ref/applications/systray/systrayv2/static/css/554.7cdbd936.chunk.css` | `af7e89ac1df1c2e5e0f423fe6695f2845f9a43705ce9747c3740e92a30e951c3` |
| `.ref/applications/systray/systrayv2/static/css/main.1665f0a2.css` | `80da6f78c2b4a93ca3d0db69f3ee0dbbdcbfd9e05490b7e4fec2f48040944e71` |
