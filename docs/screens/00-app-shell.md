# 应用外壳、主前端与产品导航

本文只描述雷蛇 Synapse 4 的外壳层级。外层 Electron 标签栏、主前端宿主页、
产品模块页是三个不同的渲染层，不能合并成一条标题栏，也不能把其中一层的按钮
或导航复制到另一层。

证据来自实际保存的 Electron 和前端构建产物：

- `.ref/synapse-asar/electron/index.css`
- `.ref/synapse-asar/electron/components/Tab/TabUI.js`
- `.ref/synapse-asar/electron/assets/image/tab/*.svg`
- `.ref/frontend/static/js/App.eb32d7cd.chunk.js`
- `.ref/frontend/static/js/4130.155387bf.chunk.js`
- `.ref/frontend/static/js/9388.2bec5db3.chunk.js`
- `.ref/frontend/static/css/55.a5b041a2.chunk.css`
- `.ref/frontend/static/css/4130.6bdf8dd0.chunk.css`

---

## 1. 三层结构

```text
Electron host window
└─ .etabs-tabgroup / Rust app_tab_bar       42px
   ├─ .etabs-tabs / Rust app-tab-drag-region 外层动态 tab 与拖动区
   └─ .etabs-window-control-btns             Windows 非 macOS 时创建
      ├─ .etab-minimize                      48px 命中区
      ├─ .etab-restore                       48px 命中区
      └─ .etab-close                         48px 命中区

Web content hosted by Electron
└─ .main-container
   ├─ .nav-tabs                             48px 主前端/产品页 header
   │  ├─ .profile-wrapper                   左侧配置文件区
   │  ├─ .navs-wrapper                     中间页面导航
   │  └─ .right                            右侧状态与辅助入口
   └─ #body-wrapper                         页面内容和滚动区域
      ├─ Dashboard                          主前端首页
      ├─ .main-setting                      应用设置
      └─ 产品模块 body                      设备产品页内容
```

### 1.1 Electron 42px 外层标签栏

Electron 层不是主前端的 `.nav-tabs`。`TabUI.js` 的 `render()` / `addTabGroup()`
动态创建 `.etabs-tabgroup`，并在非 macOS 环境中追加
`.etabs-window-control-btns`。初始化时如果页面中已经存在 `.etabs-tabgroup`，
不会再次创建；外层标签栏只有在收到 `setTabBarVisible` 后添加 `visible` 类才显示。

`TabUI.js` 的 tab 不是固定三项菜单，而是由 Electron 的 tab message 动态维护：

- `createNewTab` 消息调用 `createNewTab(name, url, featureObj, windowId)`，创建一个
  `.etabs-tab`，保存 `url`、`featureObj`、`windowId` 到 `tabList`，并发送
  `tab-create` 回 Electron。
- 第一个 tab 被保存为 `mainTab`，增加 `.main-tab` 和
  `.non-collapsible-tab`；它位于 `.etabs-tabs` 之外的固定主 tab 位置，不参与普通
  tab 的关闭/折叠规则。
- 后续 tab 进入 `.etabs-tabs`，根据持久化位置插入并分配 `pos`、`left`、`width`；
  tab 的增删、激活、标题、图标、加载/失败状态由 `tab-message` 的
  `createNewTab`、`changeActiveTab`、`closeTab`、`changeTitle`、`changeIcon`、
  `startLoading`、`stopLoading`、`failToLoad` 分支更新。
- `mainTab` 关闭时，代码会选择下一个可见 tab；只有普通 tab 显示关闭操作，主 tab
  不显示关闭按钮。`manual-close` 还会强制隐藏普通 tab 的 close 控件。
- `setTabPos` / `setTabPosArr` 和 `saveTabData()` 维护 tab 顺序；恢复时由
  `restoreTabUI` 逐项重新创建并恢复标题、图标、加载状态和可关闭状态。

```css
body {
  margin: 0;
  background-color: #000;
}

.etabs-tabgroup {
  width: 100%;
  height: 42px;
  display: none;
  position: relative;
  z-index: 2;
  background-color: #000;
  cursor: default;
  font: 14px Roboto, sans-serif;
}

.etabs-tabgroup.visible {
  display: flex;
}
```

这 42px 是 Electron host 的 tab strip 高度，不是网页 header 高度。Windows 10
兼容边框另加 `.custom-win10-border`；最大化时 `.custom-win10-border.maximized`
去掉边框并恢复 `width:100%`。这些状态属于 Electron 壳，不属于 Dashboard 或
产品模块。

### 1.2 外层标签与拖动

`TabUI.js` 创建 `.etabs-tabs-wrapper`、左右滚动按钮和 `.etabs-tabs`。可拖动区域
由 CSS 的 `-webkit-app-region: drag` 提供；正在拖动标签或所有交互控件则改为
`no-drag`。外层标签本身也是 `no-drag`，所以点击标签、滚动按钮、关闭按钮不会
拖动窗口。

```css
.etabs-tabs {
  height: 42px;
  width: calc(100vw - 365px);
  overflow: hidden;
  position: relative;
  -webkit-app-region: drag;
}

.etabs-tabs.dragging {
  -webkit-app-region: no-drag;
}

.etabs-tabs-wrapper {
  display: flex;
  align-items: center;
  flex: 1 0 auto;
  margin-left: 4px;
}

.etabs-tabs-wrapper::after {
  content: "";
  flex: 1 0 auto;
  height: 100%;
  -webkit-app-region: drag;
}
```

外层标签的主要状态来自 `TabUI.js` 和 `index.css`：

- `.etabs-tab` 默认隐藏；`.visible` 才参与显示，宽度范围为 `90px–240px`。
- `.main-tab` 是当前窗口主 tab；它位于标签区底部并带 `non-collapsible-tab`。
- 普通标签的背景为 `#000`；hover 使用从 `#222` 到 `#444` 的渐变。
- active 标签使用从 `#44d62c` 到 `#222` 的渐变，实际内容区为 `#222`。
- 标签标题为 `#ccc`，非 active 标题为 `#999`，大写、单行、最大宽度 `200px`。
- 标签可以在 `.etabs-tabs` 内拖动排序；顺序通过 `tab-updated` 和 localStorage 的
  `tab` 数据保存。
- 标签过多时，`#scroll-left-btn` 与 `#scroll-right-btn` 根据滚动位置显示或隐藏，
  并切换 default / hover / active / disabled SVG。

原始 DOM 的关系应保持为：

```text
.etabs-tabgroup
├─ .etabs-tabs-wrapper
│  ├─ #scroll-left-btn
│  ├─ .etabs-tabs
│  │  └─ 普通动态 .etabs-tab...
│  └─ #scroll-right-btn
└─ .etabs-window-control-btns
```

`mainTab` 不是把一个普通 tab 再复制一份，而是原始 `createNewTab` 在首次创建时
赋予 `.main-tab` 的主 tab；普通 tab 才进入可滚动的 `.etabs-tabs` 列表。

### 1.3 三个系统按钮：三个 48px 命中区

`TabUI.js` 的 `addWindowControlBtns()` 按固定顺序创建三个 `div`：

```text
.etabs-window-control-btns
├─ .etab-minimize  → callApiElectron({ action: "minimize" })
├─ .etab-restore   → callApiElectron({ action: "maximize" })
└─ .etab-close     → callApiElectron({ action: "handleCloseBtn", ... })
```

```css
.etabs-window-control-btns {
  display: flex;
  position: absolute;
  right: 0;
  top: 0;
  bottom: 0;
  height: 100%;
}

.etabs-window-control-btns > div {
  width: 48px;
  display: flex;
  align-items: center;
  justify-content: center;
  background-repeat: no-repeat;
  background-position: center;
  user-select: auto;
  -webkit-user-select: auto;
  -webkit-app-region: no-drag;
}

.etabs-window-control-btns > div:hover {
  background-color: #222;
}

.etab-minimize { background-image: url("./assets/image/tab/minimize.svg") }
.etab-restore  { background-image: url("./assets/image/tab/maximize.svg") }
.etab-close    { background-image: url("./assets/image/tab/close.svg") }
.etab-restore.restore-icon {
  background-image: url("./assets/image/tab/restore.svg");
}
```

按钮的**命中区**是 `48px × 42px`，不能把 SVG 的尺寸误当成按钮尺寸：

| 资源 | 原始 SVG 尺寸 | 实际绘制重点 |
|---|---:|---|
| `minimize.svg` | `48 × 32` | `#999` 最小化横线；透明背景矩形也是 `48 × 32` |
| `maximize.svg` | `48 × 32` | 中央 `12 × 12` 的 `#999` 空心方框 |
| `restore.svg` | `13 × 13` | 最大化后替换 restore 图标 |
| `close.svg` | `12.7 × 12.7` | `#999` 叉号；外围命中区仍为 `48px` |

按钮 hover 只改变外围 `#222` 背景，图标默认 `#999`。最大化状态由
`checkMaximizeWindow()` 查询 Electron 的 `isMaximized`，为 `.etab-restore` 添加
`restore-icon`，同时给 `.etabs-tabgroup` 添加 `maximized`。不可调整大小的主 tab
会隐藏 restore 按钮；这是窗口能力条件，不应在网页 `.nav-tabs` 中复制一套按钮。

### 1.4 当前 Rust 外壳的对应关系

当前 `src/shell.rs` 已经将外层高度、拖动区和窗口控制区拆开：

```text
render()
└─ app_tab_bar()                         h = 42px
   ├─ host_tabs()                        flex:1, app-tab-drag-region
   │  ├─ host-main-tab                   主 tab，仅显示 Synapse 图标
   │  └─ host-drag-region                剩余窗口拖动区
   └─ window_controls()
      ├─ window-minimize                 WindowControlArea::Min
      ├─ window-maximize                 WindowControlArea::Max
      └─ window-close                    WindowControlArea::Close
```

对应关系和边界如下：

- `app_tab_bar()` 使用 `APP_TABGROUP_HEIGHT = 42.0`，是原始 `.etabs-tabgroup` 的
  Rust 视觉层；其下才是 `top_bar()` 的 48px 主前端 `.nav-tabs`。
- `host_tabs()` 将主 tab 与剩余拖动区分开；`host-drag-region` 使用
  `WindowControlArea::Drag`，主 tab 和三个系统按钮不会拖动窗口。
- Rust 当前主 tab 宽 `90px`、高 `40px`，与原始 `.etabs-tab` 的宽度范围和底部对齐
  意图一致；系统按钮仍保持 `48px` 宽、`42px` 高。
- 当前项目尚未接入 Electron 的多窗口 tab IPC，因此只渲染源代码明确存在的主 tab；
  不再把“首页/已关联游戏/当前设备”伪装成 Electron 外层 tab。产品页面导航仍由
  下方 48px 的 `top_bar()` 负责。
- 原始 Electron 的 `tab-message` 多 tab 列表、`mainTab`、滚动按钮和持久化位置是
  参考行为；当前 Rust 外层尚未把这些 JS 对象/消息协议逐字复制为独立的数据结构，
  因此文档不能把 Rust 的单个主 tab 描述成原始 Electron 的完整动态 tab manager。
- Rust 的 `window_controls()` 使用独立的 `window_control_area` 命中区和内嵌 SVG；
  视觉上对应 `.etabs-window-control-btns > div`，不是产品 `.nav-tabs` 的右侧区域。

---

## 2. 主前端 header：宿主页面的 48px `.nav-tabs`

`App.eb32d7cd.chunk.js` 的 header renderer 返回的是一个 React `Fragment`，其中的
网页 header 是：

```jsx
<div className="nav-tabs">
  <div className="profile-wrapper">...</div>
  <div className="navs-wrapper" role="tabs">...</div>
  <div className="right" role="tablist">...</div>
</div>
```

它不是 Electron 的 42px tab strip。主前端 header 负责宿主页面的 profile、首页/模块
导航和右侧状态入口；它不负责最小化、最大化、关闭或窗口拖动。

```css
.main-container {
  display: flex;
  flex-direction: column;
  position: absolute;
  width: 100%;
  height: 100%;
  min-width: 600px;
  background-color: #222;
}

div.nav-tabs {
  display: flex;
  align-items: center;
  position: relative;
  width: 100%;
  min-height: 48px;
  z-index: 106;
  color: #5d5d5d;
  background-color: #222;
  border-bottom: 2px solid #000;
}

.nav-tabs .profile-wrapper { flex: 1 0 25%; }
.nav-tabs .navs-wrapper {
  display: flex;
  flex: 1 0 max-content;
  justify-content: center;
  font: 12px Roboto, sans-serif;
}
.nav-tabs .right { flex: 1 1 25%; }
```

### 2.1 Header 三个区域的作用

| 区域 | 作用 | 原代码条件/内容 |
|---|---|---|
| `.profile-wrapper` | 当前 profile、动态模式提示、keymap bar | `renderProfileBar`、`hasDynamicMode && activeDynamicMode`、`renderKeyMapBar` 条件渲染 |
| `.navs-wrapper` | 主前端页面导航 | `visibleNavs.map(...)` 渲染 tab；有溢出项时增加 `dots3` 菜单 |
| `.right` | 宿主状态和辅助入口 | `displayMode` 警告、教程/扩展、电池、帮助等按 props 条件渲染 |

页面导航 `.nav` 是主前端的 tab，不是 Electron 标签：

```css
.nav-tabs .nav {
  margin-right: 20px;
  padding: 7px 10px;
  border-radius: 14px;
  color: #999;
  line-height: 14px;
  text-align: center;
  text-transform: uppercase;
  white-space: nowrap;
  transition: background-color .3s, color .1s;
}

.nav-tabs .nav:hover  { background-color: #2d2d2d; color: #ccc; }
.nav-tabs .nav:active { background-color: #3cbf27; color: #111; }
.nav-tabs .nav.active,
.nav-tabs .nav.active:hover {
  background-color: #44d62c;
  color: #111;
}
.nav-tabs .nav.disabled {
  opacity: .3;
  pointer-events: none;
}
```

主前端 header 还存在宿主状态条件：

- `showLinkedGames` 时 `.nav-tabs` 增加 `disabled`，CSS 为整个 header 设置 `opacity:.5`。
- `.main-container.backdrop-on .nav-tabs` 设置 `opacity:.3; pointer-events:none`，用于
  backdrop/模态遮罩期间冻结宿主导航。
- 每个导航项使用 `role="tab"`、`aria-selected` 和当前 view；不能只渲染无事件的文本。
- `.dots3` 只承载由于空间不足而隐藏的页面项；有 active 溢出项时增加
  `has-actived-option`，背景变为 `#44d62c`。

### 2.2 `#body-wrapper`：header 下唯一内容区域

主前端 header 下方不是另一个标题栏，而是 `#body-wrapper`。前端 DOM 使用状态类
控制滚动：有 status 时保持普通类，没有 status 时追加 `scrollable`。

```css
.body-wrapper {
  flex: 1 1;
  width: 100%;
  height: 100%;
  min-width: 600px;
  padding: 10px 20px 20px;
}

@media screen and (max-width: 1279px) {
  .body-wrapper { padding: 10px 30px 20px; }
}

#body-wrapper.body-wrapper.scrollable {
  overflow: auto;
}
```

`.main-container` 的垂直关系是：Electron 42px 外层 → web view → 48px 网页
`.nav-tabs` → `#body-wrapper`。实现时不能将 body padding、Dashboard 顶部间距或
产品图区向上挪动来抵消 Electron 外层高度。

---

## 3. Dashboard：真实渲染和交互状态

Dashboard renderer 在前端 JS 中渲染教程/介绍 banner（按 props 条件），再渲染：

```jsx
<div className="dashboard flex [reflow]">
  <div style={{ display: "flex", flexDirection: "column" }}>
    {renderGroup()}
  </div>
</div>
```

当设备项数量大于 4 时，JS 为 Dashboard 增加 `reflow`；CSS 将最大宽度从 `1220px`
扩展到 `2460px`。普通 Dashboard 居中，最小宽度 `620px`，底部保留 `50px`。

```css
.dashboard {
  display: flex;
  flex-direction: column;
  position: relative;
  margin: 0 auto;
  min-width: 620px;
  max-width: 1220px;
  transition: height .2s linear;
}

.dashboard.reflow { max-width: 2460px; }
.body-wrapper .dashboard { margin-bottom: 50px; }
```

### 3.1 分组的存在条件、折叠和拖动

`SimpleBoxGroups` 的 render 只有在 `boxItems` 存在且长度不为 0 时返回
`.box-group`。分组折叠状态来自 `groupsCollapsed[groupTitle]`；点击
`.collapse-action` 调用 `toggleDashboardGroupCollapsed(groupTitle, nextValue)`，
不是静态箭头。

```css
.dashboard .box-group {
  width: 100%;
  max-width: 100%;
  min-height: 18px;
  margin: 10px 0;
  transition: all .1s ease-in-out;
}

.dashboard .box-group .title {
  display: flex;
  align-items: stretch;
  position: relative;
  width: 100%;
  color: #ccc;
  font-size: 14px;
}

.dashboard .box-group .title .collapse {
  display: flex;
  align-items: center;
  flex: 1 1;
  position: relative;
  z-index: 2;
}

.dashboard .box-group .title .collapse .icon {
  width: 10px;
  height: 10px;
  margin-right: 10px;
  transform: rotate(-90deg);
  transition: transform .3s linear;
}

.dashboard .box-group.expand .title .icon {
  transform: rotate(0deg);
}
```

展开/折叠不是立即移除 DOM：

- `.content` 折叠时 `max-height:0`，展开时最高 `2000px`，顶部间距 `10px`。
- `.content-inner` 使用 flex-wrap 和 `gap:20px`，并以 `translateY(-100%)` / `0` 做过渡。
- 非展开状态背板最高 `40px`，颜色 `#333`，圆角 `5px`，默认透明且不可见。
- 展开状态背板最大高度 `2000px`，底部延伸到内容区域。
- 分组拖动时背板为 `#3333334d`、`2px solid #44d62c`，并置于交互层上方。
- 标题 hover 或 `.show-drag-icon` 时才淡入 `22 × 19px` 的拖动图标；拖动光标为
  `grab`，实际拖动为 `grabbing`。

组内设备顺序由 JS 的 `itemsOrder` 管理。拖动时根据卡片的列数计算目标行列和位移，
释放后通过 `updateGroupItemOrder` 写回对应 group；因此 Dashboard 不能只画一组静态
卡片，也不能按数组 index 作为持久身份。

### 3.2 DeviceCard 的尺寸、内部布局和状态

DeviceCard 的 CSS 声明如下：

```css
.DeviceCard_deviceCard {
  display: flex;
  flex-direction: column;
  position: relative;
  width: 290px;
  min-height: 220px;
  padding: 10px;
  background: #0000004d;
  border-radius: 5px;
  transition: border-color .3s;
}

.DeviceCard_deviceImageContainer {
  display: flex;
  align-items: center;
  justify-content: center;
  position: relative;
  flex: 1 1;
  min-height: 140px;
  max-height: 140px;
}

.DeviceCard_deviceImage {
  width: auto;
  height: auto;
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
}

.DeviceCard_deviceInfo {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: flex-start;
  min-height: 50px;
  gap: 8px;
  font-size: 14px;
  text-align: center;
  text-transform: uppercase;
}
```

`width:290px`、`min-height:220px`、`padding:10px` 是原 CSS 声明；内部图片区固定
`140px`，信息区至少 `50px`，名称不能通过固定单行高度或 index-based 截断造成溢出。
布局实现应保留原 CSS 的内容区计算方式，不额外叠加一套“产品页 600px widget”。

DeviceCard 的 JS 根据设备数据决定有效图片或 category placeholder，并按状态拼接
这些真实类名：

| 状态/类 | 原始样式或行为 |
|---|---|
| `clickable` / `nonClickable` | 可进入设备页或只读；可点击项有 `cursor:pointer`，不可点击项保持默认光标 |
| `disabled` | 图片容器和名称标签 `opacity:.45` |
| `connected` | 卡片边框 `#44d62c4d` |
| `failed` | 卡片边框 `#ff4d4d80` |
| `pairBadge` | `#999` 边框/文字，`20px` 高、圆角 `25px`，用于配对入口 |
| `pairedBadge` | `#44d62c` 边框/文字，最小宽 `66px`，用于已配对操作 |
| `pairingBadge` | `#44d62c` 边框/文字，带 `16px` spinner |
| `unpairingBadge` | `#fd8611` 边框/文字，带橙色 spinner |
| `categoryIcon` | `80 × 80px`，白色图标、`opacity:.3`；按 mouse/keyboard 等分类切换 |

卡片中可执行的 pair/unpair 入口使用 `role="button"`、`tabIndex=0` 和
`data-device-card-action="true"`。JS 对 `Tab` / `Shift+Tab` 在同一卡片的 action
集合内循环，并将 `Space` / `Enter` 转换为点击；这不是 hover-only 装饰。

没有有效设备数据时使用 placeholder/category 图标；组没有 item 时不渲染空的
`.box-group`。扫描、配对、失败和加载状态必须由上述真实设备字段/类驱动，不能用
固定“已连接”文案替代状态。

---

## 4. 应用设置：固定导航 + 右侧内容

设置是主前端 body 中的应用级页面，不是产品模块的 `TAB_*`。宿主 48px header
继续存在；设置页面只改变 `#body-wrapper` 内部内容为 `.main-setting`。

```css
.main-setting {
  width: 100%;
  height: 100%;
}

.main-setting .side-navigation {
  position: fixed;
  width: 180px;
  margin-left: 70px;
  color: #ccc;
  font-size: 14px;
  line-height: 17px;
  text-align: left;
}

.main-setting .side-navigation ul {
  list-style: none;
  padding: 0;
}

.main-setting .side-navigation ul li {
  display: flex;
  flex-direction: column;
  justify-content: center;
  height: 30px;
  margin-bottom: 2px;
  padding-left: 10px;
  border-radius: 5px;
}

.main-setting .setting-content {
  margin-left: 250px;
}

.main-setting .setting-content:last-child {
  padding-bottom: 80px;
}

.main-setting div .body-widgets {
  display: block;
  margin: 0;
}
```

### 4.1 设置导航状态

导航条目是实际可选择的页面入口，当前项由 `.active` 表示；不能将导航写成没有
事件的静态文本：

| 状态 | 背景 | 文字/行为 |
|---|---|---|
| 默认 | 透明 | `#ccc` |
| hover | `#ffffff1a` | `#44d62c` |
| active | 透明 | `#44d62c` |
| active + hover | `#ffffff1a` | 回到 `#ccc` |
| active + pressed | `#000!important` | `#ccc` |
| 默认 + pressed | `#000` | `#44d62c` |

CSS 原规则还要求最后一个 `li` 的 `margin-bottom:0`。导航项高度为 `30px`，条目
之间只有 `2px`，左内边距为 `10px`；左栏整体不是 250px，**180px 是导航本身的
宽度，250px 是右侧内容的起始 margin**。

### 4.2 设置内容样式和条件

- 右侧 `.setting-content` 从 `margin-left:250px` 开始，避免被 fixed 左栏覆盖。
- 设置页的 `.body-widgets` 是块级流，不使用 Dashboard 的 flex 网格，也不使用
  产品页的 `.widget-prod`。
- `.main-setting .widget .title` 为 `18px`；普通粗体说明为 `14px`、`500`、
  `17px` 行高、大写、`#ccc`。
- `.general-setting` 下拉区域在窄窗口下有 `188px` 宽约束；设备设置 widget 在
  CSS 条件下使用 `padding:20px`。
- 内容较长时由 `#body-wrapper` 的 scrollable 状态承载滚动；不能通过固定高度把
  设置内容裁掉。
- 设置页面的 active 项、右侧内容和表单状态必须来自当前设置 view/state。切换项后
  应更新 active 与对应内容，而不是只改变颜色或保留上一页内容。

---

## 5. 产品模块页的 48px 导航

产品模块通过主前端的设备入口交给 TabManager / `openNewTab` 管理；这描述的是
宿主如何打开或聚焦产品 UI，不等于“产品页一定是一个独立窗口”，也不等于产品页
与主前端共用同一份 DOM。

每个产品 web view 自己渲染 `.main-container`、`.nav-tabs` 与 `#body-wrapper`。
它的 `.nav-tabs` 仍是 `48px`、`#222`、底部 `2px solid #000`，但中间
`.navs-wrapper` 换成该产品的 TAB 集合；产品页的 `.right` 仍可承载该页面的电量、
帮助或警告状态。

产品页的 `body-wrapper` 内才放产品模块的 `.widget-prod`、`.body-widgets` 和
600px widget。不能把产品页的 250px 产品图区、600px 卡片网格放到主前端
Dashboard 或应用设置。

### 5.1 三种导航不要混淆

| 层 | 选择器/实现 | 高度 | 负责什么 |
|---|---|---:|---|
| Electron host | `.etabs-tabgroup` / `TabUI.js` | `42px` | 外层 tab、窗口拖动、最小化/最大化/关闭 |
| 主前端 header | `.main-container > .nav-tabs` | `48px` | profile、Dashboard/模块/设置等宿主页面导航、右侧状态 |
| 产品页 nav | 产品 web view 自己的 `.nav-tabs` | `48px` | 设备产品 TAB；其内容是产品模块自己的 body |

允许同时存在 Electron 42px 外层和 web content 的 48px header；禁止额外增加一条
没有原始 DOM/CSS 对应的 34px 网页标题栏，禁止把三个系统按钮塞进任意 `.nav-tabs`。

---

## 6. 原代码证据与实现差异

| 结论 | 原代码证据 |
|---|---|
| Electron 外层为 42px | `electron/index.css` 的 `.etabs-tabgroup`、`.etabs-tabs` |
| 系统按钮由 JS 创建 | `electron/components/Tab/TabUI.js` 的 `addWindowControlBtns()` |
| 三个按钮动作不同 | `minimizeWindow`、`maximizeWindow`、`closeWindow` 三个 listener |
| 每个按钮命中区 48px 且 no-drag | `electron/index.css` 的 `.etabs-window-control-btns > div` |
| SVG 不是命中区尺寸 | `electron/assets/image/tab/minimize.svg`、`maximize.svg`、`close.svg` |
| 主前端 header 为三分区 | `frontend/static/js/App.eb32d7cd.chunk.js` 的 `nav-tabs` renderer |
| header 导航有真实 tab 状态 | 同一 renderer 的 `visibleNavs.map`、`role="tab"`、`aria-selected` |
| body 内容由 `#body-wrapper` 承载 | App shell renderer 与 `55.a5b041a2.chunk.css` |
| Dashboard 分组可折叠 | `4130.155387bf.chunk.js` 的 `toggleGroupCollapsed` 和 `groupsCollapsed` |
| Dashboard 分组无内容时不渲染 | 同文件 `boxItems && boxItems.length !== 0` 的 render 条件 |
| Dashboard 可排序拖动 | 同文件的 `dragMouseDown`、拖动位置计算与 `updateGroupItemOrder` |
| DeviceCard 操作可键盘访问 | `4130.155387bf.chunk.js` 的 `data-device-card-action`、Tab/Space/Enter 处理 |
| DeviceCard 尺寸与状态 | `frontend/static/css/4130.6bdf8dd0.chunk.css` 的 `DeviceCard_*` 规则 |
| 设置左栏和右侧内容分离 | `55.a5b041a2.chunk.css` 的 `.side-navigation`、`.setting-content`、`.body-widgets` |

本次文档修正相对旧版的差异：

1. 删除“所有数值逐条引自 CSS、没有推断”的绝对表述，改为逐条列出证据文件与
   可复核的 JS/CSS 条件。
2. 删除“系统按钮与产品 nav 同一行”的含混描述；系统按钮属于 Electron 42px
   外层，主前端/产品 `.nav-tabs` 属于 web content 的 48px header。
3. 删除“设备页必然是独立窗口”的断言，改为准确描述 `TabManager/openNewTab`
   的宿主管理和产品 web view 自己的 DOM。
4. 补充三个系统按钮的 `48px` 命中区、SVG `48×32` / close `12.7×12.7` 资源尺寸、
   `no-drag`、最大化 restore 状态和实际 Electron action。
5. 补充主前端 header 的渲染结构、导航条件、禁用/backdrop 状态、Dashboard 的 JS
   render 条件与持久排序、设置页的 active/hover/pressed 规则。
