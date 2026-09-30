# Razer Synapse 4 UI 复刻规范

> 本文是本项目唯一的 UI 规格来源。内容来自 `.ref/` 中保存的雷蛇 Synapse 4 主前端、产品模块、语言包、CSS 和 Electron 桥接代码；不以当前 Rust 实现为规范，也不记录当前实现状态。
>
> 快照日期：2026-09-30  
> 目标：使用 Rust/GPUI 复刻原版的信息架构、功能模型、布局、样式与交互，而不是继续堆叠占位页面。

## 1. 原版结构

Synapse 4 是 Electron 外壳 + 主前端 + 产品模块 + 原生设备桥：

```text
主应用窗口
├── Header / 顶栏
├── DeviceList / 设备列表
├── Content / Dashboard、应用设置、设备入口
└── IotPopupRoot / 全局弹窗根

独立设备窗口
├── .nav-tabs / profile + 产品页面标签 + 右侧状态
└── #body-wrapper
    ├── .widget-prod / 产品图片区
    └── .body-widgets / 产品功能卡片
```

关键结论：

- Dashboard、应用设置、产品页面是三个不同层级。
- 产品页面由 `products/<productId>/ui/` 独立模块提供。
- 设备页没有通用左侧栏；应用设置页才使用固定左侧导航。
- 页面顺序和控件集合由产品模块决定，不能按设备类别硬编码一套页面。
- `TAB_*` 语言 key 只能说明存在文案，不能说明某台设备支持该页面。

### 1.1 证据等级

| 标记 | 来源 | 可确认内容 |
|---|---|---|
| `[HTML]` | `.ref/frontend/index.html`、设备模块 `index.html` | 入口、依赖、版本、启动结构 |
| `[MANIFEST]` | `asset-manifest.json`、设备 `manifest.json` | 分块、模块、产品和资源 |
| `[JS]` | `static/js/`、`.ref/synapse-asar/electron/` | 页面组合、事件、IPC、动作 |
| `[CSS]` | `static/css/`、设备 CSS | 尺寸、颜色、布局和状态 |
| `[I18N]` | `.ref/frontend/locales/` | 标题、控件、提示、错误文案 |

具体产品模块的 `[JS]/[CSS]` 优先于主前端，语言包优先于命名推测。

## 2. 产品能力模型

每个设备模块必须转换为一个能力描述：

```text
DeviceDescriptor
├── product_id / edition / localized_name
├── ordered_pages: PageId[]
├── feature_flags
├── mapping_editors
├── hardware_protocols
└── product_assets
```

已抓取样本：

| 产品 | ID | 类型 | 页面顺序 |
|---|---:|---|---|
| Razer DeathAdder V3 Pro | `182` | 鼠标 | 自定义、性能、正在配对、校准、电源、滚动 |
| BlackWidow V4 Pro | `653` | 键盘 | 自定义、性能、灯光、电源、滚动 |
| RAZER KRAKEN BT SANRIO LIMITED EDITION | `777` | 耳机 | 自定义、灯光、校准、电源、声音、麦克风 |

页面构建流程必须是：

```text
发现设备 → productId/edition → manifest → 能力 → 有序 PageId → 页面控件
```

## 3. 全局布局

### 3.1 基础 CSS

| 选择器 | 规则 |
|---|---|
| `html, body` | `width:100%; height:100%; min-height:720px; max-width:1920px; margin:0; overflow:hidden` |
| `html, body` | `font-family:Roboto,sans-serif; font-size:16px; color:#ccc; background:#222` |
| `body` | `user-select:none` |
| `.main-container` | `position:absolute; display:flex; flex-direction:column; width:100%; height:100%; min-width:600px; background:#222` |
| `.body-wrapper` | `flex:1 1; width:100%; height:100%; min-width:600px; padding:10px 20px 20px` |
| `.body-widgets` | `display:flex; flex-wrap:wrap; justify-content:center; max-width:1240px; margin:auto` |
| `.widget` | `width:600px; min/max-width:600px; margin:10px auto; padding:30px 40px; background:#111; border-radius:5px; font-size:14px` |
| `.widget-col` | `display:flex; flex-direction:column; width:600px; height:fit-content` |
| `.widget-prod` | `height:250px; min-width:1024px; max-width:1220px; width:100%; margin:10px auto` |

### 3.2 产品页几何

```text
.main-container
├── .nav-tabs                 min-height:48px
└── #body-wrapper             padding:10px 20px 20px
    ├── .widget-prod          1024–1220px × 250px（页面需要产品视觉时才出现）
    └── .body-widgets         max-width:1240px，居中换行
        ├── .widget           600px 宽，30px/40px 内边距
        └── .widget-col       600px 宽，内部纵向堆叠
```

原版没有统一的 `PageHeader`、breadcrumb 或「设备名 + 更改会立即保存」横幅；页面标题、说明和 profile bar 是否出现，取决于具体产品模块。600px widget 是原版主要信息表面；不要把所有控件压成一张全宽卡片。产品图片区与功能卡片是两个视觉层。

### 3.3 Dashboard

```text
.dashboard
├── .box-group                可折叠分段
│   ├── .backdrop-box
│   └── .box-inner
│       └── .box-item         约 290×220px 设备卡片
└── 更多分段
```

设备卡片可显示产品图、名称、在线/离线/扫描/配对状态、电池、连接方式、固件和入口操作。分段支持折叠与重排；拖拽之外必须有键盘可达的上移/下移命令。

### 3.4 应用设置

`.main-setting` 使用约 `180px` 固定宽度的 `.side-navigation`，右侧为可滚动设置 widget。应用设置不能伪装成某个设备页面。

## 4. 顶栏

```jsx
<div className=nav-tabs>
  <div className=profile-wrapper>profile / keymap</div>
  <div className=navs-wrapper role=tabs>产品页面标签</div>
  <div className=right role=tablist>电池 / 帮助 / 状态</div>
</div>
```

| 区域 | CSS | 内容 |
|---|---|---|
| 左 | `flex:1 0 25%` | profile、动态模式、keymap |
| 中 | `flex:1 0 max-content; justify-content:center; font-size:12px` | 页面导航 |
| 右 | `flex:1 1 25%` | 电池、帮助、警告、工具 |

原版有两层但职责不同：Electron 外层 `.etabs-tabgroup` 高 `42px`、背景 `#000`，右侧三个系统按钮各 `48px`；产品网页 `.nav-tabs` 再高 `48px`、背景 `#222`、下边框 `2px solid #000`。系统按钮不能塞进产品网页的 profile/nav/right 区域。Rust/GPUI 不得再渲染额外的 `34px TitleBar`，而应直接实现这条 `42px` 外层栏。

### 4.1 导航胶囊

```css
.nav { border-radius:14px; color:#999; line-height:14px; margin-right:20px;
       padding:7px 10px; text-align:center; text-transform:uppercase;
       transition:background-color .3s, color .1s; white-space:nowrap; }
.nav:hover    { background:#2d2d2d; color:#ccc; }
.nav:active   { background:#3cbf27; color:#111; }
.nav.active   { background:#44d62c; color:#111; }
.nav.disabled { opacity:.3; pointer-events:none; }
```

最后一项不保留多余右边距；选中项 hover 不能被普通 hover 覆盖。

### 4.2 顶栏功能

左侧支持 profile 选择、新建、复制、删除、重命名、自动/手动切换和程序关联提示。右侧按能力显示电池、充电、低电量、帮助、立体声/更新/重启警告和过滤工具；没有能力时不显示假警告。窗口控制不是原版网页 DOM 的内容，但在无边框 Rust 窗口中必须作为同一行的系统控制命中区实现；导航胶囊、profile 控件和帮助按钮不能覆盖拖动命中区。

## 5. 视觉令牌

原始 CSS 值应集中进入主题，页面代码只使用语义令牌。

| 令牌 | 值 | 用途 |
|---|---|---|
| `background` | `#222` | 页面和顶栏 |
| `surface` | `#111` | widget、输入区 |
| `primary` | `#44d62c` | 选中、激活、主按钮 |
| `primary-active` | `#3cbf27` | 导航按下 |
| `foreground` | `#ccc` | 主文字 |
| `muted` | `#999` | 次要文字 |
| `border` | `#5d5d5d` | 通用边框 |
| `dropdown-border` | `#515151` | 下拉边框 |
| `secondary` | `#707070` | 次要按钮 |
| `hover-surface` | `#2d2d2d` | hover |
| `warning` | `#fd8611` | 警告 |
| `danger` | `#fd4949` / `#c8323c` | 危险和删除 |
| `black` | `#000` | 按钮文字、下边框、遮罩 |

字号固定为：body `16px`、widget 正文 `14px`、标题 `16px`、nav/小按钮 `12px`、说明 `13px`、badge `10px`、设置标题 `18px`、主标题 `20px`。字体为 `Roboto, sans-serif`。

圆角只有三档：普通控件 `3px`、widget `5px`、nav `14px`。这是高密度桌面设置 UI，不是大留白营销页面。

## 6. 通用控件

### 6.1 Button

主按钮：绿色 `#44d62c`、黑字、`3px` 圆角、上下约 `.5rem`、水平约 `1.5rem` 内边距、大写文字；hover `opacity:.8`，active `opacity:.6`，disabled `opacity:.3`。次按钮背景 `#707070`、白字。按钮文案使用动词。

### 6.2 Switch

约 `32×18px`，外壳 `padding:2px`，关闭灰色、开启绿色，滑块约 `14×14px` 且为深色；关闭位置约 `left:1px`，开启约 `left:15px`。hover `opacity:.7`，disabled `opacity:.3` 并禁用指针。

### 6.3 Dropdown

`.s3-dropdown` 高约 `27px`、透明/深色背景、`1px solid #515151`；hover/展开边框与文字变绿。选项列表黑底、同样边框、`max-height:180px`，单项约 `25px`，hover `#ffffff1a`，选中文字 `#44d62c`。枚举值必须使用 dropdown/radio，不要用步进按钮冒充下拉。

### 6.4 Slider

连续值使用 slider，显示当前值、最小/最大值、单位、说明和 hover/pressed/disabled 状态。不要把连续 slider 替换成“◀ 数值 ▶”。

### 6.5 Tooltip / Dialog / Drawer

tooltip 靠近其说明对象，常见最大宽度 `300px`；弹窗由 `IotPopupRoot` 管理；抽屉使用 `config-drawer` 语义；Escape 关闭最上层 overlay 并恢复触发器焦点。危险确认必须写出对象和动词，不使用没有上下文的“确定/取消”。

## 7. 页面功能规范

本节是跨产品的功能总览。可直接照着重建的逐页面规格位于 [`docs/screens/README.md`](screens/README.md)，其中包含每个页面的设备矩阵、卡片顺序、CSS 类名和语言包条目。

### 7.1 自定义 / 按键绑定

页面 key：`TAB_CUSTOMIZE`，部分模块拆出 `TAB_KEY_BINDS`、`TAB_MY_MACROS`。

```text
profile/keymap → Standard/Hypershift → 产品按键布局 → 选中按键
→ 动作类型 → 动作参数编辑器 → 保存/取消/恢复默认
```

动作编辑器包括：`MapDefault`、`MapDisable`、`MapMouse`、`MapKeyboard`、`MapKeyboardCombineMouse`、`MapMacro`、`MapMacroKey`、`MapMedia`、`MapLaunch`、`MapWindows`、`MapText`、`MapSensitivity`、`MapGlobalSensitivity`、`MapSwitchProfile`、`MapGlobalSwitchProfile`、`MapSwitchKeymap`、`MapHyper`、`MapDynamicKeyStroke`、`MapLighting`、`MapBrightness`、`MapBrightnessGlobal`、`MapScrolling`、`MapRoller`、`MapControlKnobFunction`、`MapDialFunction`、`MapAudio`、`MapJoyStick`、`MapControllerV2`、`MapControllerPlaystation`、`MapInterDevice`、`MapPerfect180`、`MapPerfect180GameList`、`MapAILauncher`、`MapAppSpecific`。

标准层和 Hypershift 层分别保存；动作编辑器按类型动态显示专属参数；应用专用动作、AI、音频、灯光、动态键程等不能被简化成普通按键名。

### 7.2 宏

宏模型为 `name + playback mode + steps[]`，每步包含 `key_down/key_up/mouse_down/mouse_up/text`、目标和毫秒延迟。支持新建、重命名、删除、录制、添加/删除/排序步骤、修改事件、循环/按住重复、总时长、预览和恢复。错误时保留编辑草稿。

### 7.3 性能

鼠标：DPI stages、X/Y 灵敏度、polling rate、lift-off/surface distance、smart tracking、asymmetric cut-off、motion sync、angle snapping、加速度、sensitivity clutch、sensitivity matcher、dynamic sensitivity。

键盘：polling rate、gaming mode、Win/Alt+Tab/Alt+F4 锁定、N-key rollover、Snap Tap、Dynamic Keystroke、Actuation。Snap Tap 最多四对按键；Dynamic Keystroke 有四个阶段；Actuation 必须校验主/第二触发点和风险范围。

### 7.4 灯光

支持设备灯光开关、zone、亮度、效果选择、颜色、速度、电池降亮度、Chroma 联动、预览。效果不使用颜色/速度时隐藏对应控件。效果集合来自设备能力，不能给所有设备显示同一组效果。

灯光写入生命周期：`discover → startup/register → configure → event/ack → shutdown/unregister`。失败时保留草稿并显示设备未更新，不能只保存本地配置冒充硬件成功。

### 7.5 电源

显示电池百分比、充电/充满/暂停/低电量、自动关灯、睡眠、低电量阈值、连接方式和充电保护（若支持）。顶栏是摘要，电源页是详情；低电量必须有 warning/danger 语义和解释。

### 7.6 滚动

支持滚动模式、滚轮阻尼/触觉 stages、高分辨率滚动、水平滚动、加速度、滚轮触觉和强度。每个 stage 使用稳定业务 ID，不以数组 index 作为长期身份。

### 7.7 配对

支持连接方式、扫描、发现、连接中、成功、超时、失败、取消、重新配对、解除配对、dongle 类型和多设备限制。状态机为：

```text
idle → scanning → candidate found → pairing → connected
                         └────────→ timeout / failed / cancelled
```

重复点击不能启动并发扫描。

### 7.8 校准

支持表面 profile、创建自定义表面、校准步骤、进度、成功/失败/重试；摇杆校准包含右/下/左/上回中心和顺时针旋转三次；Sensitivity Matcher 要求第二只鼠标，拔出时显示错误并可重启。

### 7.9 声音 / EQ / 增强 / 混音

按能力拆分为 `TAB_AUDIO`、`TAB_SOUND`、`TAB_EQ`、`TAB_ENHANCEMENT`、`TAB_MIXER`。包含播放设备、主音量、声道/平衡、静音、Stereo/Surround/Spatial、EQ 预设和多频段增益、Bass/Voice/Loudness 增强、系统/应用音量、Chat/Game balance 和设备切换。

### 7.10 麦克风

支持输入端口、输入音量、灵敏度、monitoring/sidetone、静音、AI 降噪、high-pass、gain limiter、采样率、录音播放、XLR Phantom Power。麦克风设置向导为：选择输入 → 录制约 10 秒正常语音 → 安静录环境噪声 → 驱动处理 → 预览测试；48V 必须显示风险警告。

### 7.11 OLED / Display / Haptics

OLED/Display 支持内容/动画模式、亮度、空闲超时、刷新/性能模式、动画、图像/文本预览。Haptics 支持总开关、音频到触觉、增益、强度、profile、预览。保存范围必须明确是 profile、设备还是全局。

## 8. 本地化和反馈

- 使用 `.ref/frontend/locales/` 的 key；支持 `de/en/es/fr/ja/kr/pt-BR/ru/zh-CN/zh-TW` 等语言。
- 标题使用名词，按钮使用动词，tooltip 说明对象/作用/限制。
- 区分加载、扫描、离线、配对、写入中、成功、失败、不支持和空状态。
- 设备掉线、超时、忙碌、协议不匹配和部分成功不能全部显示为“无设备”。

## 9. 推荐 Rust 模块边界

```text
src/
├── app/                  启动、窗口、Root、全局 overlay
├── shell/                Dashboard、产品壳、顶栏、导航
├── features/             每个能力拥有自己的 model/view/command/workflow
│   ├── customize/        profile、mapping、macro、Snap Tap
│   ├── performance/      DPI、polling、actuation、gaming mode
│   ├── lighting/         effect、color、preview、Chroma Connect
│   ├── power/            battery、sleep、charging protection
│   ├── scrolling/        stages、haptics、scroll mode
│   ├── pairing/          scan、pair、dongle、connection state
│   ├── calibration/      surface、calibration workflow
│   ├── audio/            volume、EQ、enhancement、mixer、mic
│   └── display/          OLED、preview、haptics
├── device/               descriptor、capability、transport、events
├── persistence/          profile、settings、cache、migration
├── theme/                semantic tokens、typography、spacing
└── i18n/                 locale、格式化、单位
```

页面只发 feature command，不直接拼硬件协议；feature 负责自己的状态、视图和副作用编排；设备能力由 `DeviceDescriptor` 提供；列表使用稳定 ID；side effect 不得发生在 render；原始颜色只存在主题层。完整的 GPUI Kit / Design Guide 约束见 §13。

## 10. 验收清单

- [ ] Dashboard、应用设置、产品页层级正确。
- [ ] 页面顺序来自产品模块，未支持页面不显示。
- [ ] 外层标签栏为 `42px`，三个系统按钮各 `48px`；产品 `.nav-tabs` 为 `48px`，产品页使用 `600px` widget/`1240px` body。
- [ ] widget、产品图、Dashboard 卡片尺寸和间距符合规范。
- [ ] 颜色、字号、圆角、边框和状态使用主题令牌。
- [ ] 自定义页包含 Standard/Hypershift 和动作专属编辑器。
- [ ] 宏、DPI、灯光、电源、滚动、配对、校准、音频、EQ、麦克风、OLED、触觉按能力出现。
- [ ] 异步操作有完整状态机；失败不会伪造成功。
- [ ] Escape、键盘导航、focus、disabled、loading、danger 状态完整。

## 11. 原始证据索引

| 内容 | 路径 |
|---|---|
| 主前端 | `.ref/frontend/` |
| 鼠标模块 | `.ref/devices/182/` |
| 键盘模块 | `.ref/devices/653/` |
| 耳机模块 | `.ref/devices/777/` |
| Electron 壳 | `.ref/synapse-asar/electron/` |
| IPC 契约 | `docs/re/01-ipc-api-surface.md` |
| 灯光动作 | `docs/re/02-lighting-actions.md` |
| 抓取/扫描/CSS 工具 | `.ref/tools/` |

核对命令：

```powershell
rg -n TAB_|Map[A-Z]|rzHardwareEvents .ref/frontend .ref/devices/182 .ref/devices/653 .ref/devices/777
rg -n body-wrapper|body-widgets|widget-prod|s3-dropdown|slider-container|switch|nav-tabs|thx-btn .ref/frontend/static/css .ref/devices/182 .ref/devices/653 .ref/devices/777
```

## 12. 逐页面实现契约

本节是实现时的逐页验收标准。每个页面都必须同时满足：**布局顺序正确、只显示设备支持的能力、控件状态可见、异步结果可解释、键盘和鼠标都能完成主要操作**。CSS 类名、完整尺寸表、设备差异和语言 key 保留在 [`docs/screens/README.md`](screens/README.md) 及其子文档中；本节给出不能遗漏的页面级契约。

### 12.1 共用产品页骨架

所有产品能力页都使用同一窗口外壳和内容宽度约束，但产品模块可以决定是否渲染产品图、profile bar、说明标题和卡片顺序；公共层不能强行插入原版不存在的 breadcrumb：

```text
body/html 100% x 100%
└─ main-container
   ├─ nav-tabs / 顶栏                         48px 高
   └─ body-wrapper
       ├─ 产品图或设备预览                      设备模块决定，通常是 `.widget-prod`
      └─ body-widgets                         1240px 最大宽度
         ├─ widget / widget-prod               600px 常规卡片
         └─ 底部留白                            24px
```

- 页面背景使用主题的 `surface` 语义色；卡片使用 `surface-raised`；边框使用 `border-subtle`，禁止在 GPUI 调用点直接散落原始十六进制颜色。原版的 `#222/#111/#5d5d5d` 只能在主题映射层出现。
- 卡片圆角以原始 CSS 的 `5px` 为准，控件圆角为 `3px`，导航胶囊为 `14px`；不能用一套默认圆角覆盖全部界面。
- 页面标题、产品图、状态摘要和 profile bar 不是所有页面都必有；只能按对应 product module 的 JSX/CSS 渲染。
- 顶部产品栏固定在内容流中，不能因加载、错误或弹窗改变页面主轴；错误提示、确认弹窗、颜色选择器、profile 菜单属于 root overlay。
- 页面加载时保留骨架和标题；设备掉线时保留最后一次配置摘要，但禁用写入控件并显示明确的 `disconnected` 状态。
- 每个交互控件必须具备默认、hover、focus-visible、pressed/selected、disabled、loading、success、error 八类中适用的状态。

### 12.2 自定义 / 按键绑定（Customize）

**设备范围与顺序**

| 设备 | 页面区块顺序 | 特有能力 |
|---|---|---|
| `182` 鼠标 | profile 栏 → Standard/Hypershift → 鼠标图 → 按键动作编辑器 → 保存反馈 | 鼠标按键、滚轮、灵敏度、跨设备动作 |
| `653` 键盘 | profile 栏 → Standard/Hypershift → 键盘图 → 按键动作编辑器 → Snap Tap / 快速击键 → 保存反馈 | 逐键映射、组合键、Snap Tap、Dynamic Keystroke |
| `777` 耳机 | profile 栏 → profile/设备动作 → 耳机图或说明 → 可用动作 | 不得显示鼠标/键盘专属动作编辑器 |

**布局**

1. 顶部 profile bar 高 26px；左侧新增按钮 36x36，距左 6px、距顶 8px；profile 下拉与按钮间距 10px。
2. profile 选择器宽度 `155px–280px`，黑色背景、1px 边框、14px 全大写标签；删除确认弹窗最小宽度 300px，危险操作使用 danger 色。
3. 设备预览位于页面主视觉区；可点击键必须覆盖在设备图坐标上，并通过稳定业务 ID 标记，不能用数组 index 作为持久化身份。
4. 选中按键后，右侧/下方展示动作类型选择器和动作参数；Standard 与 Hypershift 使用同一布局但拥有独立映射状态。
5. 动作编辑器按动作类型动态显示参数：宏、鼠标、键盘、媒体、启动程序、切换 profile、灵敏度、灯光、滚动、音频、跨设备动作和 AI/应用动作不可被压成一个通用文本输入框。
6. 键盘组合键显示两个以上按键的按下/释放顺序；录制中显示 recording 状态，取消后恢复草稿，保存成功后显示设备确认结果。
7. `653` 额外显示 Snap Tap/快速击键区；其说明、开关、冲突提示和最大配对数量必须在同一卡片内，不能藏在 tooltip 中。

**样式与状态**

- 选中设备键使用主题 accent 背景/描边；不可用键使用 disabled 对比度，并在 tooltip 中说明原因。
- 动作下拉菜单使用分组标题；危险或会覆盖已有配置的动作需要二次确认。
- profile、映射和设备写入必须分别显示 saving、saved、failed；本地保存成功不能冒充硬件写入成功。

**功能验收**

- 新建、切换、重命名、复制、导入、导出、删除和恢复默认 profile 均可完成，删除不能误删当前 profile 而不提示。
- Standard/Hypershift 映射互不覆盖；只允许设备能力清单中的动作类型。
- 快速连续点击保存不会产生并发写入；失败后保留用户草稿并提供重试。
- 页面离开前有未保存改动时，必须给出保存/放弃/取消选择；Escape 只关闭最上层弹窗，不直接丢弃草稿。

来源：[`screens/01-customize.md`](screens/01-customize.md)、`.ref/devices/182/`、`.ref/devices/653/`、`.ref/devices/777/`。

### 12.3 性能（Performance）

**布局与设备差异**

- `182`：DPI stages → X/Y 灵敏度 → polling rate → lift-off/surface distance → asymmetric cut-off → acceleration/motion sync 等鼠标能力。
- `653`：polling rate → gaming mode → Win/Alt+Tab/Alt+F4 锁定 → N-key rollover → Snap Tap/Dynamic Keystroke/Actuation；不显示鼠标 lift-off 控件。
- DPI 卡片中的每个 stage 高 68px，DPI 行约 78px；输入框约 60x26px，stage 颜色按原版 red/green/blue/cyan/yellow 顺序区分。
- polling rate 下拉约 72x27px、展开后最小宽 90px；选中项使用 accent 边框和浅色文字。

**功能**

- DPI stage 支持新增、删除、选中、数值输入和 X/Y 独立控制；删除最后一个 stage 时必须阻止并解释原因。
- 灵敏度、加速度、角度修正、motion sync 和 asymmetric cut-off 只有在设备能力存在时显示；禁用状态仍保留当前值。
- 键盘 gaming mode 逐项控制 Win、Alt+Tab、Alt+F4；Snap Tap 最多四组按键，Dynamic Keystroke 最多四个阶段，Actuation 必须校验第二触发点和风险范围。

**验收**

- 数值输入支持键盘编辑、范围校验、非法值回退和 loading；滑块拖动与输入框修改保持双向同步。
- 所有性能写入都有设备确认或明确失败；断线时不把本地状态更新成成功。

来源：[`screens/02-performance.md`](screens/02-performance.md)。

### 12.4 配对（Pairing）

**布局**

1. 页面上方是连接方式、配对状态和操作说明；中间是扫描/停止/重试主操作；下方是扫描到的设备列表。
2. 设备列表行必须同时显示名称、类型、连接方式、信号/可用状态和当前操作按钮；已连接设备与候选设备使用不同状态样式。
3. 扫描/配对对话框属于 root overlay，弹出后焦点进入主按钮，关闭后焦点回到触发按钮。

**状态机**

```text
idle → scanning → candidate-found → pairing → connected
                    └──────────────→ timeout / failed / cancelled
```

- 重复点击扫描不能启动并发扫描；配对中必须禁用会破坏当前流程的按钮。
- 成功显示设备名称和连接方式；失败显示可操作原因和重试；取消回到 idle；超时不能伪装成 disconnected 以外的成功。
- 支持 HyperPolling/HyperSpeed、dongle 和设备数量限制时，说明文字必须使用能力数据生成，不能硬编码到通用页面。

来源：[`screens/03-pairing.md`](screens/03-pairing.md)。

### 12.5 校准（Calibration）

**布局**

- 校准说明卡片最大宽约 600px，欢迎内容使用 20px/30px 内边距；关闭按钮 36x36px，位于右上角。
- 校准弹窗顶部约 105px，最大高度为视口减去顶部安全区；宽度在宽屏约 800px，小屏降到约 546px，并允许内部滚动。
- 鼠标垫/表面选择使用网格卡片：常规卡片约 240x172px，紧凑卡片约 196x112px；卡片内含缩略图、名称、选中边框和操作入口。

**功能与状态**

1. 进入页面先解释校准目的、适用设备和需要的表面；不直接启动硬件写入。
2. 支持选择已有表面、新增自定义表面、编辑/删除可编辑表面；内置表面不可被删除。
3. 选中表面后启动校准流程，显示阶段、进度、成功、失败、重试和取消；失败保留上一次有效校准。
4. 校准中的关闭、返回和切换设备都要二次确认；Escape 只关闭最上层确认层。
5. `182` 与 `777` 的表面/校准能力按设备清单生成，不支持的设备不能显示空白卡片。

来源：[`screens/04-calibration.md`](screens/04-calibration.md)。

### 12.6 电源（Power）

**布局**

```text
产品标题
├─ battery summary       电量、充电、满电、低电量、连接方式
├─ charging / protection  充电保护与充满行为
├─ timeout selectors      空闲、睡眠、自动关闭
├─ dimming                低电量/空闲时亮度
└─ power-off / status     关机、恢复、写入反馈
```

- 电量摘要行约 46px 高；电量数值 14px；低电量使用 danger 语义色并配文字说明。
- 电量滑块使用独立轨道和百分比标签；电源保存/超时下拉最小宽约 90px，控件背景 `surface-raised`、1px 边框、3px 圆角。
- 选中项使用 accent 边框和深色背景；不支持/不可写状态约 30% opacity，并保留禁用原因 tooltip。
- 低亮度层使用从透明到深色的渐变遮罩，但不能遮挡文本、焦点环或错误提示。

**功能**

- 显示 battery percentage、charging、fully charged、paused、low battery、sleep、idle timeout、auto power-off 和 charging protection。
- `182`、`653`、`777` 的电源区块顺序可以不同，但每个区块必须来自设备能力清单；耳机特有的无线/充电状态不能出现在鼠标页面。
- 自动关闭、睡眠和低电量阈值写入前显示影响范围；写入失败保留原值而不是更新为表单值。

来源：[`screens/05-power.md`](screens/05-power.md)。

### 12.7 滚动（Scrolling）

**布局与样式**

- 滚动 stage 卡片约 230x36px，背景深色、1px 边框；不可用项降低 opacity 但仍显示原因。
- 滚动 stage 高约 68px；stage 颜色遵循性能页相同的主题序列，选中态使用 accent 边框。
- 触觉滚动区域宽约 255px、上下 20px padding；数值显示字号约 28px，便于远距离读取。
- 滚动模式切换器包含约 22x56px 的外壳和约 22x43px 的内部按钮；小于 1279px 时页面允许横向内容滚动而不压缩控件。

**功能**

- 支持 scroll mode、scroll resistance、触觉 stages、高分辨率滚动、横向滚动、加速度、滚轮触觉和强度。
- stage 使用稳定业务 ID；增加/删除/排序不会因数组重排改变已绑定配置。
- 触觉强度、阻尼和滚动模式修改必须即时预览但仍有保存状态；设备不支持触觉时显示 disabled explanation。

来源：[`screens/06-scrolling.md`](screens/06-scrolling.md)。

### 12.8 灯光（Lighting）

**布局**

```text
brightness / master switch
├─ quick effects
├─ effect selection grid
├─ effect parameters       颜色、速度、方向、区域、亮度
├─ preview                 设备/键位/灯带预览
└─ Chroma Connect          联动设备与第三方应用
```

- 亮度区块约 600px 宽、30px padding；主开关、亮度值和滑块处于同一水平层级。
- 效果选择使用网格卡片，效果数量来自设备能力；不支持颜色/速度的效果必须隐藏对应参数，不可显示空控件。
- 颜色控件约 48px 高；颜色 swatch 约 20x20px、圆角约 20%；颜色下拉打开约 250x250px，白色面板、1px 边框、3px 圆角。
- 速度选择器最小宽约 70px，激活态使用 accent；预览区域必须显示当前效果和当前参数，不得只显示静态图片。

**功能**

- 支持灯光开关、zone、亮度、效果选择、颜色、速度、低电量亮度、Chroma Connect、预览和应用/保存。
- 颜色列表支持添加、删除、排序和恢复默认；输入非法颜色时保留草稿并标记字段错误。
- 灯光写入流程为 `discover → startup/register → configure → event/ack → shutdown/unregister`；设备无确认时必须显示 pending/failed，不能把本地保存当作硬件成功。
- `653` 与 `777` 的效果集、区域和参数由 manifest 决定；键盘专属区域不能出现在耳机页面。

来源：[`screens/07-lighting.md`](screens/07-lighting.md)、[`docs/re/02-lighting-actions.md`](re/02-lighting-actions.md)。

### 12.9 声音 / EQ / 增强 / 混音（Sound）

**布局**

- 音量与输出区先显示输出设备/通道，再显示主音量、平衡和静音；说明文字与控件形成上下层级，不让图标代替可读标签。
- EQ 区域采用固定高度约 240px 的纵向说明/控制布局；需要授权或 Synapse 服务时显示约 32x32px 的状态图标与约 210x44px 的提示卡片。
- 增强区按 THX/Dolby、bass、voice clarity、loudness 等能力分卡片；混音区将输入、输出、应用通道和恢复默认分开。

**功能与状态**

- 输出设备、主音量、应用音量、静音、EQ 预设/自定义频段、音效增强、空间音频和混音器均应支持读回、修改、保存失败提示。
- 未安装、未授权、服务未启动和设备不兼容是四种不同状态，不能都显示为“不可用”。
- EQ 滑块键盘可访问；拖动时显示实时数值，失焦/Enter 提交，Escape 恢复本次编辑前值。
- `777` 之外的设备不显示耳机专属增强；页面扩展能力必须由设备 manifest 决定。

来源：[`screens/08-sound.md`](screens/08-sound.md)。

### 12.10 麦克风（Mic）

**布局**

- 麦克风主控区域约占内容左侧 25%，最大宽约 300px，黑色背景、1px 边框、8px/10px padding；包含输入音量、增益、静音和输入设备。
- monitoring/noise cancellation 使用独立 dashboard；基础电平条约 80x5px、4px 圆角，表格字号约 12px/14px，监控数值与设置控件不能混在同一行。
- 增强项、mic boost、监听、噪声抑制、回声消除和高级工具提示分组呈现；profile 提示卡约 300px 宽、accent/danger 边框。

**功能与状态**

- 支持输入设备选择、实时音量/增益、静音、mic boost、monitoring、noise cancellation、voice clarity 和高级调节。
- 监控图表或电平条没有数据时显示 no signal；设备断开时停止动画并显示 disconnected，不能冻结成“正常电平”。
- 噪声抑制/监听启停需要显示 processing；音频服务失败必须允许重试和回退到原设备设置。
- 所有持续刷新只在页面可见且设备连接时运行，离开页面或关闭监控时释放订阅。

来源：[`screens/09-mic.md`](screens/09-mic.md)。

### 12.11 OLED / Display / Haptics

**OLED / Display**

- 页面顺序为显示开关 → 内容/图像/文本来源 → 方向/亮度/超时 → 预览 → 应用；不支持 OLED 的设备不显示占位页面。
- 文本编辑、图像选择、预览和写入分别显示 draft、previewing、uploading、applied、failed；大文件上传显示进度和取消。
- 预览尺寸、裁切和方向必须与设备 descriptor 的物理规格一致，不能只在 UI 层缩放后假装写入成功。

**Haptics**

- 触觉页面先显示总开关，再显示音频到触觉、强度、profile、预览和恢复默认；每项标记作用域是 profile、设备还是全局。
- 触觉预览应有明确触发按钮，不使用循环环境动画；写入失败保留表单值并显示 retry。

来源：[`screens/10-main-frontend-pages.md`](screens/10-main-frontend-pages.md)、`.ref/devices/777/`。

### 12.12 Dashboard、应用设置和主前端页

**Dashboard**

- 采用 `dashboard → box-group → DeviceCard` 层级；设备卡片显示名称、类别、连接状态、关键摘要、快捷入口和更多菜单。
- 卡片支持 hover、focus、selected、disconnected、updating、error；没有设备时显示引导而不是空白画布。
- Dashboard 的卡片顺序与产品 catalog 一致；卡片点击进入产品页，快捷动作必须通过统一 command 分发。

**应用设置**

- 使用 `main-setting` 外壳：左侧持久导航，右侧设置内容；导航选中项、滚动位置和返回焦点可恢复。
- 通用设置包括语言、主题、启动行为、通知、更新、日志和关于；保存反馈在对应设置项附近显示，不使用全局 toast 代替字段错误。
- 破坏性操作（清除缓存、重置配置、退出登录/服务）用确认对话框，Escape 关闭对话框并返回触发控件。

**效果/颜色、游戏模式、音频、增强、混音、演示页**

- `lighting-effects` 复用灯光效果网格和参数卡；`color` 复用颜色选择器和预览；`gaming` 复用 profile/应用关联与开关状态。
- `audio`/`enhancement`/`mixer` 复用声音页的输出、EQ、增强和通道布局；不复制一套相互冲突的音频状态模型。
- `demo` 只能展示可回滚的预览状态，不能绕过设备服务直接写入；预览结束要恢复进入前状态。

来源：[`screens/00-app-shell.md`](screens/00-app-shell.md)、[`screens/10-main-frontend-pages.md`](screens/10-main-frontend-pages.md)。

## 13. GPUI Kit / Design Guide 实现契约

### 13.1 按能力组织，而不是按技术类型堆目录

依赖方向必须向下：app shell 组合能力，能力模块拥有自己的 model、view、command、dialog 和 workflow。不要建立全局的 `models/`、`views/`、`modals/`、`commands/` 垃圾抽屉。

```text
src/
├─ main.rs                      启动参数、主题初始化、窗口选项
├─ shell.rs                     Root/AppShell、顶栏、路由、全局窗口控制
├─ domain.rs                    能力模型、profile、DPI、灯光、音频、工作流状态
├─ model.rs                     设备描述、productId/edition、实测快照
├─ nav.rs                       设备类别到有序页面的映射、TAB 文案与稳定 key
├─ store.rs                     本地 profile/配置持久化与迁移
├─ backend/                     引擎目录、DLL/IPC/设备服务适配；禁止在 render 中调用
├─ features/                    按页面能力组织的页面模块
│  ├─ dashboard.rs              首页设备分组、设备卡片、扫描和入口
│  ├─ setting.rs                应用设置左导航和设置内容
│  ├─ customize.rs              profile、Standard/Hypershift、按键动作
│  ├─ performance.rs            DPI、polling、传感器、键盘性能
│  ├─ pairing.rs                dongle、扫描、配对、解绑状态机
│  ├─ calibration.rs            表面选择、校准弹窗和进度
│  ├─ power.rs                  电池、充电、睡眠和省电
│  ├─ scrolling.rs              滚动阶段、模式和触觉
│  ├─ lighting.rs               效果、颜色、区域、预览和 Chroma
│  ├─ sound.rs / eq.rs          音量、输出、均衡器和预设
│  ├─ enhancement.rs / mixer.rs 音效增强、混音和通道镜像
│  ├─ mic.rs / audio.rs         麦克风、监听、降噪、系统音频路由
│  ├─ keyboard.rs / macros.rs   键盘专属模式、Snap Tap、宏编辑器
│  ├─ display.rs / oled.rs      OLED/显示预览、上传、设备规格
│  ├─ haptics.rs                触觉强度、音频到触觉和滚轮触觉
│  └─ engines.rs                引擎诊断与服务状态，不伪装成产品页
├─ ui/                          只放跨页面视觉组件和几何令牌
│  ├─ widgets.rs                卡片、行、按钮、开关、slider、空状态
│  └─ mod.rs
└─ i18n.rs                      locale 选择、key 查找和格式化
```

目录边界是实现约束：`shell` 只负责窗口/导航/页面组合，`features` 负责页面状态和命令，`domain/model` 负责可序列化数据，`backend` 负责设备协议，`ui` 不得读写设备。不得把没有页面归属的逻辑继续堆进 `shell.rs`，也不得用一个 `placeholder` 页面承接原版已有能力。

### 13.2 GPUI 状态、渲染和副作用

- 每个窗口只初始化一次 `Root`；Root 负责产品壳、overlay、通知、focus scope 和全局 command 路由。
- 保留行为和异步状态放入 `Entity<T>`；无状态、值语义、只依赖输入的控件使用 `RenderOnce`。不要用全局可变状态跨页面共享设备数据。
- 所有列表、设备、profile、mapping、stage 和 overlay 使用稳定业务 `ElementId`；不得用渲染顺序、数组 index 或本地化文本当 ID。
- render 只读状态并生成元素；设备写入、文件 IO、扫描、定时刷新和订阅释放放在 command/workflow/service 中，不能在 render 中触发副作用。
- 异步结果必须回到明确的状态机：`idle/loading/success/error/cancelled/disconnected`；失败必须保留可恢复的草稿或上一份有效值。
- 页面只发送领域 command/event，不直接拼 HID/USB/BLE/DLL 协议；设备服务负责能力检查、序列化、确认和错误映射。

### 13.3 设计系统与可访问性

- 颜色、字体、间距、圆角、边框、阴影和状态全部先定义 semantic token，再由主题提供值；产品页不能直接写原始 hex。
- Button 用于应用内命令，Link 只用于外部/导航链接；危险操作必须有 danger token 和确认流程。
- 键盘焦点可见；Tab 顺序遵循视觉顺序；Enter/Space 执行主动作；Escape 关闭最上层 overlay 并恢复触发控件焦点。
- hover、focus、selected、disabled、loading、validation、danger 都必须有可辨识的视觉差异，不能只依赖颜色。
- 动画只解释状态变化；扫描、上传、写入可用明确进度，禁止无意义的循环动画；支持 reduced-motion。
- tooltip 只补充说明，不能承载唯一操作或唯一错误原因；所有关键状态必须在页面文本中可读。

## 14. 页面覆盖与验收矩阵

| 页面/能力 | 主规格 | 详细事实 | 必验收项 |
|---|---|---|---|
| App shell / Dashboard / Settings | §3、§4、§12.12 | `screens/00-app-shell.md` | 48px 顶栏、持久导航、卡片状态、焦点恢复 |
| Customize / Mapping | §12.2 | `screens/01-customize.md` | profile、Standard/Hypershift、动作编辑、设备差异 |
| Performance | §12.3 | `screens/02-performance.md` | DPI/stage、polling、范围校验、设备能力过滤 |
| Pairing | §12.4 | `screens/03-pairing.md` | 扫描状态机、候选设备、超时/取消/重试 |
| Calibration | §12.5 | `screens/04-calibration.md` | 表面选择、进度、失败保留、弹窗焦点 |
| Power | §12.6 | `screens/05-power.md` | 电量、充电、超时、禁用态、写入确认 |
| Scrolling | §12.7 | `screens/06-scrolling.md` | stages、滚动模式、触觉、响应式溢出 |
| Lighting | §12.8 | `screens/07-lighting.md`、`re/02-lighting-actions.md` | effect/color/preview、能力过滤、硬件 ack |
| Sound / EQ | §12.9 | `screens/08-sound.md` | 输出、EQ、授权、增强、混音状态 |
| Mic | §12.10 | `screens/09-mic.md` | 电平、监听、降噪、服务断开、订阅释放 |
| OLED / Display / Haptics | §12.11 | `screens/10-main-frontend-pages.md` | 预览、上传、设备规格、作用域 |

完成实现时，逐行勾选 §10，并同时核对对应 `docs/screens/*.md` 的 CSS 类名、尺寸表和 locale key。任何“看起来差不多”但没有来源证据的布局，都视为未完成。

## 15. 全量界面、模块与交互对齐表

本节是对 §1–§14 的最终执行清单。它把原版中容易被遗漏的主前端页面、产品扩展页、弹窗、服务状态和窗口行为全部列出。实现时以本节和 `docs/screens/*.md` 为联合契约；二者冲突时，以对应 `.ref` 产品模块的 JSX/CSS/locale/manifest 为准。

### 15.1 窗口与根容器

#### 原版事实

- Electron 页面内部没有 `.title-bar`、`.titlebar`、`.drag-region` 或网页 `window-controls`；网页第一行就是 `.nav-tabs`。
- 窗口边框、最小化、最大化/还原、关闭和拖动由 Electron 壳处理，不属于产品模块 DOM。
- `.main-container` 是唯一纵向根容器，`min-width:600px`；`#body-wrapper` 是唯一页面滚动/内容容器。
- 产品窗口和主前端窗口使用同一套 `.nav-tabs` 视觉规则，但中间标签来自不同能力集合。

#### Rust/GPUI 实现约束

- 使用无边框窗口时，必须在外层 `42px` `.etabs-tabgroup` 右侧提供三个系统控制命中区：`Min`、`Max/Restore`、`Close`，每个宽 `48px`。
- 不得渲染额外的 `34px TitleBar`；允许且必须保留“Electron 外层标签栏 + 产品 `.nav-tabs`”这两层原版结构。
- 外层按钮必须 `no-drag`，外层剩余区域才是拖动区；按钮不能把产品 `.navs-wrapper` 挤出居中位置。
- 拖动区只能覆盖顶栏空白区域；profile、导航、帮助、电池、最小化、最大化、关闭等交互元素必须优先命中，不能被拖动事件吞掉。
- 双击拖动空白区执行最大化/还原；关闭按钮使用系统关闭命中区，不能用普通页面按钮伪装。
- 内容区从顶栏下方开始，禁止把 `#body-wrapper` 的 padding、产品图区或卡片向上偏移来补偿窗口栏。

### 15.2 主前端页面全集

主前端能力不是设备页能力。以下页面必须由主前端路由按能力显示，不能因为 `TAB_*` 存在就给所有设备添加同名标签。

| 页面/入口 | 原版主体 | 必须包含 | 禁止做法 |
|---|---|---|---|
| `TAB_HOME` Dashboard | `.dashboard`、`.box-group`、`DeviceCard` | 分组展开/折叠、拖拽排序、设备卡片、在线/离线/扫描/配对/失败/空状态、进入设备页 | 用产品页 600px widget 两列代替首页 |
| `TAB_SETTING` 应用设置 | `.main-setting`、`.side-navigation`、右侧设置内容 | 约 180px 固定左栏、设置分组、语言/账户/启动/通知/更新/日志/关于、危险确认 | 复用产品页顶栏当左侧设置导航 |
| `TAB_BATTERY` 电池摘要 | 电池摘要卡/状态面板 | 百分比、充电、电源来源、低电量、无电池和设备不支持状态 | 没有电池时显示假的 0% |
| `TAB_AUDIO` 音频 | 音频设备与路由模块 | 输出设备枚举、默认设备、音频模式、服务不可用和至少一个设备的依赖提示 | 用设备自身声音页冒充系统音频页 |
| `TAB_GAMING` 游戏/性能 | 游戏模式及应用关联 | profile/应用关联、Win/Alt+Tab/Alt+F4 等锁定、启停状态和冲突提示 | 无键盘能力时显示键盘专属锁定项 |
| `TAB_COLOR` / `TAB_EFFECTS` | 主前端灯光扩展 | 颜色/效果入口、预览、设备/区域范围和 Chroma 连接状态 | 把两个入口无条件显示为重复的灯光页 |
| `TAB_DISPLAY` / `TAB_OLED` | 显示/OLED 模块 | 设备规格过滤、内容预览、上传/应用、格式校验、失败重试和恢复默认 | 只显示一个“上传文件”按钮而没有预览/范围状态 |
| `TAB_ENHANCEMENT` | 增强模块 | 可用增强项、依赖、开关/强度、互斥关系、设备确认 | 把不支持的增强项显示为可写 |
| `TAB_EQ` | EQ 模块 | 预设、频段编辑、重置、保存/应用、设备支持的频段数量 | 用固定频段数量覆盖所有耳机 |
| `TAB_MIXER` | 混音模块 | 通道、音量、镜像/路由、输出设备状态和服务断开 | 只显示静态音量文本 |
| `TAB_HAPTICS` | 触觉模块 | 触觉强度、音频到触觉、滚轮/游戏事件来源、依赖和关闭确认 | 把触觉开关与灯光状态混用 |
| `TAB_DEMO` | 演示/预览 | 可回滚的预览状态、开始/停止、恢复进入前状态 | 通过演示页绕过设备服务直接写硬件 |

首页设备分组的标题、拖拽把手、折叠箭头和 `DeviceCard` 的状态层级必须独立实现：标题行约 14px；卡片约 290×220px；设备图片区约 140px；信息区至少 50px；展开背板为 `#333`、圆角 5px，拖拽中为 2px 绿色描边。分组拖拽必须同时提供键盘上移/下移命令，不能只支持鼠标。

### 15.3 三个已抓取产品模块的完整页面契约

#### `productId=182`：Razer DeathAdder V3 Pro

标签顺序必须是：

```text
自定义 → 性能 → 正在配对 → 校准 → 电源 → 滚动
```

- 自定义：profile bar、Standard/Hypershift、鼠标预览、按键/滚轮动作编辑器、灵敏度/灯光/跨设备动作。
- 性能：DPI stages、X/Y 灵敏度、轮询率、抬升距离、智能跟踪、非对称截止、motion sync、角度修正、加速度和灵敏度匹配；只显示 manifest 声明的能力。
- 正在配对：接收器类型/固件、扫描指引、候选设备、配对中、已配对列表、解绑确认和失败重试。
- 校准：表面列表、内置/自定义表面、选择、校准弹窗、进度、成功/失败/取消和删除保护。
- 电源：电池摘要、充电/无线状态、睡眠时间、灯光省电和低电量状态。
- 滚动：滚动阶段、刻度/自由滚动模式、触觉强度、方向、应用和设备确认。

#### `productId=653`：BlackWidow V4 Pro

标签顺序必须是：

```text
自定义 → 性能 → 灯光 → 电源 → 滚动
```

- 自定义：逐键映射、组合键、Hypershift、宏、媒体、拨轮/旋钮、Snap Tap/快速击键、Dynamic Keystroke；键盘图上的每个输入点必须有稳定业务 ID。
- 性能：轮询率、Gaming Mode、Win/Alt+Tab/Alt+F4 锁定、N-key rollover、Snap Tap、Dynamic Keystroke 和 actuation；不得显示鼠标 lift-off。
- 灯光：区域、亮度、效果网格、速度/方向/颜色、实时预览、Chroma Connect 和设备确认。
- 电源：有线/无线/蓝牙连接、电池或不适用状态、睡眠/灯光省电；无电池字段时不要伪造百分比。
- 滚动：仅在 manifest 有滚动/拨轮能力时显示；页面布局仍使用 600px widget 约束，不得复制鼠标专属校准控件。

#### `productId=777`：RAZER KRAKEN BT SANRIO LIMITED EDITION

标签顺序必须是：

```text
自定义 → 灯光 → 校准 → 电源 → 声音 → 麦克风
```

- 自定义：耳机按键、旋钮和可用设备动作；不得出现鼠标图或键盘逐键图。
- 灯光：耳机灯光区域、颜色/效果/亮度、预览和 Chroma 连接状态。
- 校准：耳机/摇杆/灵敏度匹配等由模块能力决定；没有表面校准字段时不显示鼠标表面网格。
- 电源：电池、充电、无线连接、睡眠和低电量提示。
- 声音：输出设备、音量、音频模式、THX/空间音效、EQ、增强、预设和设备确认。
- 麦克风：输入电平、增益、监听/侧音、采样率、高通滤波、模拟增益限制器、降噪训练、录音进度和服务断开。

### 15.4 产品页公共视觉合同

公共合同只约束容器，不强行约束页面内容：

1. `nav-tabs` 最小高 48px；`body-wrapper` 使用 `10px 20px 20px` 内边距。
2. 产品图使用 `.widget-prod` 时，高 250px、最小宽 1024px、最大宽 1220px、上下 margin 10px；图片以绝对定位在区域中心，特殊音频/耳机布局遵循模块覆盖规则。
3. `.body-widgets` 最大宽 1240px、水平居中、可换行；普通 `.widget` 固定 600px、30px 纵向/40px 横向内边距、背景 `#111`、圆角 5px、正文 14px。
4. `.widget-col` 为 600px 纵向列；卡片之间使用原版 10px margin，不用全局 24px 间距替代。
5. `.thx-btn` 为应用主按钮：绿色、黑字、3px 圆角、约 `.5rem 1.5rem` 内边距、大写；hover/active/disabled 分别使用 `.8/.6/.3` 透明度语义。
6. `.switch` 为约 32×18px；关闭轨道 `#707070`、开启轨道 `#44d62c`、深色 14px thumb；不能使用尺寸不同且颜色反转的通用开关。
7. `.s3-dropdown` 高约 27px、边框 `#515151`、列表最大高约 180px、选项约 25px；枚举字段必须使用下拉/单选，不能改成左右步进器。
8. 连续值必须显示 slider、当前值、单位、最小/最大和 disabled/loading 状态；输入框与滑块双向同步。
9. 产品图、卡片和弹窗不得使用“产品图位置”“尚未接通但成功”等占位文案冒充原版功能；缺素材时显示明确的 capability/asset unavailable 状态。

### 15.5 所有弹窗、抽屉和全局 overlay

overlay 不得改变页面主轴，且必须有打开来源、焦点进入、Escape 关闭和关闭后焦点恢复：

| overlay | 触发页面 | 内容和状态 |
|---|---|---|
| profile 菜单 | 顶栏/自定义 | 切换、新建、复制、重命名、删除、导入、导出、恢复默认、当前 profile 标记 |
| 删除/解绑确认 | profile、配对、设备卡 | 对象名称、不可逆说明、取消/确认、danger 样式、失败重试 |
| 动作选择器 | 自定义/宏 | 分组动作、搜索、设备能力过滤、参数编辑、冲突提示 |
| 宏编辑器 | 自定义/宏 | 录制、步骤列表、排序、延迟、循环/按住、预览、取消草稿、保存失败 |
| 配对流程 | 配对页 | idle/scanning/candidate/pairing/connected/timeout/failed/cancelled，主按钮焦点保持唯一 |
| 校准向导 | 校准页 | 表面/设备选择、阶段、进度、成功、失败、重试、取消，内部滚动 |
| 颜色选择器 | 灯光/颜色 | 色板、当前颜色、RGB/HSV 输入、预览、取消/应用、键盘输入 |
| 固件/更新提示 | Dashboard/设置/设备 | 版本、下载、安装、重启需求、失败、稍后处理；不能把下载中当完成 |
| 音频授权/依赖确认 | 音频/增强/触觉 | 依赖服务、将被关闭的联动能力、确认后状态和取消恢复 |
| 文件上传 | OLED/显示 | 文件类型/大小/分辨率校验、预览、上传进度、失败、替换确认 |
| 通用错误 | 所有页面 | 可读错误原因、保留草稿、重试/取消；禁止只显示 toast 后丢状态 |

### 15.6 所有状态和写入语义

每个会改变设备或 profile 的控件都必须区分以下状态；页面文字和视觉不能只靠颜色：

```text
unavailable       能力不存在或模块未声明
disconnected      设备/服务断开，保留最后摘要，禁止写入
idle              可开始操作
loading           扫描、读取、上传或写入进行中，防止重复提交
dirty             只有本地草稿改变，尚未保存
saving            正在写 profile/本地配置
awaiting-device   本地写入完成，等待硬件确认
saved             profile 和硬件都确认成功
failed            写入/扫描/上传失败，保留可重试草稿
cancelled         用户取消，恢复操作前状态
```

- 本地文件保存成功不能显示“设备已应用”；必须在硬件服务确认后才显示 `saved`。
- 设备掉线时保留最后一次有效值，并显示 `disconnected`；不能清零、回退成默认值或继续启用写入控件。
- 快速连续点击保存只能合并为一个 workflow；重复扫描、重复配对和重复上传必须被阻止。
- 页面切换遇到 `dirty` 状态时提供保存、放弃、取消；Escape 只关闭最上层 overlay，不能静默丢稿。

### 15.7 路由与能力过滤

路由的唯一来源是 `productId + edition + manifest/descriptor`：

```text
device discovery
  → descriptor
  → ordered_pages
  → capability flags
  → feature view
  → command/workflow
```

- `Tab::Battery`、`Tab::Color`、`Tab::Effects`、`Tab::Gaming`、`Tab::KeyBinds`、`Tab::Demo` 是主前端扩展或兼容入口，不得无条件插入三个产品的设备页顺序。
- 如果 Rust 需要兼容这些入口，必须明确标记为 host extension/alias，并在能力不满足时隐藏或显示真实 empty state，不能把它们伪装成原版产品标签。
- 页面不可用、设备未声明或服务未连接时，页面仍可以显示结构化说明，但主按钮必须 disabled，说明必须写明缺失能力和恢复路径。
- `placeholder.rs` 不能作为已有原版能力的最终路由；只有原版没有对应模块且明确标注 unsupported 时，才能使用 capability empty state。

### 15.8 本地化、键盘和可访问性

- 所有标题、按钮、状态、错误和 tooltip 优先来自 `.ref` locale key；新增中文只能作为明确的实现状态说明，不能替换原版功能文案。
- 大写只用于原版 `.nav`/`.thx-btn` 等明确使用 `text-transform:uppercase` 的区域；中文不做无意义大小写转换。
- 所有卡片、分组、设备、profile、stage、映射点和 overlay 使用稳定业务 ID；禁止数组 index、显示文本或本地化结果作为持久化 ID。
- Tab 顺序必须遵循页面视觉顺序；Enter/Space 执行按钮；方向键可操作 stage/slider；Escape 关闭 overlay 并恢复焦点。
- focus-visible 必须有描边或高对比背景；disabled 必须降低对比度并阻止指针；loading 必须禁用重复命令；danger 必须有确认。
- tooltip 只能补充说明；主操作、错误原因、断线原因和保存结果必须直接出现在页面或 dialog 文本中。

### 15.9 当前 Rust 文件到原版模块的对照

| Rust 模块 | 原版职责 | 必须负责的范围 |
|---|---|---|
| `shell.rs` | `.main-container`、`.nav-tabs`、窗口 | 根布局、窗口命中区、设备页路由、全局 overlay 入口；不承载设备协议 |
| `features/dashboard.rs` | Dashboard | 分组、设备卡、扫描/空/失败/入口状态 |
| `features/setting.rs` | `.main-setting` | 左导航、应用设置、确认与保存反馈 |
| `features/customize.rs` / `macros.rs` | Mapping/Map* | profile、输入点、动作编辑、宏 workflow |
| `features/performance.rs` / `keyboard.rs` | 性能/键盘模块 | DPI、轮询率、Gaming、Snap Tap、actuation |
| `features/pairing.rs` / `calibration.rs` | 配对/校准模块 | 完整状态机、弹窗、候选/表面选择 |
| `features/power.rs` / `scrolling.rs` | 电源/滚动模块 | 电池、睡眠、滚动阶段、触觉 |
| `features/lighting.rs` | Lighting/Color/Effects | 区域、效果、颜色、预览、Chroma |
| `features/sound.rs` / `eq.rs` / `enhancement.rs` / `mixer.rs` | 音频模块 | 输出、EQ、增强、混音及依赖 |
| `features/mic.rs` / `audio.rs` | Mic/Audio | 电平、监听、降噪、系统设备枚举和断开 |
| `features/display.rs` / `oled.rs` / `haptics.rs` | Display/OLED/Haptics | 预览、上传、触觉和设备过滤 |
| `backend/` | 原生引擎/IPC | capability、读写、确认、错误映射、订阅释放 |
| `domain.rs` / `model.rs` | descriptor/profile/settings | 可序列化状态、设备能力和页面输入模型 |
| `ui/widgets.rs` | CSS 视觉原语 | card、row、button、switch、slider、empty state；不添加原版没有的公共页面标题 |

### 15.10 完成标准

一个页面只有在以下条件全部满足时才算“完成”：

1. 页面只在正确的 productId/edition/capability 下出现，顺序与 manifest/原版模块一致。
2. DOM/GPUI 层级、宽高、内边距、圆角、颜色、字体和状态与对应 screen 文档一致。
3. 主操作、次操作、危险操作、禁用、加载、成功、失败、断线和空状态都有可见表现。
4. 页面中的每个写入动作都经过 command/workflow/service，并区分本地保存与硬件确认。
5. 弹窗、抽屉、颜色选择器、上传器和 profile 菜单不改变页面主轴，焦点可以进出并恢复。
6. 键盘可以完成与鼠标相同的主要任务，列表和设备卡不依赖数组顺序。
7. 代码中没有“模拟配对成功”“产品图位置”“尚未接通但成功”“占位页承接真实能力”等伪完成文案。
8. 对应 `docs/screens/*.md`、`.ref` 证据路径和 `src/features` 页面实现可以逐项互相追溯。
## 0.1 源码优先与过时文档淘汰规则

本规范只接受 `.ref` 原代码证据。任何 `docs/screens/*.md` 与 `.ref` 冲突时，必须改写或删除，不能继续作为实现依据。

总层级必须保持为：Electron `.etabs-tabgroup`（`42px/#000`，右侧 `3×48px`、`no-drag` 系统按钮）→ 产品 `.main-container`（`.nav-tabs` `48px/#222`、下边框 `2px/#000`）→ `.body-wrapper`（`10px 20px 20px`）→ 模块自己的 `.widget-prod` / `.body-widgets` / `config-wrapper`。不得额外加入 GPUI `34px TitleBar`，也不得把系统按钮塞进产品 `.nav-tabs`。

禁止把所有页面套成同一个通用模板：原版没有统一 `PageHeader`；`182` 自定义使用 `config-wrapper/config-block/ButtonPanelComponent`，`653` 使用键盘真实布局，`777` 使用耳机模块自己的声音/麦克风结构。没有 `.ref` 证据的尺寸、颜色、页面顺序和成功状态必须标为 `unverified`、降级为 `unsupported/unavailable`，或直接删除。旧截图、旧架构说明和冲突 MD 不恢复。
