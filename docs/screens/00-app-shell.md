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

### 1.0 外层 Electron 标签栏与产品顶栏是两层不同结构

在主前端全部 CSS 中检索 `title-bar` / `titlebar` / `drag-region` /
`window-controls` / `app-header` / `rz-title` / `top-bar`，**全部 0 命中**。

窗口边框由 Electron 提供；网页内容的第一行就是 `.nav-tabs`：

```css
div.nav-tabs {
  align-items:center; background-color:#222; border-bottom:2px solid #000;
  color:#5d5d5d; min-height:48px; position:relative; width:100%; z-index:106;
}
```

因此原版网页内容的第一行就是 `.nav-tabs`：窗口边框、系统按钮和拖动由 Electron 壳处理，
原版 Electron 壳先渲染 `.etabs-tabgroup`：黑色 `42px` 外层标签栏，右侧是三个各 `48px` 的系统按钮；其下才是产品网页自己的 `.nav-tabs`。系统按钮不属于 `.nav-tabs` 的 profile/nav/right 三个网页区域。GPUI 的 `TitleBar` 只是实现技术，不能额外再渲染一条 `34px` 标题栏；本项目用自绘 `42px` 外层栏承载原版按钮和拖动区。

> 原始前端没有独立网页标题栏；窗口边框和拖动由 Electron 壳处理。

### 1.0.1 Rust 窗口实现边界

- 原版事实：`.nav-tabs` 只有 profile、导航、right 三个网页区域。
- Rust 实现：窗口可以使用 client-side decoration，但最小化、最大化/还原、关闭必须占用同一行右侧的系统命中区。
- 外层 `.etabs-tabs` 拖动区高度 `42px`；三个按钮各 `48px` 且必须是 `no-drag` 命中区。
- 产品 `.nav-tabs` 高度 `48px`，只负责 profile、导航和右侧状态；点击导航、profile、帮助不能触发窗口拖动。
- 不得把 gpui-component 的 34px `TitleBar` 作为雷云页面的额外视觉层；也不得把系统按钮错误塞进产品 `.nav-tabs`。

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
