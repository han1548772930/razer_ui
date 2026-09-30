# Razer Synapse 4 UI 复刻规范

> 本文件是实现约束，不是当前 Rust 实现的状态报告。唯一的一手依据是仓库内的 `.ref/` 原始 Electron、主前端、产品模块、CSS、JS、manifest 和 locale。`docs/screens/*.md` 只是按产品/界面拆分的审计索引，不能反过来覆盖 `.ref`。
>
> 快照日期：2026-09-30

## 1. 证据优先级

同一结论出现冲突时，按以下顺序处理：

1. `.ref/synapse-asar/electron/` 的 Electron 外壳 JS/CSS。
2. 目标产品的 `.ref/devices/<module>/static/js/`、`static/css/`、`manifest.json`。
3. `.ref/frontend/` 主前端 JS/CSS、`index.html` 和 manifest。
4. `.ref/frontend/locales/` 与产品模块 locale。
5. `docs/screens/*.md` 的整理结果。

以下内容不能单独证明页面存在或设备支持：

- locale 中出现的 `TAB_*`、功能名或 tooltip；
- 主前端公共 bundle 中存在的组件；
- 其他产品的 CSS class；
- 当前 Rust 的 enum、占位页面或默认路由；
- 图片、截图和 OCR 结果。

每条实现结论都必须能回溯到源码路径，并注明它是 `[HTML]`、`[MANIFEST]`、`[JS]`、`[CSS]` 或 `[I18N]` 证据。没有直接证据的内容写成 `未证实`，不得写成“支持”。

## 2. 原版外层架构

原版不是一个单层标题栏。它由 Electron 外壳、产品网页和页面内容组成：

```text
窗口根
├─ Electron .etabs-tabgroup       42px；可见时显示外层 tab、拖动区、3 个系统按钮
└─ 产品模块 .main-container       #222；产品网页自己的 48px .nav-tabs
   ├─ .nav-tabs                   profile / 页面导航 / 右侧状态
   └─ #body-wrapper               10px 20px 20px；页面滚动内容
      ├─ .widget-prod             产品图片区（按模块需要出现）
      └─ .body-widgets            600px widget 的居中换行容器
```

### 2.1 Electron 外层标签栏

证据：`.ref/synapse-asar/electron/index.css`、`.ref/synapse-asar/electron/components/Tab/TabUI.js`。

| 项目 | 原始规则 |
|---|---|
| 根类 | `.etabs-tabgroup`；默认 `display:none`，带 `.visible` 时 `display:flex` |
| 高度 | `42px` |
| 背景 | `#000` |
| 标签区域 | `.etabs-tabs` 高 `42px`，宽 `calc(100vw - 365px)`，默认 `-webkit-app-region:drag` |
| tab 尺寸 | `min-width:90px`、`max-width:240px`、底部对齐、圆角 `5px 5px 0 0` |
| tab 内容 | 图标约 `20px`、标题最大 `200px`、标题 `12px`、非活动文字 `#999`、活动文字 `#ccc` |
| 活动 tab | 外层绿色渐变，内容区域 `#222`；悬停/按下有独立渐变状态 |
| 系统按钮容器 | `.etabs-window-control-btns`，绝对定位到右侧，铺满 `42px` 高 |
| 系统按钮 | 依次为最小化、最大化/还原、关闭；每个命中区 `48px × 42px`，`no-drag` |
| 按钮 hover | 外围背景 `#222`，不能改变为绿色 |

窗口按钮的 SVG 只能使用 `.ref/synapse-asar/electron/assets/image/tab/` 中的原始资源。SVG 图标尺寸不等于命中区尺寸：

| 按钮 | 默认资源 | 还原/悬停资源 |
|---|---|---|
| 最小化 | `minimize.svg` | 由按钮状态控制 |
| 最大化 | `maximize.svg` | 最大化后切换 `restore.svg` |
| 关闭 | `close.svg` | hover/pressed 由 CSS 资源切换 |

拖动规则：空白 tab 区域可拖动；tab、系统按钮和其他交互控件必须 `no-drag`。最大化时由 `TabUI.js` 通过 `checkMaximizeWindow()` 切换图标；系统按钮事件分别发送 `minimize`、`maximize` 和 `handleCloseBtn`。不能把按钮渲染到产品网页 `.nav-tabs` 内，也不能在其上方再增加一条 GPUI `TitleBar`。

### 2.2 产品网页顶栏

证据：各产品模块的 `main.js/main.css` 以及主前端 header 组件。`.nav-tabs` 与 Electron 外层是两个不同 DOM 层。

| 区域 | 原始结构/规则 |
|---|---|
| 根 | `.nav-tabs`：`display:flex`、`align-items:center`、`min-height:48px`、背景 `#222`、`border-bottom:2px solid #000` |
| 左侧 | `.profile-wrapper`；profile、keymap 或产品相关 profile 控件，按 props/能力出现 |
| 中间 | `.navs-wrapper`；`visibleNavs.map(...)` 渲染产品实际页面导航，空间不足时可出现 `dots3` 溢出菜单 |
| 右侧 | `.right`；电池、帮助、警告、教程/扩展等按条件出现 |
| 页面 tab | `.nav`：圆角 `14px`、`padding:7px 10px`、右间距 `20px`、字号 `12px`、文字大写转换 |
| tab 颜色 | 默认 `#999`；hover `#2d2d2d/#ccc`；active `#44d62c/#111`；按下 `#3cbf27/#111`；disabled `opacity:.3` |

产品网页顶栏不负责最小化、最大化、关闭和窗口拖动。最后一个导航项不能保留多余右间距；不可见/不支持的页面不能通过固定 tab 数量补齐。

### 2.3 页面内容边界

产品模块的公共 CSS 证据为：

| 选择器 | 规则 |
|---|---|
| `html, body` | `width/height:100%`、`min-height:720px`、`max-width:1920px`、`overflow:hidden`、`font:16px Roboto,sans-serif`、文字 `#ccc`、背景 `#222`、`user-select:none` |
| `.main-container` | `position:absolute`、纵向 flex、`width/height:100%`、`min-width:600px`、背景 `#222` |
| `#body-wrapper` / `.body-wrapper` | `flex:1 1`、`min-width:600px`、`padding:10px 20px 20px`；滚动由源码状态类控制 |
| `.body-widgets` | 横向 flex、`flex-wrap:wrap`、居中、`max-width:1240px` |
| `.widget` | 宽 `600px`、上下外边距 `10px`、内边距 `30px 40px`、背景 `#111`、圆角 `5px`、正文 `14px` |
| `.widget-col` | 纵向 flex、宽 `600px` |
| `.widget-prod` | 高 `250px`、宽 `100%`、最小 `1024px`、最大 `1220px`、上下外边距 `10px` |

这些是产品模块反复使用的公共外壳，不代表每个页面都必然同时拥有产品图片区或 widget。模块自己的 CSS/JS 对公共规则有优先级。

## 3. 视觉令牌与通用控件

下表只收录在产品 CSS 中反复出现且可作为语义令牌的值；具体控件仍以目标模块 CSS 为准。

| 角色 | 值 | 典型用途 |
|---|---|---|
| 页面/顶栏背景 | `#222` | `.main-container`、`.nav-tabs`、内容背景 |
| 卡片/输入底色 | `#111` | `.widget`、输入区域 |
| 主色 | `#44d62c` | active、主按钮、激活边框 |
| 导航按下色 | `#3cbf27` | `.nav:active` |
| 主文字 | `#ccc` | body、标签、说明正文 |
| 次文字 | `#999` | inactive、辅助信息 |
| 通用边框 | `#5d5d5d` | 卡片/控件边框 |
| 下拉边框 | `#515151` | `.s3-dropdown` |
| 次按钮 | `#707070` | secondary button |
| hover 底色 | `#2d2d2d` | nav、控件 hover |
| 警告橙 | `#fd8611` | 警告/提示 |
| 危险色 | `#fd4949`、`#c8323c` | 错误、删除、危险状态 |
| 黑色 | `#000` | 按钮文字、窗口栏、分隔线 |

已由公共 CSS 直接证明的控件约束：

- `.thx-btn`：绿色 `#44d62c`、黑字、圆角 `3px`、约 `.5rem 1.5rem` 内边距、大写；hover `opacity:.8`、active `opacity:.6`、disabled `opacity:.3`；secondary 为 `#707070/#fff`。
- `.s3-dropdown`：约 `27px` 高、`1px solid #515151`；展开/hover 时边框和文字变绿；选项列表黑底，约 `25px` 行高，最大高度 `180px`。
- switch、slider、stepper、dialog 的尺寸和状态不能仅套用这组公共令牌；必须查目标模块的 CSS/JS。
- 原版没有证据支持一个全局统一的 `PageHeader`、面包屑或“更改会立即保存”横幅。

## 4. 产品模块与 productId 条件

`manifest.json` 的产品翻译键包含 edition/variant，不能把 `<productId>_<edition>` 当成新的产品页面，也不能仅依据 `deviceName` 猜测页面能力。

| 模块目录 | manifest 名称 | 主 productId/variant 证据 | 已审计页面顺序/范围 |
|---|---|---|---|
| `.ref/devices/182` | Razer DeathAdder V3 Pro | `182_0`、`182_128`、`182_129`、`182_130`、`182_131`、`182_132` | 已审计：自定义、性能、正在配对、校准；电源/滚动只能在目标 bundle 的可见 tab 条件确认后实现 |
| `.ref/devices/653` | Blackwidow V4 Pro | `653_0`、`653_128`、`653_129`、`653_130` | 已审计：自定义、性能、灯光；电源/滚动只能在目标 bundle 的可见 tab 条件确认后实现；653 性能不是鼠标 DPI/抬升距离页面 |
| `.ref/devices/777` | RAZER KRAKEN BT SANRIO LIMITED EDITION | `777_0`，manifest 还含 `1328_*`、`1332_*` 变体键 | 已审计：声音、麦克风；自定义、灯光、校准、电源是否可见必须由该变体的 bundle 条件确认，不能仅凭公共 tab 名称实现 |

路由条件必须至少保留：

```text
发现设备
  → module / productId / edition
  → manifest translation + 产品能力
  → 产品 bundle 的 ordered visible tabs
  → 对应页面组件
```

`182`、`653`、`777` 的共同 `TAB_*` 字符串不等于三者拥有相同页面。`653` 的源代码明确声明 `category:"KEYBOARD"`、`isOBMDevice:true`、`OBMSlots:4`；其性能能力包含 polling rate，但不能因此增加鼠标 DPI、Lift-off 或表面校准。`777` 的 manifest 变体键也不能无条件压成 `productId=777`。

## 5. 页面与模块清单

### 5.1 主前端页面

证据和页面关系见 `docs/screens/00-app-shell.md`、`docs/screens/10-main-frontend-pages.md`。

| 页面/模块 | 允许的结论 | 不允许的结论 |
|---|---|---|
| Dashboard / `TAB_HOME` | 主前端设备分组、设备卡、空/扫描/错误/入口状态；`.box-group` 可折叠，设备项顺序由 JS 管理 | 把设备卡套成产品页 `600px widget`；固定写“已连接” |
| 应用设置 / `TAB_SETTING` | `.main-setting`、约 `180px` 的侧导航和右侧设置内容；它是应用级页面 | 当作设备模块页面或产品 tab |
| Battery / header 状态 | 仅在主前端/产品 props 声明时显示电池、充电、低电量摘要 | 因为 locale 有 `TAB_BATTERY` 就给所有设备增加独立页 |
| Audio / Gaming / Display / Haptics / Demo 等 | 主前端存在组件、词汇或扩展入口的证据 | 在没有目标产品模块路由的情况下，伪造产品页面和完整控件 |

Dashboard 的设备卡 CSS 不是公共 `.widget`：原始卡片约 `290px × 220px`、`padding:10px`，分组和状态由主前端 JS 驱动。设备图、类别占位图、连接/扫描/失败状态和入口操作必须按真实字段渲染。

### 5.2 182：鼠标模块

详细页面契约：

- `docs/screens/01-customize.md`：`TAB_CUSTOMIZE`、`config-wrapper`、`config-block`、鼠标映射、Canvas 连线、左右按键索引和 `ButtonPanelComponent`。
- `docs/screens/02-performance.md`：DPI、XY、polling rate、Lift-off/Smart Tracking 的真实分区和条件。
- `docs/screens/03-pairing.md`：dongle、设备卡、扫描、确认、解除配对、skeleton 和失败状态。
- `docs/screens/04-calibration.md`：鼠标表面卡片、添加表面、校准中/完成/错误/重试。

实现边界：

- 182 自定义不是通用宏列表卡片；映射编辑器由真实按键区域和 `ButtonPanelComponent` 组成。
- 182 性能页只显示鼠标 bundle 声明的 DPI、XY、轮询率、抬升/追踪条件，不能套用 653 键盘能力。
- 配对页的候选、扫描和确认必须由真实设备状态驱动；不能点击后直接显示成功。
- 校准页是鼠标表面校准，不得把共享 locale 中的手柄/耳机步骤写入页面。

### 5.3 653：键盘模块

653 的页面细节由 `.ref/devices/653/static/js/main.7b71cce5.js`、其 chunk/CSS 和 `docs/screens/07-lighting.md` 中的 653 小节共同索引。实现必须遵守：

- Customize 使用可点击的 keyboard SVG、`config-wrapper`、`config-block`、`key-config`、`keymap-action`；键盘布局不是静态产品图片。
- 键盘 SVG 的源 CSS 约束为最大高度 `387px`；`.config-wrapper` 约 `min-width:770px`、`max-width:1220px`。
- Performance 以 polling rate 等 653 明确能力为准；禁止渲染鼠标 DPI、Lift-off、Surface Calibration。
- Lighting 必须区分 `QUICK_EFFECTS` 与 `HARDWARE_QUICK_EFFECTS`，并按 `deviceQuickEffect`、`chroma-sync`、`useHardwareEffect`、`automationSupported` 等条件显示效果和参数。
- Macros 是映射编辑器内部的宏选择/播放方式/重复次数区域，不是独立的“宏列表 + 录制卡片”页面；不可用时降低透明度并禁用。

### 5.4 777：耳机模块

详细页面契约：`docs/screens/08-sound.md`、`docs/screens/09-mic.md`。两页必须按 `.ref/devices/777` 自己的组件树实现，不能复用 653 键盘或 182 鼠标页面模板。

- Sound 的组件链为 `WM → VM → FM → gM`；EQ 图表范围为 `-5..5 dB`、步长 `1`，使用窄型 slider。
- Mic 的组件链为 `KM → kM → YM → gM`；麦克风 EQ 使用宽型 slider，并带 `tabScale:true` 的布局条件。
- `#eqBox` 源 CSS 约 `width:940px`、`min-height:473px`。
- 只能实现 bundle 中实际出现的输出、EQ、音频模式、麦克风和依赖状态；不能因为 locale 或公共 bundle 中有名称，就添加十段 EQ、Game/Chat mix、sidetone、降噪、采样率、XLR phantom power 等未被目标组件证明的控件。
- `setupStatus === READY` 等初始化状态必须按源逻辑处理；不能用假硬件连接、假保存成功或假设备确认替代。

### 5.5 电源、滚动、灯光及扩展模块

| 模块 | 当前可引用范围 | 约束 |
|---|---|---|
| Power | `docs/screens/05-power.md` 的公共骨架和待复核线索 | 电池/睡眠/充电等控件以及页面是否可见，必须先由目标 product bundle 的能力分支确认；不能对三种设备无条件显示 |
| Scrolling | `docs/screens/06-scrolling.md` 的公共骨架和待复核线索 | stage、顺序、滚动模式以及页面是否可见，必须由目标设备 bundle 声明；不能把滚动页塞给键盘或耳机 |
| Lighting | `docs/screens/07-lighting.md` | 653/777 的灯光路径不同；效果、颜色、速度、方向和 Chroma Studio 入口按 `renderEffect()`、资源安装状态及设备条件显示 |
| EQ/Enhancement/Mixer/Audio | 主前端与 777 目标 bundle 的实际组件 | 仅有 `TAB_*` 或 locale 不足以建立独立页面；缺少目标路由时标记未证实 |
| OLED/Display/Haptics | 主前端/公共 bundle 的扩展能力 | 未找到目标产品模块组件树前不得作为已实现产品页 |

## 6. 状态与交互规则

状态必须来自源代码实际分支或设备服务返回；名称相同不代表行为相同。

```text
unavailable   能力未声明或模块未安装
disconnected  设备/服务断开
idle          可开始操作
loading       扫描、读取、上传或写入中
failed        源代码显示失败或错误路径
cancelled     用户取消操作
```

只有在源代码明确存在对应状态时，才增加 `dirty`、`saving`、`awaiting-device` 或 `saved`。本地文件写入成功不能自动写成硬件已应用；配对、校准、灯光、音频设置和上传都必须保留源代码的确认/失败路径。loading 时阻止重复命令，disabled 时阻止点击，错误必须保留可恢复入口；不能只用 toast 或颜色表达关键状态。

通用交互要求：

- 弹窗、下拉、颜色选择器和确认层必须在页面主轴之上渲染，不改变 widget 排布。
- 关闭 overlay 后恢复触发控件焦点；Escape 只关闭最上层 overlay，不静默丢弃草稿。
- 稳定业务 ID 使用产品/设备数据，不使用数组 index、本地化文字或视觉顺序作为持久化 ID。
- 页面标题、按钮、错误和 tooltip 优先取 `.ref` locale；locale key 只能作为文案证据，不能作为能力证据。

## 7. 文档关系

`docs/screens/` 是本规范的可点击分屏索引，不能替代 `.ref`：

| 文档 | 对应源码审计范围 |
|---|---|
| `docs/screens/00-app-shell.md` | Electron `TabUI.js`、Electron CSS、主前端 header、Dashboard、应用设置 |
| `docs/screens/01-customize.md` | 182 鼠标 Customize |
| `docs/screens/02-performance.md` | 182 鼠标 Performance |
| `docs/screens/03-pairing.md` | 182 鼠标 Pairing |
| `docs/screens/04-calibration.md` | 182 鼠标 Calibration |
| `docs/screens/05-power.md` | Power 公共骨架与当前产品审计结果；使用前仍查目标 bundle |
| `docs/screens/06-scrolling.md` | Scrolling 公共骨架与当前产品审计结果；使用前仍查目标 bundle |
| `docs/screens/07-lighting.md` | 653/777 Lighting 分支、效果和资源条件 |
| `docs/screens/08-sound.md` | 777 Sound、EQ 和音频状态 |
| `docs/screens/09-mic.md` | 777 Mic、麦克风 EQ 和相关状态 |
| `docs/screens/10-main-frontend-pages.md` | 主前端 Dashboard、Setting 及扩展入口的边界 |

分屏文档与本文件冲突时，保留 `.ref` 证据，修改或删除冲突文档；不要为了让文档看起来完整而恢复旧截图、旧 `src/pages` 架构或未证实的功能。

## 8. 实现验收清单

一个页面只有同时满足以下条件，才允许标记为完成：

1. 路由由 `module + productId + edition + capability` 决定，不由设备类别或 locale key 猜测。
2. 外层高度严格区分 `42px` Electron 标签栏和 `48px` 产品 `.nav-tabs`；三个系统按钮是 `48px × 42px` 的独立 `no-drag` 命中区。
3. 页面使用目标模块的 DOM/组件层级、尺寸、颜色、间距、圆角和状态，不套用不存在的公共 `PageHeader`。
4. 182、653、777 的 Customize、Performance、Lighting、Sound、Mic 等页面分别遵守各自 bundle 的条件分支。
5. 每个控件的 disabled、loading、失败、取消和确认行为都有源代码依据；不添加假成功状态。
6. 颜色和 SVG 资源来自 `.ref` CSS/SVG；不要用截图取色或凭印象替换系统按钮图标。
7. `docs/screens/<page>.md`、`.ref` 路径和实现文件可以逐项互相追溯。

## 9. 本次审计删除的过时结论

- 删除“所有设备共享同一套页面顺序”的结论；产品 bundle 才是页面来源。
- 删除把 `TAB_*` locale key 当成设备能力的结论。
- 删除 777 的通用十段 EQ、Game/Chat mix、mic volume、sidetone、降噪、采样率、XLR phantom power 等无目标组件证据的功能承诺。
- 删除把 653 当成鼠标、渲染 DPI/Lift-off/表面校准的结论。
- 删除把 653 宏功能描述成独立宏列表/录制页面的结论。
- 删除把所有页面套成一个统一全宽卡片、统一页面标题或统一顶部系统按钮的结论。
- 删除把当前 Rust 文件、截图或旧页面目录作为 UI 规范来源的结论。
