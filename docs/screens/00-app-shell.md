# 应用外壳与整体布局（从主前端实测）

> 本文补齐此前缺失的**整体骨架**。各设备页文档（`01-*.md` … `09-*.md`）只覆盖
> 「设备页内部」，没有覆盖「应用外壳 / Dashboard / 应用设置」这一层。
>
> 全部数值逐条引自雷云自己的 CSS，**没有推断**。证据文件见文末。

---

## 0. 一句话结论

雷云 Synapse 4 是**两级页面模型**：

```
应用外壳（主前端 apps.razer.com/synapse/dashboard/）
├─ 顶栏 .nav-tabs      [配置文件 25%] [功能标签页 居中] [电池/帮助 25%]
└─ #body-wrapper
   ├─ Dashboard        .dashboard —— 可折叠、可拖拽的 .box-group 分段
   │                   每段里是 290×220 的 DeviceCard
   └─ 应用设置          .main-setting —— 固定左侧 .side-navigation（宽 180px）

设备页（每个设备独立窗口，openNewTab 打开）
  products/{productId}/ui/index.html
  ├─ 顶栏 .nav-tabs     同样的三区结构，但中间是该设备的标签页
  └─ #body-wrapper
     └─ .body-widgets   600px 卡片两列换行 + 250px 产品图区
```

**设备页没有左侧栏。** 设备是在 Dashboard 上选的，点开后在**新窗口/新标签页**里
加载该设备自己的 UI。左侧栏只出现在**应用设置页**。

---

## 1. 应用外壳

### 1.0 顶栏就是最高的一行——**没有独立的标题栏**

在主前端全部 CSS 中检索 `title-bar` / `titlebar` / `drag-region` /
`window-controls` / `app-header` / `rz-title` / `top-bar`，**全部 0 命中**。

窗口边框由 Electron 提供；网页内容的第一行就是 `.nav-tabs`：

```css
div.nav-tabs {
  align-items:center; background-color:#222; border-bottom:2px solid #000;
  color:#5d5d5d; min-height:48px; position:relative; width:100%; z-index:106;
}
```

因此本实现的顶栏**只有一条**：窗口拖动区与 `.nav-tabs` 合并，
由 `TitleBar` 只承担拖动，里面直接就是三区内容。

> ⚠️ 曾经做成**两条**（`TitleBar` 一条 + `.nav-tabs` 一条），是错的，已修正。

### 1.1 DOM 结构（引自 `App.eb32d7cd.chunk.js`）

```jsx
<div className="main-container">
  <div className={"nav-tabs" + (keymapbarEnabled ? " keymapbar-enabled" : "")
                            + (showLinkedGames ? " disabled" : "")}
       style={extraStyle}>
    <div className="profile-wrapper" ref={profileEl}>…</div>
    <div className="navs-wrapper" role="tabs">…</div>
    <div className="right" ref={rightEl} role="tablist">…</div>
  </div>
  <div id="body-wrapper" className={"body-wrapper" + (status ? "" : " scrollable")}
       ref={bodyRef} style={extraStyle}>…</div>
</div>
```

### 1.2 外壳 CSS

| 选择器 | 布局 | 样式 |
|---|---|---|
| `body, html` | `height:100%; margin:0; max-width:1920px; min-height:720px; overflow:hidden; width:100%` | `background-color:#222; color:#ccc; font-family:Roboto,sans-serif; font-size:16px; user-select:none` |
| `.main-container` | `display:flex; flex-direction:column; height:100%; min-width:600px; position:absolute; width:100%` | `background-color:#222` |
| `div.nav-tabs` | `align-items:center; min-height:48px; position:relative; width:100%; z-index:105` | `background-color:#222; border-bottom:2px solid #000; color:#5d5d5d` |
| `.nav-tabs` | `display:flex` | |
| `.nav-tabs .profile-wrapper` | `display:flex; flex:1 0 25%` | |
| `.nav-tabs .navs-wrapper` | `display:flex; flex:1 0 max-content; justify-content:center; font-family:Roboto,sans-serif; font-size:12px` | |
| `.nav-tabs .right` | `flex:1 1 25%` | |
| `.body-wrapper` | `flex:1 1; height:100%; min-width:600px; padding:10px 20px 20px; width:100%` | |
| `.main-container > #body-wrapper > .body-widgets` | `min-width:0; min-width:auto` | |
| `body .main-container .body-wrapper` | `min-height:0; min-height:auto` | |

> 注意 `.body-wrapper` 会按状态加类：可滚动时加 `.scrollable`，
> 宏显示模式加 `.customMacro`，armory 模式加 `.custom-scrollable`。

### 1.3 顶栏三个区

顶栏是 **3 个区**，不是「一个标签条」：

| 区 | 类名 | 占比 | 内容 |
|---|---|---|---|
| 左 | `.profile-wrapper` | `flex:1 0 25%` | 配置文件下拉（`renderProfileBar`）、动态模式提示 `.profile-tips`、keymap bar |
| 中 | `.navs-wrapper` | `flex:1 0 max-content`，`justify-content:center` | **功能标签页**（`.nav`）+ 「⋯」溢出菜单 `.dots3` |
| 右 | `.right` | `flex:1 1 25%` | 重启/立体声警告、过滤栏、**电池指示器**、扩展项、帮助图标 `.help` |

### 1.4 标签页胶囊 `.nav` 的完整状态

```css
.nav-tabs .nav {
  border-radius: 14px;          /* 胶囊 */
  padding: 7px 10px;
  margin-right: 20px;
  color: #999;
  font-size: 12px;              /* 来自 .navs-wrapper */
  line-height: 14px;
  text-align: center;
  text-transform: uppercase;
  white-space: nowrap;
  transition: background-color .3s, color .1s;
}
.nav-tabs .nav:last-child { margin-right: 0 }
.nav-tabs .nav:hover  { background-color:#2d2d2d; color:#ccc }
.nav-tabs .nav:active { background-color:#3cbf27; color:#111 }
.nav.active,
.nav-tabs .nav.active:hover { background-color:#44d62c; color:#111 }
.nav.disabled { opacity:.3; pointer-events:none }
.nav-tabs .user.disabled:hover, .nav.disabled:hover { background-color:#0000; cursor:default }
```

> `.nav` 用 `role="tab"` + `aria-selected`，溢出项进 `.dots3` 下拉：
> ```css
> .nav-tabs .navs-wrapper .dots3 { border:none; border-radius:13px }
> .nav-tabs .navs-wrapper .dots3:hover { background-color:#2d2d2d }
> .nav-tabs .navs-wrapper .dots3.has-actived-option { background-color:#44d62c }
> .nav-tabs .navs-wrapper .dots3 .act { color:#ccc; font-size:14px }
> .nav-tabs .navs-wrapper .dots3 .act:hover { background-color:#1a1a1a }
> .nav-tabs .navs-wrapper .dots3 .act.action.active { background-color:#000; color:#44d62c }
> .nav-tabs .navs-wrapper .dots3 .act.action.active:hover { background-color:#1a1a1a; color:#44d62c }
> ```

### 1.4.1 `.nav` 规格与本实现的令牌映射（已逐条核对）

| 属性 | 真实值 | 本实现 | 状态 |
|---|---|---|---|
| `border-radius` | `14px` | `rounded(px(14.))` | ✅ |
| `padding` | `7px 10px` | `px(px(10.)).py(px(7.))` | ✅ |
| `margin-right` | `20px`，末项 `0` | `mr(px(20.))` + `:last-child` 判断 | ✅ |
| `font-size` | `12px` | `.text_size(px(12.))`（`.navs-wrapper` 上） | ✅ |
| `line-height` | `14px` | `.line_height(px(14.))` | ✅ |
| `text-transform` | `uppercase` | 对标签文本 `.to_uppercase()`（GPUI 无该属性） | ✅ 等价 |
| `color` | `#999` | `theme.muted_foreground` = `#999` | ✅ |
| `:hover` | `#2d2d2d` / `#ccc` | `theme.secondary_hover` / `theme.foreground` | ✅ |
| `.active` | `#44d62c` / `#111` | `theme.primary` / `theme.primary_foreground` | ✅ |
| `:active` | `#3cbf27` / `#111` | `theme.button_primary_hover` / `theme.primary_foreground` | ✅ |
| 栏底色 | `#222` | `theme.background` = `#222` | ✅ |
| 栏下边框 | `2px solid #000` | `border_b(px(2.))` + `theme.group_box`（`#111`） | ⚠️ 近似 |
| `min-height` | `48px` | `.min_h(px(48.))` | ✅ |

> **为什么不用 `TabBar::pill()`**：`TabVariant::Pill` 的字色**写死**在库里
> （`gpui-component/src/tab/tab.rs` 第 148-152、192-196 行）——常态取
> `theme.foreground`、hover 取 `theme.tokens.secondary`，绑死在**全局**令牌上；
> 而圆角/内距/字号在主题里**没有对应令牌**（`schema.rs` 只暴露
> `tab_bar.background` 与 `tab_bar.segmented.background`）。
> 改令牌会连带改掉整个应用，所以按《Design Guides · Layout patterns》
> 「Wrap it in an application component when it carries domain language or
> policy」把 `.nav` 规格封成应用自己的 `tab_strip`，颜色一律走令牌。
> 另：`TabBar` 的中间区是 `flex_1` **左对齐**，而 `.navs-wrapper` 是
> `justify-content:center` **居中**，用 `prefix`/`suffix` 也复现不出居中。
>
> **`#3cbf27` 的来源**：它在全部 CSS 里出现 14 次，14 次都是同一个角色
> 「主色悬停/按下」——`.nav-tabs .nav:active`、`.header-offline>.box:hover`、
> `.header-unsaved.active>.box`、`.header-avatar>.box:hover`、
> `.header-main>.header-links>.item:hover`、
> `.macro-keypad-module .function-choice-section>div .item.active`。
> 因此它是**产品级显式值**，不是推出来的。此前用 `.thx-btn:active{opacity:.6}`
> 混色得到的 `#368e28` 是错的（那只适用于 `.thx-btn`，现已改回专用于按钮按下态）。

### 1.5 顶栏电池指示器

```jsx
<div role="img" id="battery-level-tips" aria-roledescription="battery status"
     className={"battery " + (hideBattValue ? "hideBattValue" : "")}>
  {!hideBattValue && !externalPowerConnected &&
    <span className={battery 0..10 ? "low-batt" : ""}>{batteryValue >= 0 ? `${batteryValue} %` : "-"}</span>}
  <div className={getBatteryState()} />
  <Tooltip position="bottom-left" target="battery-level-tips">{batteryTips}</Tooltip>
</div>
```

电量**显示在顶栏右侧**，不在电源页里；`0–10%` 加 `.low-batt`。

---

## 2. Dashboard

### 2.1 容器

| 选择器 | 布局 |
|---|---|
| `.dashboard` | `display:flex; flex-direction:column; margin:0 auto; max-width:1220px; min-width:620px; position:relative; transition:height .2s linear` |
| `.dashboard.reflow` | `max-width:2460px` |
| `body .main-container .body-wrapper .dashboard` | `margin-bottom:50px` |

### 2.2 分段 `.box-group`：可折叠 + 可拖拽

```css
.dashboard .box-group { margin:10px 0; max-width:100%; min-height:18px; width:100%; transition:all .1s ease-in-out }
.dashboard .box-group.loading { visibility:hidden }
.dashboard .box-group:hover { z-index:20 }

/* 标题行：折叠箭头 + 标题 + 拖拽把手 */
.dashboard .box-group .title {
  align-items:stretch; display:flex; font-size:14px;
  position:relative; width:100%; color:#ccc;
}
.dashboard .box-group .title .collapse { align-items:center; display:flex; flex:1 1; z-index:2 }
.dashboard .box-group .title .collapse:hover { color:#fff }
.dashboard .box-group .title .collapse .icon {
  height:10px; width:10px; margin-right:10px;
  transform:rotate(-90deg);                 /* 折叠态 */
  transition:transform .3s linear;
  background-image:url(icon_expand.svg); background-size:10px; background-position:50%;
}
.dashboard .box-group.expand .title .icon { transform:rotate(0deg) }   /* 展开态回正 */
.dashboard .box-group .title .drag-div {
  align-items:center; cursor:grab; display:flex; flex:1 1 auto;
  height:17px; justify-content:center; top:-3px; z-index:6;
}
.dashboard .box-group .title .drag-icon { position:absolute; inset:0; justify-content:center; z-index:3 }
.dashboard .box-group .title .drag-icon:before {
  content:""; cursor:grab; display:block; height:19px; width:22px; transform:rotate(90deg);
  background-image:url(icon_draggable_large.svg); opacity:0; visibility:hidden;
}
/* hover 标题 或 .show-drag-icon 时才淡入把手 */
.dashboard .box-group .title:hover .drag-icon:before,
.dashboard .box-group.show-drag-icon .drag-icon:before { animation:delayedShow .5s linear 0s forwards }

/* 内容：max-height 0 → 展开 */
.dashboard .box-group .content { margin-top:10px; max-height:0; transition:max-height .3s ease-in }
.dashboard .box-group .content.show-overflow { transition:max-height .15s ease-in-out }
.dashboard .box-group .content .content-inner {
  display:flex; flex-wrap:wrap; gap:20px;
  position:relative; transform:translateY(-100%); transition:transform .3s linear;
}

/* 背板：展开时形成整段的卡片底 */
.dashboard .box-group .backdrop-box {
  position:absolute; left:0; right:0; top:0; bottom:10px; max-height:40px;
  background-color:#333; border-radius:5px; opacity:0; visibility:hidden; z-index:0;
  transition:all .3s ease-in-out .3s;
}
.dashboard .box-group.expand .backdrop-box { bottom:0; max-height:2000px }
.dashboard .box-group.dragging .backdrop-box {
  background-color:#3333334d; border:2px solid #44d62c; border-radius:5px; opacity:1; z-index:5;
}
.dashboard .box-group #devices { position:relative; z-index:1 }
```

**要点**：分段折叠时只有 40px 高的背板；展开后背板撑到 `max-height:2000px`
形成整段底色 `#333` + 圆角 5px。拖拽时背板变 `2px solid #44d62c`。

### 2.3 设备卡片 `DeviceCard`

```css
.DeviceCard_deviceCard {
  background:#0000004d; border-radius:5px;
  display:flex; flex-direction:column;
  width:290px; min-height:220px; padding:10px;
  position:relative; transition:border-color .3s;
}
.DeviceCard_deviceImageContainer { align-items:center; display:flex; flex:1 1; justify-content:center;
                                   max-height:140px; min-height:140px; position:relative }
.DeviceCard_deviceInfo { align-items:center; display:flex; flex-direction:column;
                         font-size:14px; gap:8px; justify-content:flex-start;
                         min-height:50px; text-align:center; text-transform:uppercase }
```

即 **290×220**：图片区固定 140px，信息区最小 50px、居中、大写。

卡片还有这些状态类（`4130.css`，共 49 条规则）：
`_clickable` `_nonClickable` `_disabled` `_dimmedContent` `_connected` `_failed`
`_pairBadge` `_pairedBadge` `_pairingBadge` `_unpairingBadge` `_spinnerSmall`
`_categoryIcon` `_mouse` `_keyboard` `_deviceImage` `_devicePlaceholder` `_boxImgContainer`

> 卡片是**可翻转**的（`box-flip-front` / `box-flip-inner` / `box-flip-back`），
> 背面含 `.header` / `.body` / `.description` / `.device-list`（`ul > li`）/ `.footer-link`。
> 键盘可达性由代码显式实现：卡片内 `[data-device-card-action]` 之间用
> `Tab` / `Shift+Tab` 循环，`Space` / `Enter` 触发。

---

## 3. 应用设置页

设置是**应用级页面**，不是设备标签页。三台已下载设备声明的标签页里
**都没有 `TAB_SETTING`**（见 `docs/screens/README.md` 的按设备标签页表）。

```css
.main-setting { height:100%; width:100% }

.main-setting .side-navigation {
  position:fixed; margin-left:70px; width:180px;
  font-size:14px; line-height:17px; text-align:left;
  color:#ccc;
}
.main-setting .side-navigation ul { list-style:none; padding:0 }
.main-setting .side-navigation ul li {
  display:flex; flex-direction:column; justify-content:center;
  height:30px; padding-left:10px; margin-bottom:2px;
  border-radius:5px;
}
.main-setting .side-navigation ul li:last-child { margin:0 }
.main-setting .side-navigation ul li:hover  { background:#ffffff1a none; color:#44d62c }
.main-setting .side-navigation ul li:active { background-color:#000; color:#44d62c }
.main-setting .side-navigation .active          { color:#44d62c }
.main-setting .side-navigation .active:hover    { background:#ffffff1a none; color:#ccc }
.main-setting .side-navigation .active:active   { background-color:#000!important; color:#ccc }

/* 设置页的 .body-widgets 是块级，不是 flex 网格 */
.main-setting div .body-widgets { display:block; margin:0 }
.main-setting .widget .title { font-size:18px }
.main-setting .text-bold { font-size:14px; font-weight:500; line-height:17px; text-align:left; text-transform:uppercase; color:#ccc }
```

**要点**：左侧导航 `position:fixed`、`margin-left:70px`、宽 `180px`；
条目高 `30px`、圆角 `5px`、hover 底色 `#ffffff1a` + 绿字，激活字色 `#44d62c`。
设置页里的 `.widget` 标题放大到 `18px`。

---

## 4. 设备页（本仓库各 `0X-*.md` 覆盖的对象）

设备页由主前端 `openNewTab` 打开：

```js
this.addDeviceUITab = function (device, tabName, newTab) {
  const { productId, deviceContainerId } = device;
  const query = stringify({ containerId: deviceContainerId });
  const url = `${HOST_URL}/products/${productId}/ui/index.html?${query}`;
  newTab ? openNewTab(url, tabName).then(() => setTimeout(() => focusTab(tabName), 200))
         : openNewTab(url, tabName);
};
this.getDeviceTabName = ({ vendorId = 5426, productId, deviceContainerId }) => ({
  ui_tab_name: `usb_${vendorId}_${productId}_${deviceContainerId}_ui`,
});
```

设备页自身仍是 `.main-container` + `.nav-tabs`（三区，中间换成该设备的标签页）
+ `.body-wrapper`，其中：

| 选择器 | 布局 | 样式 |
|---|---|---|
| `.body-widgets` | `flex-direction:row; flex-wrap:wrap; justify-content:center; margin:auto; max-width:1240px` | |
| `.body-widgets .widget` | `flex:0 0 auto; margin:10px auto; max-width:600px; min-width:600px; padding:30px 40px; font-size:14px` | `background-color:#111; border-radius:5px` |
| `.widget-col` | `flex-direction:column; height:fit-content; width:600px` | |
| `.widget-prod` | `height:250px; margin:10px auto; max-width:1220px; min-width:1024px; width:100%` | |
| `.widget-prod img` | `left:50%; position:absolute; top:50%` | |

---

## 5. 本项目实现进度（对照本文件所述真实布局）

### 5.1 已修正

| # | 原偏差 | 现在的做法 | 位置 |
|---|---|---|---|
| 1 | **左侧设备侧栏** | **已删除**。外壳改为 `.main-container` 纵向：窗口标题栏 + `.nav-tabs` + `#body-wrapper`，与真实结构一致 | `src/app.rs` 的 `Render for AppShell` |
| 2 | 标签页单独一条 | 顶栏**只有一条**：窗口拖动区与 `.nav-tabs` 合并（雷云前端没有独立标题栏，见 §1.0）；三区按 `flex:1 0 25%` / `flex:1 0 max-content; justify-content:center` / `flex:1 1 25%` 排 | `AppShell::top_bar` |
| 2b | 标签胶囊样式与真实值不符 | `tab_strip` 逐条对齐 `.nav` 规格：圆角 14px、内距 7×10、字号 12px、行高 14px、右距 20px、`#999` / hover `#2d2d2d`+`#ccc` / active `#44d62c`+`#111` / `:active` `#3cbf27`（对照表见 §1.4.1） | `AppShell::tab_strip` |
| 2c | 主题里 `primary_foreground` 与主色悬停色是推出来的 | 按实测改为 `#111`（`.nav.active` 前景）与 `#3cbf27`（14 条规则共用的产品级「主色悬停/按下」值） | `src/main.rs` 主题 |
| 3 | 顶栏没有左右区 | 新增 `profile_region`（左：设备名 + 配置文件）与 `right_region`（右） | `AppShell::profile_region` / `right_region` |
| 4 | 电池只在电源页 | 电量移到**顶栏右区**；`≤10%` 用 `theme.danger` 强调（对应原版 `.low-batt`） | `AppShell::right_region` |
| 6 | 设备卡片尺寸自定 | 改为真实 `DeviceCard` 几何：**290×220**，图片区固定 140px，信息区 ≥50px、`gap:8px`、居中大写 14px；并**可点击选设备** | `src/pages/dashboard.rs::device_card` |
| 7 | `Tab::Setting` 当设备标签页 | `Tab::Home` / `Tab::Setting` 改为**应用级**标签页，恒定在顶栏最前；设备标签页接在其后 | `AppShell::top_bar` |
| 8 | `Tab::Home` 当设备标签页 | 同上 | 同上 |
| 5 | Dashboard 是设备卡片网格 | 改为 `.dashboard` 容器 + **可折叠分段 `.box-group`**：`.backdrop-box` 背板（`#333` + 圆角 5px，铺在内容之下）、`.title`（折叠箭头 10×10 + 标题）、`.content-inner`（`flex-wrap` + `gap:20px`） | `src/pages/dashboard.rs::box_group`、`AppShell::home_sections` |

> - 顶栏溢出项交给 `TabBar::menu(true)`，对应原版 `.dots3` 下拉。
> - `.body-wrapper` 的 `padding:10px 20px 20px` 现由滚动容器承担，不再重复加在
>   `.body-widgets` 上（此前是错的归属）。
> - `TabBar` 的 `prefix` / `suffix` 位置已核对库源码 `src/tab/tab_bar.rs` 第 519 / 586 行。
> - 窗口标题栏精简为**只放应用名**——依据 《Design Guides · Layout patterns》
>   「The title bar is window chrome first」；设备名/配置/保存状态都在顶栏三区里，不重复。

### 5.2 仍待修正

| # | 现状 | 真实布局 | 影响 |
|---|---|---|---|
| — | 顶栏右区只有电量、保存提示、帮助 | 原版还有重启/立体声警告、过滤栏 | 缺两项；本实现没有安装器与立体声音频路由，因此不做假的警告 |
| — | 栏下边框用 `theme.group_box`（`#111`）近似 | `2px solid #000` | 主题里没有「顶部栏下边框」这个语义档位，差 1 档 |
| — | 标签页**不溢出**，没有「⋯」下拉 | `.navs-wrapper .dots3`，溢出项进下拉（`border-radius:13px`、hover `#2d2d2d`、有激活项时底 `#44d62c`） | 设备标签页最多 8 项，当前窗宽下不溢出；未实现溢出折叠 |
| — | 分段折叠/重排**无动画**，重排用上/下移按钮 | 原版是 `max-height` + `transition .3s` 动画，重排是**拖拽**（`.drag-div` / `.drag-icon`） | 交互形态不同；已保留 `cursor_grab` 把手作为视觉提示 |
| — | 提示气泡走库默认（**有圆角**） | `.tip` 是**直角**、`#000` 底、`padding:8px 10px`、`max-width:300px` | 见 §6.4 |
| — | 开关、下拉的 `disabled` 态未接 | `.switch.disabled { opacity:.3 }`、`.s3-dropdown.disabled { opacity:.3 }` | 视觉上分不出禁用 |
| — | `stepper_row`（「◀ 值 ▶」）仍是自造控件，**41 处** | 真实界面用 `.s3-dropdown` 直选或滑块 | 枚举型已全部换成真下拉（本轮，见 §6.7）；剩下的是**连续型**数值，应换滑块 |

> **为什么重排不做成拖拽**：拖拽本身没有键盘路径，而
> 《Design Guides · Accessibility checklist》要求每个操作都有键盘可达的替代。
> 上/下移按钮同时满足两者，因此本实现以按钮承担同一操作，并保留拖拽把手的外观。
>
> **折叠箭头**：原版是同一个图标在 `rotate(-90deg)` ↔ `rotate(0deg)` 之间切换；
> 本实现按状态换 `ChevronRight` / `ChevronDown`，视觉等价，不做旋转动画。
>
> **`stepper_row` 的性质说明**：「◀ 值 ▶」在雷云里**不存在**。
> 它是本轮之前为了「数值可调」而自造的控件，因此**不能算作已对齐**。
> 49 处里，**枚举型已全部换成 `.s3-dropdown`**（见 §6.7），
> 剩下的 41 处是**连续型**数值（百分比、毫米），应换滑块——
> `Slider` 需要 `Entity<SliderState>`（每个可调项一份状态），是一次独立重构。

### 5.3 无法在本实现中复现的差异

| 差异 | 原因 |
|---|---|
| 原版把「仪表盘 / 应用设置」与「设备页」做成**两个独立窗口**（`openNewTab` 打开 `products/{id}/ui/index.html`） | 本实现只有一个窗口，因此把设备选择放进顶栏左区，并用 `Tab::Home` / `Tab::Setting` 承载那两个页面 |

### 5.4 已与实测一致、不需改的部分

设备页内部骨架 `.body-widgets` / `.widget`（600px 两列）、`.widget-prod`（250px）、
配色与圆角令牌映射、`.nav` 胶囊的配色与各状态色值、`body,html` 的字体与最小尺寸。

---

## 6. 共用部件规格对照（逐条核对）

本节把**跨页面复用的部件**与雷云真实 CSS 逐属性对齐。数值全部引自
`.ref/devices/777/static/css/main.e4bab2aa.css`（3644 条规则）与
`.ref/frontend/static/css/55.a5b041a2.chunk.css`（3138 条规则）。

### 6.1 卡片表面 `.widget`

```css
.body-widgets .widget { flex:0 0 auto; font-size:14px; height:auto;
                        margin:10px auto; max-width:600px; min-width:600px;
                        padding:30px 40px; position:relative;
                        transition:height .2s;
                        background-color:#111; border-radius:5px }
.body-widgets .widget.disabled > div { pointer-events:none; opacity:.3 }
```

| 属性 | 真实值 | 本实现 | 状态 |
|---|---|---|---|
| `flex` | `0 0 auto` | `flex_grow(0.).flex_shrink_0()` | ✅ |
| `margin` | `10px auto` | `.my(10).mx_auto()` | ✅ 本轮补上 `mx_auto` |
| `min/max-width` | `600px` | `min_w(600).max_w(600)` | ✅ |
| `padding` | `30px 40px` | `.py(30).px(40)` | ✅ |
| `position` | `relative` | `.relative()` | ✅ 本轮补上 |
| `background` | `#111` | `GroupBox` 默认 `theme.group_box` = `#111` | ✅ |
| `border-radius` | `5px` | `geometry::WIDGET_RADIUS` = 5 | ✅ 本轮修（原用 `theme.radius`=3px） |
| `font-size` | `14px` | `theme.font_size` = 14px | ✅ |
| `disabled` | 子元素 `opacity:.3` | 各页自行控制 | ⚠️ 未做成统一状态 |
| `transition:height` | `.2s` | 无 | ⚠️ GPUI 侧未接 |

> `padding:10px 20px 20px` 属于 `.body-wrapper`，由外壳滚动容器承担；
> `.body-widgets` 此前**重复**加过一次，本轮已删除。

### 6.2 按钮 `.thx-btn`

```css
.thx-btn { display:inline-block; padding:.5rem 1.5rem; text-align:center;
           text-transform:uppercase; transition:opacity .3s; width:auto;
           background-color:#44d62c; border-radius:3px; color:#000 }
.thx-btn:hover  { opacity:.8 }
.thx-btn:active { opacity:.6 }
.thx-btn.fit, .thx-btn.inline { font-size:12px; height:27px; line-height:12px;
                                margin:auto; border:1px solid #0000004d }
.thx-btn.fit { max-width:fit-content; padding:7px 5px 6px }
.thx-btn.sm { font-size:12px; height:27px; width:90px }
.thx-btn.secondary { background-color:#707070; color:#fff }
.thx-btn.disabled, .thx-btn.disabled:hover { cursor:default; opacity:.3 }
```

| 属性 | 真实值 | gpui-kit `Button` | 本实现 `btn()` |
|---|---|---|---|
| 高度 | `27px`（`.fit`/`.inline`/`.sm`） | `small()` → `h_6()` = **24px** | `.h(27px)` ✅ |
| 横向内距 | `5px`（`.fit`） | `small()` → `px_2()` = **8px** | `.px(5px)` ✅ |
| 圆角 | `3px` | 主题 `radius` = 3px | `.rounded(3px)` ✅ |
| 字号 | `12px` | 主题 `font_size` = 14px | ⚠️ 14px（见下） |
| 宽度 | `.fit` = `fit-content`；`.sm` = 固定 90px | 内容自适应 | `fit-content` ✅ |
| `text-transform` | `uppercase` | 无该属性 | 标签 `.to_uppercase()` ✅ |
| `:hover` / `:active` | `opacity:.8` / `.6` | 主题色令牌 | `#3db22a` / `#368e28` ✅ |

**为什么封装 `widgets::btn()`**：48 处 `Button::new` 里 47 处形状相同
（`Button::new(ID).small()[.rounded(…)].label(LABEL)`）。按
《Design Guides · Components and composition》
「the application owns composition and product semantics」，
把规格收成一个入口，避免 47 处各写一遍、改规格要改 47 处。
本轮已全部替换，只剩 `btn()` 内部一处 `Button::new`。

### 6.3 顶栏标签 `.nav`

见 §1.4.1 的完整对照表。摘要：圆角 14px、内距 `7px 10px`、字号 12px、
行高 14px、右距 20px、`#999` / hover `#2d2d2d`+`#ccc` /
active `#44d62c`+`#111` / `:active` `#3cbf27`。

> **`#3cbf27` 与 `#3db22a` 是两个不同的值，本轮曾被我混用一个令牌。**
> - `.nav-tabs .nav:active { background-color:#3cbf27 }` —— 产品级显式值，全站 14 处
> - `.thx-btn:hover { opacity:.8 }` 与底色 `#222` 混合 → `#3db22a`
> - `.thx-btn:active { opacity:.6 }` 与底色 `#222` 混合 → `#368e28`
>
> 前两者含义不同，现已分开：主题里 `button_primary_hover = #3db22a`（按钮），
> `src/app.rs` 里 `NAV_ACTIVE_BG = #3cbf27`（顶栏标签）。

### 6.4 提示气泡 `.tip`

```css
.widget .tip { font-size:14px; line-height:18px; max-width:300px; padding:8px 10px;
               position:absolute; right:14px; top:34px; text-align:left;
               text-transform:none; white-space:pre-wrap; width:max-content;
               background-color:#000; border:1px solid #5d5d5d; color:#ccc }
```

| 属性 | 真实值 | 本实现 | 状态 |
|---|---|---|---|
| 底色 / 边框 / 字色 | `#000` / `1px solid #5d5d5d` / `#ccc` | gpui-kit `Tooltip` 走主题 `popover`(`#111`) + `border`(`#5d5d5d`) | ⚠️ 底色差 1 档 |
| 字号 / 行高 | `14px` / `18px` | 库默认 | ⚠️ |
| 内距 / 最大宽 | `8px 10px` / `300px` | 库默认 | ⚠️ |
| **圆角** | **无（直角）** | 库默认有圆角 | ⚠️ 明显差异 |

> 提示气泡由库统一渲染，逐项覆盖需要改成自绘提示。已登记为待办。

### 6.5 开关 `.switch`

```css
.switch { display:inline-block; position:relative; width:32px; height:18px;
          padding:2px; background-color:#707070; border:1px solid #0000004d;
          border-radius:16px; transition:background-color .3s }
.switch:hover { opacity:.7 }
.switch.on { background-color:#44d62c }
.switch .handle { position:absolute; left:1px; top:1px; width:14px; height:14px;
                  background-color:#111; border-radius:7px; transition:left .2s }
.switch.on .handle { left:15px }
.switch.disabled { opacity:.3 }
```

| 属性 | 真实值 | 本实现 | 状态 |
|---|---|---|---|
| 轨道 | `32×18`，`padding:2px` | `geometry::SWITCH_W/H/PAD` | ✅ |
| 圆角 | `16px`（胶囊） | `SWITCH_RADIUS` = 16 | ✅ |
| 关态底色 | **`#707070`** | `theme.switch`（**本轮由 `#333` 改正**） | ✅ |
| 开态底色 | `#44d62c` | `theme.primary` | ✅ |
| 滑块 | `14×14` | `SWITCH_THUMB` = 14 | ✅ |
| 滑块底色 | **`#111`** | `theme.switch_thumb`（**本轮由 `#ccc` 改正**） | ✅ |
| 滑块圆角 | `7px`（圆形） | `SWITCH_THUMB_RADIUS` = 7 | ✅ |
| 滑块位移 | `left:1px` → `15px` | `SWITCH_THUMB_OFF/ON` | ✅ |
| 边框 | `1px solid #0000004d` | `SWITCH_BORDER` | ✅ |
| `:hover` | `opacity:.7` | `.hover(opacity .7)` | ✅ |
| `disabled` | `opacity:.3` | 未接（见 §5.2） | ⚠️ |

> **几何自洽性验证**：轨道 32 − 内距 2×2 − 滑块 14 = **14**，
> 正好等于 `15 − 1` 的位移。这说明真实值用的是 `box-sizing:border-box`
> （GPUI 的边框同样是内绘），据此实现。

> **为什么不用 gpui-kit 的 `Switch`**：
> 1. 几何按 `Size` 写死，`Small` = 28×16、默认 = 36×20，**都不是 32×18**（滑块是 12 / 16，不是 14）。
> 2. 轨道圆角取自 `cx.theme().radius`（`if theme.radius >= px(4.) { 全圆角 } else { theme.radius }`），
>    而 `impl Styled for Switch` 的 `.rounded()` 作用在**外层 wrapper** 上，碰不到轨道。
>    全站 `radius` 是 3px，改成 ≥4px 会连带改掉整个应用。
> 3. 主题里的 `switch_thumb` 此前被我设成浅色，方向就是反的。

> **调用点**：`toggle_button` 共 **44 处**（`customize/display/enhancement/eq/haptics/
> keyboard/mic/mixer/oled/performance/power/scrolling/setting/sound`），
> 本轮全部改为渲染真实开关。

### 6.6 下拉 `.s3-dropdown` / `.s3-options`

```css
.s3-dropdown { font-size:14px; height:27px; max-height:27px; line-height:17px;
               padding:4px 5px; text-transform:none; width:100%;
               background-color:#0000; border:1px solid #515151; color:#ccc }
.s3-dropdown:hover, .s3-dropdown.expand { border:1px solid #44d62c }
.s3-dropdown.disabled { pointer-events:none; opacity:.3 }
.s3-dropdown.placeholder { color:#666 }
.s3-options { position:absolute; margin-top:1px; flex-direction:column;
              overflow-y:auto; min-width:100%; width:fit-content;
              background-color:#000; border:none }
.s3-options.expand { height:auto; max-height:180px; border:1px solid #515151 }
.s3-options .option { font-size:14px; height:25px; min-height:25px; padding:4px;
                      width:100%; overflow:hidden; white-space:nowrap }
.s3-options .option:hover { background-color:#ffffff1a }
.s3-options .option.selected, .s3-options .option:active { color:#44d62c }
```

| 属性 | 真实值 | 本实现 `widgets::select_row` | 状态 |
|---|---|---|---|
| 面高 / 内距 | `27px` / `4px 5px` | `DROPDOWN_H` / `DROPDOWN_PAD_X` | ✅ |
| 面底色 | 透明 (`#0000`) | `ButtonVariant::Ghost`（无填充） | ✅ |
| 面边框 | `1px solid #515151` | `DROPDOWN_BORDER` | ✅ |
| **悬停/展开变绿边** | `border:1px solid #44d62c` | `.hover(border_color(primary))` | ✅ **本轮实现** |
| 圆角 | 无（直角） | 未覆盖库默认 | ⚠️ 差 3px |
| 列表底色 | `#000` | `DROPDOWN_LIST_BG` | ✅ |
| 列表边框 | `1px solid #515151` | `DROPDOWN_BORDER` | ✅ |
| 列表最大高 | `180px` | `DROPDOWN_LIST_MAX_H` | ✅ |
| 选项高 / 内距 | `25px` / `4px` | `DROPDOWN_OPTION_H/PAD` | ✅ |
| 选项 hover | `#ffffff1a` | `DROPDOWN_OPTION_HOVER` | ✅ |
| 选中项字色 | `#44d62c` | `theme.primary` | ✅ |
| 占位字色 | `#666` | 主题无该档位 | ⚠️ |
| 不可选项 | `color:#cccccc4d` | 未实现（当前所有选项都可选） | ⚠️ |

> **弹层用 gpui-kit 的 `Popover`**：定位、点击外部关闭、Esc 关闭都由它负责，
> 不需要自绘 `deferred` / `anchored`。
>
> **两个实现细节**（都来自编译器约束，已写在代码注释里）：
> 1. **触发元素必须是 `Button`** —— `Popover::trigger` 要求 `Selectable`，
>    而 `Stateful<Div>` 没有实现它。因此用 `ButtonVariant::Ghost` 再逐项覆盖。
> 2. **开合是受控的**（`AppShell::open_select`）—— 选中后要收起列表，
>    而选项点击回调的上下文是 `&mut App`，拿不到 `Context<PopoverState>`
>    （`PopoverState::dismiss` 要后者）。状态放到应用侧后，选项回调只需 `&mut App`。
>
> **为什么不用库的 `Select` / `PopupMenu`**：
> `Select` 要求为每个下拉实现一个 `SearchableListDelegate` 泛型，且外观取自
> `theme.input`（`#111`）与 `theme.radius`（3px），**没有**透明底 / `#515151` /
> 悬停变绿这三个特征；`PopupMenu` 是 **Action 驱动**的
> （`menu(label, Box<dyn Action>)`），而这里要的是「从一组字符串里选一个」——
> Actions 是类型，无法在运行时生成。

---

### 6.7 已换成真下拉的枚举（本轮）

判据：**取值来自一个固定的字符串集合**。这类在雷云里一律是 `.s3-dropdown`。

| 页面 | 设置项 | 枚举 | 选项数 | 原先的控件 |
|---|---|---|---|---|
| 性能 | 回弹模式 | `DebounceMode` | 2 | 「◀ 值 ▶」两处箭头调用**同一个**循环函数（本身是错的） |
| 性能 | 轮询率 | `PollingRate` | 7 | 「◀ 值 ▶」 |
| 性能 | 抬升距离 | `LiftOffDistance` | 3 | 「◀ 值 ▶」 |
| 滚动 | 模式 | `ScrollingMode` | 2 | 「◀ 值 ▶」 |
| 音效增强 | 模式 | `AudioEnhancement` | 3 | 「◀ 值 ▶」 |
| 声音 | 音效增强 | `AudioEnhancement` | 3 | 「◀ 值 ▶」 |
| 灯光 | 效果 | `LightingEffect` | 10 | 「◀ 值 ▶」 |
| 设置 | 配置文件切换 | `ProfileSwitchMode` | 2 | 「◀ 值 ▶」，同样两处箭头调同一个函数 |
| 麦克风 | 采样率 | `SamplingRate` | 3 | 「◀ 值 ▶」 |
| 键盘 | Snap Tap 每组录入模式 | `SnapTapMode` | 5 | 一个循环 `Button` |

配套改动：

- 每个枚举新增 `pub const LABELS: [&'static str; N]`，**与 `ALL` 同序**，
  并加 `set_*` 方法按值写入。原先的 `cycle_*`（循环）方法在改用下拉后
  全部无调用方，已删除——顺带把 `cycle` 这个泛型辅助函数也删了
  （不留无人调用的「备用 API」）。
- `select_row` 的 `id` / `label` 改为 `SharedString`：同一页可能有多个同类下拉
  （Snap Tap 每组一个），必须能拼出互不相同的元素 id。
  相应地把 `AppShell::open_select` 也改成 `Option<SharedString>`。

---

### 6.8 滑块 `.slider-container`（本轮新增）

```css
.slider-container { position:relative; height:64px; opacity:.3;
                    pointer-events:none; transition:opacity .3s }
.slider-container.on { opacity:1; pointer-events:auto }
.slider-container.no-tip { height:36px }
.has-slider .slider-container { margin-left:30px; width:490px }
.slider-container .track { position:absolute; bottom:25px; height:6px; width:100%;
                           background:#44d62c4d; border-radius:3px; z-index:1 }
.slider-container .left, .slider-container .right {
                           position:absolute; bottom:25px; height:6px; z-index:2;
                           background:#44d62c; border-radius:3px }
.slider-container .foot { position:absolute; bottom:-2px; text-transform:uppercase }
.slider-container .title-more { position:absolute; bottom:-25px; font-size:12px;
                           line-height:14px; color:#999; text-transform:uppercase }
.slider::-webkit-slider-thumb { background:#44d62c; border-radius:8px;
                                width:16px; height:16px }
.slider-container.on .slider::-webkit-slider-thumb:hover
                           { background:#5d5d5d; border:2px solid #44d62c }
.slider-container.on .slider::-webkit-slider-thumb:active
                           { background:#383838; border:2px solid #44d62c }
```

| 属性 | 真实值 | 本实现 `widgets::slider_row` | 状态 |
|---|---|---|---|
| 容器 | `height:64px`、`width:490px` | `SLIDER_H` / `SLIDER_W` | ✅ |
| 轨道 | `6px`、`bottom:25px`、`border-radius:3px` | `SLIDER_TRACK_H/BOTTOM/RADIUS` | ✅ |
| 轨道底色 | `#44d62c4d`（30% 绿） | `SLIDER_TRACK_BG` | ✅ |
| 已填充 | `#44d62c`、6px、圆角 3px | `SLIDER_FILL_BG` | ✅ |
| 手柄 | `16×16`、`border-radius:8px`（圆形） | `SLIDER_THUMB/RADIUS` | ✅ |
| 手柄常态 | 实心 `#44d62c`，**无边框** | `SLIDER_THUMB_BG` | ✅ |
| 手柄 hover | `#5d5d5d` 底 + `2px solid #44d62c` | `.hover(...)` | ✅ |
| 数值文字 | `.foot { bottom:-2px }`、`#999` | `SLIDER_FOOT_BOTTOM` | ✅ |
| 不可用 | `opacity:.3` | `SLIDER_DISABLED_OPACITY` | ✅ |
| 手柄 active | `#383838` + 绿边 | 未接 | ⚠️ |

> **为什么不用 gpui-kit 的 `Slider`**：
> 1. **没有变更回调** —— `Slider` 只暴露 `new/horizontal/vertical/disabled/reverse`，
>    值只能从 `Entity<SliderState>` 里读；本项目的模型在 `AppShell` 里，
>    需要「值变了就写回模型」这条路径。
> 2. **手柄多了光环** —— 库里画的是 `THUMB_RING_WIDTH = px(3.)` 的 `theme.ring` 光环，
>    而真实常态是**无边框实心圆**，绿边只出现在 hover / active。
> 3. **需要 `Entity<SliderState>`** —— 它要靠 `Window::use_keyed_state` 持有，
>    而页面签名是 `fn render(app, cx)`，拿不到 `&mut Window`；要么改 24 个页面
>    的签名，要么在渲染期往 `AppShell` 里塞 41 个实体。
>
> **拖动**用 gpui 的鼠标事件：`MouseMoveEvent` 自带 `pressed_button`，
> 因此**不需要自己维护「正在拖动」的标志位**；元素宽度由内部的 `canvas` 捕获，
> 再把窗口横坐标换算成吸附到 `step` 的值。

---

## 7. 证据来源

| 内容 | 文件 |
|---|---|
| 外壳 DOM（`main-container` / `nav-tabs` 三区 / `profile-wrapper` / `navs-wrapper` / `right` / 电池） | `.ref/frontend/static/js/App.eb32d7cd.chunk.js` |
| 设备页打开方式（`addDeviceUITab` / `getDeviceTabName` / `openNewTab`） | 同上 |
| 外壳 CSS（`.main-container` / `.nav-tabs` / `.nav` / `.body-wrapper` / `.dashboard` / `.box-group` / `.main-setting` / `.side-navigation`） | `.ref/frontend/static/css/55.a5b041a2.chunk.css`（3138 条规则） |
| `DeviceCard` 与配对相关内容 | `.ref/frontend/static/css/4130.6bdf8dd0.chunk.css`（217 条规则） |
| 设备页内部 CSS（`.body-widgets` / `.widget` / `.widget-prod` / `.thx-btn`） | `.ref/devices/{182,653,777}/static/css/main.*.css` |
| 每设备标签页集合 | `.ref/tools/gen-screen-docs.js` 的 `DEVICE_TABS` |

### 复现命令

```powershell
node .ref/tools/audit-css.js ".ref/frontend/static/css/55.a5b041a2.chunk.css" main-container nav-tabs side-navigation
node .ref/tools/grep-css.js  ".ref/frontend/static/css/4130.6bdf8dd0.chunk.css" "DeviceCard_deviceCard" 4
node .ref/tools/dump-shell.js
node .ref/tools/frontend-inventory.js
```


