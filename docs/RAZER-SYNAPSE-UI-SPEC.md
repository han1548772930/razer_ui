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
    ├── .widget-prod          1024–1220px × 250px
    └── .body-widgets         max-width:1240px，居中换行
        ├── .widget           600px 宽，30px/40px 内边距
        └── .widget-col       600px 宽，内部纵向堆叠
```

600px widget 是原版主要信息表面；不要把所有控件压成一张全宽卡片。产品图片区与功能卡片是两个视觉层。

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

顶栏只有一条，最小高度 `48px`，背景 `#222`，下边框 `2px solid #000`。不要再叠加第二条伪标题栏。

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

左侧支持 profile 选择、新建、复制、删除、重命名、自动/手动切换和程序关联提示。右侧按能力显示电池、充电、低电量、帮助、立体声/更新/重启警告和过滤工具；没有能力时不显示假警告。

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
├── app/                  启动、窗口、全局 overlay
├── shell/                Dashboard、产品壳、顶栏
├── catalog/              product/edition/manifest/能力
├── domain/               device、profile、mapping、lighting、performance、audio、calibration
├── pages/                PageId → view
├── components/           widget、field、switch、dropdown、slider、dialog
├── services/             异步设备服务、存储、通知
├── protocols/            HID、USB、BLE、Protocol 25/30/40、DLL adapter
└── i18n/                 locale、格式化、单位
```

页面只发 domain command，不直接拼硬件协议；组件只负责展示和交互；能力由 `DeviceDescriptor` 提供；列表使用稳定 ID；side effect 不得发生在 render；原始颜色只存在主题层。

## 10. 验收清单

- [ ] Dashboard、应用设置、产品页层级正确。
- [ ] 页面顺序来自产品模块，未支持页面不显示。
- [ ] 顶栏为单条 `48px`，产品页使用 `600px` widget/`1240px` body。
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
