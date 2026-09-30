# 主前端入口与 Dashboard 规格

> 本文只记录 `.ref/frontend/` 中能够由 JavaScript、CSS、locale 和 manifest 互相印证的主前端事实。它不是产品设备页规格；产品页必须继续以 `.ref/devices/<productId>/` 的页面 bundle 为准。

## 证据范围

| 证据 | 用途 |
|---|---|
| `.ref/frontend/static/js/App.eb32d7cd.chunk.js` | 主前端 `HomePage`、顶层导航、`nav-tabs`、`body-wrapper` 和异步页面入口 |
| `.ref/frontend/static/js/4130.155387bf.chunk.js` | 设备 Dashboard、设备分组、设备卡片、推荐/模块/在线服务入口 |
| `.ref/frontend/static/js/9388.2bec5db3.chunk.js` | Gamer Room 页面、灯光/房间营销卡、设备组及教程入口 |
| `.ref/frontend/static/js/6505.93b828df.chunk.js` | 固件更新页面及固件详情展开内容 |
| `.ref/frontend/static/css/55.a5b041a2.chunk.css` | 公共壳层、导航、Dashboard 分组和通用卡片样式 |
| `.ref/frontend/static/css/4130.6bdf8dd0.chunk.css` | Dashboard 设备卡、扫描/配对/解绑状态样式 |
| `.ref/frontend/static/css/6505.9782778c.chunk.css` | 固件更新列表样式 |
| `.ref/frontend/locales/zh-CN.json`、`trans-zh-CN...chunk.js` | 文案来源；组件通过 locale key 和 `getTextItem` 解析，不应在实现中臆造文案 |
| `.ref/frontend/manifest.json`、`asset-manifest.json` | 应用入口、构建版本、入口 bundle 和 PWA 外壳信息 |

## 1. 公共前端壳层

### 1.1 根容器

源码组件 `MainContainer` 只输出一个 `.main-container`，页面内容作为其子节点注入。CSS 明确规定：

- `.main-container` 为绝对定位、`width:100%`、`height:100%`、`min-width:600px`、纵向 flex 容器。
- 页面背景是 `#222`，不是白色，也不是产品卡片的 `#111`。
- `.body-wrapper` 占用剩余高度，`flex:1`、`height:100%`、`min-width:600px`，默认内边距为 `10px 20px 20px`。
- `#body-wrapper.body-wrapper.scrollable` 使用 `overflow:auto`；`no-scroll` 和 `custom-scrollable` 分支会关闭垂直滚动，不能一律强制显示滚动条。
- `.body-wrapper` 的 `background-edition` 与 `background-edition-gradient` 只在带版本/背景图的入口使用；不能把它们当成所有页面都存在的背景层。

### 1.2 产品顶栏 `.nav-tabs`

这是页面产品导航，不是 Windows/Electron 系统标题栏。Electron 外层标签栏和系统按钮属于 `.ref/synapse-asar/electron/`，不在本文的前端 `.nav-tabs` 内。

实际 CSS：

- `display:flex`、`align-items:center`、`min-height:48px`、`width:100%`、`position:relative`、`z-index:106`。
- 背景 `#222`，底部 `2px solid #000`，默认文字色 `#5d5d5d`。
- `.profile-wrapper` 左侧和 `.right` 右侧分别承担 `flex` 空间；普通布局中两侧约为 `25%`，中间 `.navs-wrapper` 按导航内容宽度布局并居中。
- `.navs-wrapper` 使用 `Roboto, sans-serif`、`font-size:12px`；导航项 `.nav` 为大写文本、`margin-right:20px`、`padding:7px 10px`、`border-radius:14px`、`line-height:14px`、`white-space:nowrap`。
- 普通导航色为 `#999`，激活项背景 `#44d62c`、文字 `#111`；导航项存在 `background-color` 和文字颜色过渡。
- 更多菜单 `.dots3` 为圆角按钮；悬停背景 `#2d2d2d`，激活菜单使用绿色背景/图标。不能把隐藏导航简单渲染成第二行。
- 右侧可能出现帮助、Tutorial、THX 警告、重启警告、电池图标和筛选条；这些元素由 props 和能力状态决定，不是固定控件。
- `disabled` 或 `showLinkedGames` 状态会降低 `.nav-tabs` 透明度并阻止交互；不能仍然显示为可操作的成功状态。

### 1.3 页面状态约束

- 主前端使用 React 异步路由，加载中或加载失败时不能凭空渲染完整产品页。
- `HomePage` 可能在 Dashboard 禁用时叠加全屏半透明层：源码为 `rgba(0,0,0,.5)`，层级 `1000`。
- 帮助、提示、教程和弹窗应在对应组件真正出现时渲染；源码没有证据的“保存成功”“连接成功”“自动完成”状态不得写入页面。

## 2. HomePage 的真实顶层导航

`App.eb32d7cd.chunk.js` 中的 `HomePage` 创建四个导航项，并用 `active_view` 切换异步页面。导航项名称来自 locale 模块导出，不应把变量名误当成最终显示文案。

| 源码导航常量 | 异步加载入口 | 当前可确认内容 |
|---|---|---|
| `Rav` | `8302`、`1031`、`2973`、`4130` | 设备 Dashboard / 设备与模块分组 |
| `BSg` | `8302`、`9388` | Gamer Room 页面 |
| `iwS` | `6505` | 固件更新列表 |
| `fUK` | `2973`、`7282` | 存在独立异步视图，但仅凭当前压缩 bundle 未稳定映射其中文标题和完整布局，标为未验证 |

因此，本文删除以下无源码依据的假设：

- 不存在可由主前端证实的通用 `.main-setting`、`.side-navigation`、`.setting-content` 页面骨架。
- `TAB_SETTING`、`TAB_AUDIO`、`TAB_GAMING`、`TAB_EQ`、`TAB_MIXER`、`TAB_HAPTICS`、`TAB_DEMO` 等名称不能单凭词汇或 locale 导出推导出页面布局。
- 电池、音频、灯光、OLED、EQ、混音、触觉等能力属于具体设备/模块条件；主前端的警告、图标或文案不能替代产品页面证据。

## 3. 设备 Dashboard

### 3.1 页面树

```text
.main-container
├── .nav-tabs
└── #body-wrapper.body-wrapper
    └── .dashboard[.reflow]
        └── .box-group*
            ├── .backdrop-box
            ├── .title
            │   ├── .collapse > .icon
            │   └── .drag-div / .drag-icon
            └── .content
                └── .content-inner
                    └── 设备卡或入口卡
```

`4130.155387bf.chunk.js` 的 Dashboard 通过 store 中的设备、安装模块、分组顺序、卡片顺序和折叠状态构建内容；源码还存在推荐、在线服务、合作伙伴和模块等分组类型。不能将它简化成两个固定的 `600px widget`。

### 3.2 尺寸和布局

- `.dashboard` `margin:0 auto`、`max-width:1220px`、`min-width:620px`、`flex-direction:column`。
- 当进入 reflow 状态时，`.dashboard.reflow` 的 `max-width` 为 `2460px`；这是宽窗口重排分支，不是默认宽度。
- `.box-group` `width:100%`、`margin:10px 0`、`min-height:18px`，组之间不是无间距堆叠。
- `.content-inner` 使用 flex、`gap:20px`、`flex-wrap:wrap`；卡片数量和窗口宽度决定换行。
- 分组背景 `.backdrop-box` 为 `#333`、`border-radius:5px`，默认透明/不可见；展开后最大高度 `2000px`，拖动中使用半透明 `#3333334d` 和 `2px solid #44d62c`。
- 标题 `.title` 使用 `14px`、`#ccc`；折叠图标为 `10px`，展开时旋转；拖拽图标约 `22×19px`，只在 hover/拖拽状态显现。
- 分组内容默认 `max-height:0` 并通过 `transform`、`max-height` 动画展开；不能通过静态 `display:none` 取代所有状态。

### 3.3 设备卡和入口卡

通用 `.box-item` 的源码样式为：

- 背景 `#111`、圆角 `5px`、`box-sizing:border-box`、内边距 `10px 20px 9px`。
- 产品图区域通常为 `250×140px`，`object-fit:contain`；有商店入口的图片高度可为 `120px`。
- 加载图使用 `27×27px` spinner；失败时图片降低透明度并显示重试入口。
- 名称区 `.name-tag` 为纵向 flex；名称 `14px/16px`、大写、居中；edition 名称 `12px/14px`、颜色 `#707070`；状态文字 `12px/14px`，可为 `#44d62c`、`#ccc`、`#707070`、`#fd8611` 或取消/错误红色。
- 禁用卡使用 `opacity:.3` 和 `pointer-events:none`，不能只降低文字颜色而保留点击。
- 电池图标/电量位于卡片右上角，低电量使用专用 warning 图标；不是固定显示一个百分比文本。

`4130` 中的双链设备卡另有明确尺寸：`.duallink-device-content .box-item` 为 `250×210px`，`.box-inner` 填满卡片并纵向布局。扫描、绑定、解绑和错误状态使用不同的文字颜色及操作区。

### 3.4 Dashboard 分组内容

源码可确认的分组/入口类型包括：

- `devices`：设备卡和设备连接/安装状态。
- `recommendation`：推荐入口或产品卡；不能与已安装设备卡混用。
- `module`：模块安装/打开相关入口。
- `onlineService`、`partnerDeals`：在线服务和合作伙伴入口。
- `syn2`、`window10`、`inDevelopment`、`noDevice`、`xboxHeadset`、`xboxController`：源码中的特殊组/状态标识。

卡片行为可能包括打开设备 UI、打开外部链接、安装/升级、重试、显示 tooltip 或展开详情。实际是否出现由 store 数据、安装状态、设备能力和 locale 决定。

## 4. Gamer Room 页面

Gamer Room 不是通用“设置页”。`9388.2bec5db3.chunk.js` 导出 `GamerRoom`，其页面包含：

- `#gamerRoom` 根节点。
- 可关闭的 `gr-banner`，包含营销内容、文本、链接和 Gamer Room Tutorial 入口。
- `dashboard` 设备组，复用设备分组模型，但设备卡使用 `wrapper-box-item`、`.box-item`、`.gr-device-img`、`.gr-device-name`、`.gr-device-status` 和连接按钮。
- 设备卡加载中会显示 skeleton/状态文本；安装完成后才显示对应连接弹出层。
- 页面会按窗口宽度计算列数，并从设备组中排除特定产品后判断是否移除营销 banner；不能固定渲染一张 Gamer Room 横幅。

CSS/组件明确存在 `gr-marketing`、`gr-marketing__image`、`gr-marketing__name`、`gr-marketing__description`、`gr-banner-content`、`.popup` 等结构。教程由状态控制，关闭后不代表设备或房间设置已保存。

## 5. 固件更新页面

`6505.93b828df.chunk.js` 的页面使用 `.items` 列表，不是侧边设置页，也不是产品页 widget 网格。

### 5.1 结构

```text
.items
├── .item-header
│   ├── .header-title
│   └── .header-link（可选）
└── .item.firmware*
    ├── .item-main-content
    │   ├── .item-icon
    │   ├── .item-name
    │   ├── .to-left
    │   └── .item-action
    └── .firmwareDescription（展开时）
        ├── .leftContent
        └── .rightContent
```

### 5.2 样式和交互

- `.items` 宽 `1220px`、纵向 flex、水平居中、底部间距 `40px`。
- `.header-title` 使用 `RazerF5`、`24px`、绿色 `#44d62c`、大写；链接使用 `Roboto`、`14px`、`#ccc`，hover 绿色。
- `.item-main-content` 高 `80px`、背景 `#111`、水平内边距 `0 30px 0 20px`；设备图标 `40×40px`。
- 项目名使用 `16px`、`#ccc`，固定 flex 基础宽度 `500px`，过长时省略；信息文字使用 `14px`、`#707070`。
- 操作按钮位于右侧，最小宽度 `90px`、高度 `27px`；维护中或不可用时显示 disabled/警告，而不是可点击的假按钮。
- 展开详情使用 `#2d2d2d` 背景和 `20px` 内边距；版本、日期、大小与 release notes 分区排列。
- 更新警告使用 `#fd8611`；release notes 分类标签使用 `#28aadc`、`#8b7add`、`#44d62c` 等源码颜色。
- 更新项可能有 warning、最近更新后断开、详情展开、外部帮助链接和固件升级动作；这些状态由固件数据决定。

## 6. 未验证与禁止推断

以下内容在当前 `.ref/frontend` 证据中不能作为主前端页面规格：

- 独立的应用设置左侧导航，以及固定的 `.main-setting`、`.side-navigation`、`.setting-content` 布局。
- 主前端统一提供的音频、EQ、混音、OLED、触觉、Gaming Mode 或电池详细设置页。
- 固定的“设备卡约 `290×220px`”；通用卡片图像区实际为 `250×140px`，双链卡另有 `250×210px`。
- 所有页面共用 `600px` 两列 widget、所有页面都有保存按钮、所有页面都有预览区。
- 仅由 `TAB_*` 文案、locale key、截图或当前 Rust 页面名称推导出的组件顺序和能力。

当某个入口只有异步 chunk 编号而没有可读组件/样式证据时，文档必须保留“未验证”，实现也必须隐藏该入口或使用真实加载状态，不能用通用占位页面冒充原版。

## 7. Manifest 与入口边界

`.ref/frontend/manifest.json` 只证明这是 `dashboard` 应用、`display: standalone`、`theme_color:#000000`、`background_color:#ffffff`，并包含构建版本和资源清单；它不能证明应用内页面使用白色背景。应用内背景仍以 `.main-container` 的 `#222` CSS 为准。

`.ref/frontend/asset-manifest.json` 将 `main.js` 指向 `static/js/main.7897a4cf.js`，并列出 `App.js`、`App.css` 及上述异步 chunk。页面实现应沿用这些入口关系，不应引用已经删除的 `src/pages/...` 路径或自行发明主前端路由。
