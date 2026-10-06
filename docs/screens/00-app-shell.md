# 应用外壳、工具栏与产品导航

> 来源迁移（2026-10-02）：旧参考版本已停用，链接已切换到当前核验源码。本文历史压缩符号及未重新审计的结论不得作为最新版确认；以[当前来源与复核记录](../re/20-current-source-version.md)为准。
> Rust 已进入重构版本。本文的原版 JS/CONFIG/CSS 证据继续适用；旧 Rust 对照已作为重构前基线保留，当前代码、已完成项和剩余差异见[重构状态](../re/03-implementation-gap.md)。

## 1. 原版证据和层次

`[JS/CSS]` 宿主 [index.css](../../.ref/host-4.0.827/electron/index.css)、[TabUI.js](../../.ref/host-4.0.827/electron/components/Tab/TabUI.js)，前端 [App chunk](../../.ref/applications/synapse/dashboard/static/js/App.72827d47.chunk.js)、[公共 CSS](../../.ref/applications/synapse/dashboard/static/css/55.4e8559cb.chunk.css)，以及各产品 main JS/CSS。

```text
Electron 宿主
├─ 应用标签栏（42）及 Windows 系统按钮
└─ 当前应用
   ├─ 工具栏（38）
   ├─ 应用 / 产品导航（min 48）
   │  ├─ profile / 条件状态
   │  ├─ 当前应用的页面导航
   │  └─ 帮助 / 电池 / 条件提示
   └─ body-wrapper
      └─ 当前页面自己的内容和滚动
```

`components/Titlebar` 另有替代标题栏，不能与 TabUI 堆叠成额外一条系统栏。应用窗口有各自的宿主配置。

## 2. Electron 标签和系统按钮

| 部位 | CSS / 行为 |
|---|---|
| .etabs-tabgroup | 高 42，#000；默认 display:none，.visible 后 flex |
| .etabs-tabs | 宽 calc(100vw - 365px)，宿主拖动区域 |
| 应用标签 | 最小宽 90、最大宽 240，顶部圆角 5 |
| active 标签 | 外层绿色到 #222 的渐变，内层 #222 |
| 系统按钮 | 每个宽 48、占完整 42 高；no-drag；hover #222 |

标签来自窗口/客户端集合，不是按设备类别硬拼。关闭当前应用标签、关闭整个原生窗口、关闭页面弹窗是不同动作。最大化状态决定 maximize/restore；SVG 路径尺寸与命中区尺寸不能混为一谈。

外层标签必须真正激活对应应用，并具有正确的 active、关闭和标题溢出行为。

## 3. 前端工具栏

`.toolbar` 高 38；左侧历史按钮 40×38，右侧工具项常见 46×38，名称居中。后退/前进依据历史决定 enabled；刷新有未保存检查，不能直接丢弃草稿。

右侧可能显示应用探索、更新、离线、重启/警告、OLED、profile 等入口，受应用/设备/状态控制。不是所有图标始终可见。

## 4. 前端导航和 Profile

| 属性 | 基线 |
|---|---|
| .nav-tabs | flex，align-items:center，width:100%，position:relative，z-index:106 |
| 高度 / 背景 | min-height:48，#222，底边 2px solid #000 |
| 两侧区域 | profile-wrapper / right 分担伸缩空间；普通布局约各 25% |
| nav 文字 | Roboto 12px、uppercase、默认 #999 |
| 间距 | margin-right:20；padding:7px 10px；line-height:14 |
| active | 圆角 14，#44d62c 背景、#111 前景 |
| 更多 | dots3，hover #2d2d2d，active 绿色 |

中间导航按内容居中。最后一项 margin、profile 和状态区宽度、隐藏项都会影响位置；不能简单把三块设成等宽。宽度不足时应保留更多菜单入口，不是换行后挤压内容。

普通页顺序见 [页面索引](README.md)。HELP 虽在根 navs 中，但被 header 过滤到右侧。

当前帮助图标进入实际[设备帮助页](11-help.md)，保持当前设备身份及普通导航；Profile在Help/Power/Calibration隐藏（653无普通Calibration页）。帮助的产品外链、序列号和固件信息不能用统一支持站跳转代替。

Profile 是当前设备实例的配置选择/操作入口，包含当前项、列表及管理操作。不能点击一次就切到“下一个”。切 profile 后映射、DPI、灯光、EQ 应切换到该配置的数据。草稿保护属于实际事件链的一部分。

当前本地实现已补齐新增、复制、原位重命名、删除、182/653 重置，以及本项目格式的导入导出和关联程序列表。原菜单 `jN` 先列新增/导入，后列关联游戏，再列重命名/复制/导出，最后为按产品过滤的重置/删除。重命名为原选择框位置的 32 单元输入；删除最后一项禁用，确认之后才执行，仍经过未保存映射保护。本地导入导出明确使用 `.razer-ui-profile.json`；关联游戏仅保存每个 Profile 的 `.exe` 列表。原 Synapse 格式、游戏运行时自动切换和板载服务尚未接通。以上为历史批次记录；当前覆盖口径见[产品统计](../re/native-product-coverage.md)，尚未进行运行验收。

`renderProfileBar`、`renderKeyMapBar`、`hasDynamicMode && activeDynamicMode` 控制附属内容。教程、电池、THX/重启提示和 linked games 也是条件状态。disabled / showLinkedGames 同时限制交互与透明度，不能只把控件画灰。

## 5. 内容、卡片和缩放

`.main-container` 为 absolute 全尺寸纵向 flex，min-width:600。`.body-wrapper` 占剩余空间，默认 padding 为 10px 20px 20px；scrollable 才使用 overflow:auto，no-scroll / custom-scrollable 有独立逻辑。

body-widgets 常见 max-width:1240。`.widget-col` 是纵向列容器，每列内部继续堆卡片，常用宽度 600；widget padding 30px 40px、圆角 5。部分 margin:0 30px 在 max-width:1279 媒体查询内，不能全局应用。

不同页面保留各自契约：

- Dashboard 使用可重排分组和卡片。
- Customize 上半部是设备图与输入命中区，鼠标和键盘高度不同。
- Sound 左列音量/系统属性、右列 EQ；Mic 的 eqBox 才有特殊宽度。
- dot-bg 来自 CSS gradients，格距 22；特殊 edition 背景只在对应分支使用。

原始 CSS px 是比较基线。GPUI Kit 应集中使用主题和缩放规则，参见 [实现映射](../re/05-gpui-kit-mapping.md)。

## 6. 本页资源

位于 `.ref/host-4.0.827/electron/assets/image/tab/`：

| 用途 | 文件 |
|---|---|
| 最小化 | [minimize.svg](../../.ref/host-4.0.827/electron/assets/image/tab/minimize.svg) |
| 最大化 / 还原 | [maximize.svg](../../.ref/host-4.0.827/electron/assets/image/tab/maximize.svg)、[restore.svg](../../.ref/host-4.0.827/electron/assets/image/tab/restore.svg) |
| 窗口关闭相关 | [close.svg](../../.ref/host-4.0.827/electron/assets/image/tab/close.svg)、[close-hover.svg](../../.ref/host-4.0.827/electron/assets/image/tab/close-hover.svg)、[close-original.svg](../../.ref/host-4.0.827/electron/assets/image/tab/close-original.svg) |
| 活动标签关闭 | [close_active_tab.svg](../../.ref/host-4.0.827/electron/assets/image/tab/close_active_tab.svg)、[close_active_tab_hover.svg](../../.ref/host-4.0.827/electron/assets/image/tab/close_active_tab_hover.svg)、[close_pressed.svg](../../.ref/host-4.0.827/electron/assets/image/tab/close_pressed.svg) |
| 历史 | [left-arrow.svg](../../.ref/host-4.0.827/electron/assets/image/tab/left-arrow.svg)、[right-arrow.svg](../../.ref/host-4.0.827/electron/assets/image/tab/right-arrow.svg)，同目录另有 active / disabled / hover |
| 品牌 / 加载 | [razer_wordmark.svg](../../.ref/host-4.0.827/electron/assets/image/tab/razer_wordmark.svg)、[progress_spinner.svg](../../.ref/host-4.0.827/electron/assets/image/tab/progress_spinner.svg) |

当前 Rust 的 `assets/window-*.svg` 应与原图内容核对，不能按名称判定相同。Roboto 字体和实际接入关系见 [资源索引](../re/04-resource-index.md)。

## 7. 重构前基线与验收

`[RUST 基线]` [shell.rs](../../src/shell.rs) 已具备四层外观雏形；[main.rs](../../src/main.rs) 已初始化 GPUI Kit、主题、open_window。不能再把“没有 Root/主题初始化”当作缺陷。

| 位置 | 差异 |
|---|---|
| tabs() | 固定 Home + 产品页 + Setting，且产品页列表不符 |
| host_tab() | 外层标签没有切换 handler |
| profile_region() | 点击轮换 profile，缺原版选择/管理 |
| 帮助图标 | 无实际 handler |
| content() | 统一滚动/padding，未覆盖各页面契约 |
| 导航空间 | 缺完整更多菜单和缩放/窄宽行为 |

`[建议]` 分开应用窗口身份、route、设备实例与 profile。验收覆盖标签激活/关闭、原生按钮、历史边界、草稿切页、profile、更多菜单、中文/英文与高 DPI。

## 宿主页签修复（2026-10-02）

[实现](../../src/shell/host_tabs.rs) 已将可关闭页签与设备数据分离，补齐设备页签关闭、Dashboard 重开、相邻选择、历史清理、关闭栈、Ctrl+W／Ctrl+Shift+T／Ctrl+Tab／Ctrl+Shift+Tab 和中键操作。主应用页签不显示关闭按钮。隐藏设备工作区继续保留草稿，保存／退出检查仍覆盖它。

按原 TabUI 修正 34px 页签内容盒、24px 关闭目标、活动渐变、悬停与按下 SVG、产品 favicon 和系统按钮；剩余页签位置过渡为 200ms ease。详细来源、编译结果和未覆盖项见[滚动与动画复核](../re/18-scroll-and-motion-audit.md)。

宿主页签已继续补齐鼠标拖动、Ctrl+Shift+PageUp／PageDown 换位，以及本地顺序保存／恢复；鼠标越过相邻项的 1/3 宽度时换位。主标题和窗口空白宽度采用原版条件与公式。平滑滚动的浏览器时序仍待对齐，详见上述复核记录。
