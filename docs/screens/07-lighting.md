# 灯光（`TAB_LIGHTING`）

> 本文只记录 `.ref` 中设备模块实际渲染出来的灯光界面。判断顺序是：
> `main.js` / 动态 chunk 的 `render()` 与条件分支优先，CSS 只用于确认已渲染节点的几何和视觉；共享 CSS 类名、语言包 key、未被当前产品 render 调用的组件都不能单独证明页面存在。

## 1. 页面归属

| 设备 | productId | 模块 | 是否有灯光标签 | 产品标签顺序 |
|---|---:|---|---|---|
| BlackWidow V4 Pro | `653` | `Selenat1_UI` | 有 | 自定义 → 性能 → 灯光 → 电源 → 滚动 |
| RAZER KRAKEN BT SANRIO LIMITED EDITION | `777` | `Kittybt_UI` | 有 | 自定义 → 灯光 → 校准 → 电源 → 声音 → 麦克风 |

标签文案来自各模块的 `TAB_LIGHTING`，但标签存在不代表所有灯光子控件都出现。具体控件以该设备 `main.js` 的 render 条件为准。

## 2. 证据边界

### 2.1 653 的产品能力

`.ref/devices/653/static/js/main.7b71cce5.js` 的产品常量块明确声明：

- `productId:653`、`category:"KEYBOARD"`、设备名 `Blackwidow V4 Pro`。
- `isOBMDevice:true`，支持硬件 profile；`OBMSlots:4`。
- `OBMSpecs.supportedFeature` 只在该块声明 `brightness:true`、`dpi:false`、`pollingRate:true`。
- 同一产品常量块定义 `QUICK_EFFECTS` 共 12 项：`Ambient`、`Audio Meter`、`Breathing`、`Fire`、`Reactive`、`Ripple`、`Spectrum`、`Starlight`、`Static`、`Tidal`、`Wave`、`Wheel`。
- 同一块定义 `HARDWARE_QUICK_EFFECTS` 共 7 项：`Breathing`、`Reactive`、`Spectrum`、`Starlight`、`Static`、`Tidal`、`Wave`。

因此 653 页面不能把“所有通用 Chroma 效果”无条件当作硬件可用；硬件效果和软件/Chroma 效果必须按当前 render 分支使用的数据集区分。

### 2.2 653 的实际灯光 render

在 `main.7b71cce5.js` 的灯光组件 render 中可以观察到两条实际分支：

1. **普通 quick-effect 分支**
   - 外层是 `modes-area`，只有满足能力条件时追加 `active`；不满足时保持隐藏或不可用状态。
   - 先渲染 `quickeffect-text` 说明文字。
   - 接着渲染 `chroma-flex-row`：一个以 `deviceQuickEffect` 为数据源的 dropdown；非硬件模式额外渲染 `chroma-sync` 同步入口。
   - dropdown 改变当前效果后调用 `renderEffect()`，效果参数区不是固定卡片，而是由当前 effect id 选择具体 effect component。
   - 只有 `DeviceInfo.automationSupported` 为真时才追加 warning/link 区块。

2. **BLE / hardware-effect 分支**
   - 先渲染 `quickeffect-text`。
   - 如果支持全局应用，会渲染 `checkIsToggleGlobalQuickEffect` 开关以及针对设备/端口的说明。
   - 开关打开后才渲染 `chroma-flex-row`、效果 dropdown 和 `renderEffect({ selectedEffectId, effectSettings, regionId:0 })`。
   - 硬件模式不应强行显示普通模式的 Chroma 同步入口；同步入口由 `!useHardwareEffect` 条件控制。

### 2.3 777 的实际灯光 render

`.ref/devices/777/static/js/main.eb70ce38.js` 使用同一套产品灯光基础组件，但条件不同：

- 亮度组件 render 为带 `hasSwitch:true` 的 widget：开关控制 `brightness.isEnabled`，滑块范围 `0..100`，步长默认 `1`；组件还可能在 `supportLogoBrightness` 为真时追加 `logo-brightness-container`。
- Chroma / quick-effect 组件先异步检查资源安装状态；`allChromaResourcesInstalled` 为假时不渲染完整效果设置，而是渲染安装/资源不可用状态。
- 资源可用时先渲染 `adveffect-detail`；非 SensaHD 分支才追加 Chroma profile dropdown 和 `chroma-studio-btn` 入口。
- `isChromaVisualizerEnabled` 会使 `modes-area` 进入 disabled；只有 BLE 且设备类别为 `KEYBOARD` 时例外。777 是耳机，不应套用 653 键盘的例外。

结论：777 可以共享组件实现，但不能从 653 的硬件键盘条件推导出 777 的区域、键盘效果或硬件快速效果。

## 3. 共用页面骨架

以下是 653/777 灯光组件实际使用的设备模块骨架；这是产品模块内部网页，不包含宿主窗口系统按钮。

```text
.main-container                         # 纵向填满窗口
├─ .nav-tabs                            # 48px 产品标签栏
└─ #body-wrapper / .body-wrapper        # flex:1，内边距 10px 20px 20px
   ├─ .widget-prod                      # 产品展示区；若当前模块 render 提供
   └─ .body-widgets                     # 最大 1240px，横向排列并自动换行
      ├─ .widget                        # 600px 固定宽度的功能 widget
      └─ .widget-col                    # 600px 纵向 widget 列
```

### 3.1 尺寸和基础颜色

这些值来自 653 与 777 各自的 `static/css/main.*.css`，且是被灯光 render 使用的通用 widget 样式：

| 元素/选择器 | 实际样式 |
|---|---|
| `body, html` | `Roboto`、`16px`、背景 `#222`、文字 `#ccc`、`min-height:720px`、`overflow:hidden` |
| `.nav-tabs` | `min-height:48px`、背景 `#222`、下边框 `2px solid #000`、默认文字 `#5d5d5d` |
| `.body-wrapper` | `padding:10px 20px 20px`、宽度填满、内容区可滚动由页面容器控制 |
| `.body-widgets` | `display:flex`、`flex-wrap:wrap`、`justify-content:center`、`max-width:1240px` |
| `.body-widgets .widget` | `min-width/max-width:600px`、`padding:30px 40px`、`margin:10px auto`、背景 `#111`、圆角 `5px`、字号 `14px` |
| dropdown / option button | 通常是 `#000` 或透明底、`1px solid #5d5d5d`、圆角 `3px`；激活边框 `#44d62c` |
| 主色 | `#44d62c`；激活、标题、滑块填充、选中边框使用该色 |
| 次要文字 | `#999` / `#707070`；禁用状态通常使用 `opacity:.3` |
| 警告 | `#fd8611`；危险/错误状态使用 `#fd4949` 或 `#c8323c` |

## 4. 653 页面自上而下的真实结构

### 4.1 亮度 widget

653 `main.js` 中实际存在 `brightness-container` 组件 render；不是“灯光区域列表”。其行为是：

- widget 使用开关控制 `brightness.isEnabled`。
- 内部 slider 使用 `min:0`、`max:100`；步长由 `steps` prop 提供，未提供时为 `1`。
- slider 激活条件为 `nanoLeafEnabled && brightness.isEnabled`；不满足时，widget 通过 disabled 样式降低不透明度并阻止交互。
- 只有产品 props 提供 `supportLogoBrightness` 时，才追加 `logo-brightness-container` 和 Logo 开关；不能因为键盘有灯光就必然显示 Logo 独立亮度。

653 CSS 对该区域的产品化覆盖：

- `#multipleBrightness .brightness-container`：`margin-top:10px`、`padding:30px`、`width:600px`。
- `.switching-brightness`：`display:inline-flex`、`align-items:center`、`margin:0 10px 10px`。
- `.label-brightness`：主色 `#44d62c`、字号 `16px`。
- `.global-brightness`：`display:flex`、高度 `20px`、`margin:10px`；左右两侧分别使用 `margin-right:auto` / `margin-left:auto`。

### 4.2 Quick Effects widget

653 的 quick effect 不是每个效果一个大卡片，而是一个 widget 内的两列/多列紧凑布局：

- `.multiple-quickeffect`：`display:flex`、`flex-wrap:wrap`、纵向间距 `21px`。
- 每个 `.quickeffect` 占 `width:50%`；不可用时 `.quickeffect--disabled` 为 `opacity:.3` 且不可点击。
- `.quickeffect__header`：横向 flex、`gap:10px`、下边距 `5px`、文本大写。
- `.quickeffect__dropdown`：宽 `200px`。
- 无线/紧凑效果区使用 `width:150px` 的包裹区；不能把所有参数拉伸成整行。

### 4.3 当前效果参数区

dropdown 选中效果后，653 的 `renderEffect()` 选择对应 effect component。CSS 通过 `data-effect` 只显示相关参数：

- `wave` 显示方向，并使用 `315px` 的 speed slider wrapper。
- `breathing` / `starlight` 使用 flex 参数区。
- `ambient`、`audioMeter`、`fire`、`spectrum`、`wave` 等效果会隐藏不适用的主色控件。
- `reactive` / `ripple` / `static` 会按 `data-effect` 隐藏第二颜色或随机颜色控件。
- `audioMeter` 的 stepper 输入背景透明；普通 stepper 为 `#111`、边框 `#5d5d5d`、高度 `27px`、宽度 `60px`。
- `effects-area` 采用相对定位；参数区是当前选中效果的条件子树，不应预先展示“颜色、速度、方向”全部字段。

### 4.4 同步、自动化和禁用状态

- 普通软件效果分支有 `chroma-sync` 与同步文字；硬件效果分支按 `useHardwareEffect` 条件隐藏同步入口。
- automation 支持只在 `automationSupported` 为真时显示 warning/link；不能用语言包里存在自动化文案作为渲染依据。
- `isChromaVisualizerEnabled` 或电池省电效果会把 `modes-area` 置为 disabled；CSS 是 `opacity` / `pointer-events` 语义，而不是删掉状态文案。
- 资源未安装时应该显示安装/不可用分支；不要显示空的效果控制器或伪造已应用。

## 5. 777 页面真实差异

777 的灯光页不是 653 键盘页的复制品：

1. 设备类别是耳机；不渲染键盘 SVG、逐键灯光、Snap Tap 或键盘硬件快速效果。
2. 亮度组件仍然是 `brightness` slider + switch，范围 `0..100`；Logo 子开关只有 `supportLogoBrightness` 为真才存在。
3. Chroma profile 区先等待资源检查；`allChromaResourcesInstalled` 为假时只显示安装/资源状态。
4. 资源可用时显示效果说明、Chroma profile dropdown；`chroma-studio-btn` 由非 SensaHD 分支提供。
5. Visualizer 打开时，非键盘设备的 `modes-area` 进入 disabled；777 不满足 BLE keyboard 例外。

因此 777 的文档只能描述这些真实分支，不能把 653 的 `HARDWARE_QUICK_EFFECTS`、键盘按键区或键盘特定条件移植过去。

## 6. 不应出现在本页的推定内容

以下内容不能仅凭共享 CSS、语言包 key 或通用组件名写入 653/777 灯光页：

- “每个设备都有标志、滚轮、底部灯带等灯光区域”。653/777 的 render 证据是 quick-effect / brightness / effect component，不是通用 `LightingZone` 列表。
- “所有 10/12 个效果始终显示且全部可用”。653 明确同时存在 `QUICK_EFFECTS` 与 `HARDWARE_QUICK_EFFECTS`，实际 dropdown 数据由 props 决定。
- “所有效果同时显示颜色、速度、方向”。实际由 `renderEffect()` 和 `.effects-area[data-effect=...]` 条件控制。
- “灯光写回 awaiting-device”“Chroma Connect 未接通”等项目自己的后端状态。这不是原版 653/777 页面 DOM 证据，不应混入原版规格。
- “777 使用键盘的 Chroma 同步/硬件效果”。777 的 render 条件按耳机类别和资源安装状态分支，不能由 653 推断。

## 7. 原代码证据索引

### 653

- `.ref/devices/653/manifest.json`：`deviceName`、`layoutId`、模块版本。
- `.ref/devices/653/asset-manifest.json`：`MapLighting.js`、`MapBrightness.js`、`MapBrightnessGlobal.js` 资源登记；这些是实际存在的 chunk，不代表每个 chunk 都是灯光标签主体。
- `.ref/devices/653/static/js/main.7b71cce5.js`：
  - 产品常量 `productId:653`、`category:"KEYBOARD"`、`QUICK_EFFECTS`、`HARDWARE_QUICK_EFFECTS`。
  - `brightness-container` render：`hasSwitch:true`、亮度范围 `0..100`、`supportLogoBrightness` 条件。
  - `modes-area` render：`deviceQuickEffect` dropdown、`chroma-sync` 条件、`renderEffect()`、`automationSupported` warning 条件。
  - 键盘硬件分支：`useHardwareEffect`、`isBle`、`category===KEYBOARD`、`checkIsToggleGlobalQuickEffect`。
- `.ref/devices/653/static/css/main.5425442a.css`：
  - `#multipleBrightness`、`.brightness-container`、`.multiple-quickeffect`、`.quickeffect__header`、`.quickeffect__dropdown`。
  - `.effects-area[data-effect=...]` 对颜色、方向、速度、stepper 的条件显示。
  - `.modes-area`、`.modes-area.active`、`.modes-area.invisible`、disabled 状态。

### 777

- `.ref/devices/777/manifest.json`：模块名 `Kittybt_UI`、设备名和产品翻译。
- `.ref/devices/777/static/js/main.eb70ce38.js`：
  - 亮度组件的 switch/slider/Logo 条件。
  - Chroma 资源检查 `allChromaResourcesInstalled`。
  - `adveffect-detail`、Chroma profile dropdown、`chroma-studio-btn` 的 render 条件。
  - `isChromaVisualizerEnabled` 与 BLE keyboard 例外条件。
- `.ref/devices/777/static/css/main.e4bab2aa.css`：
  - `#multipleBrightness` 下的亮度、quick effect 两列布局。
  - `.modes-area`、`.effects-area`、`.panel-light--chroma`、widget 尺寸颜色。

## 8. 与项目实现的边界

本文件是原版规格，不宣称当前 Rust 页面已经完成。任何实现 653/777 灯光页的改动，都必须先满足本文件的条件渲染和布局顺序，再接入真实设备服务；没有原代码证据的控件必须标记为待验证，而不能以通用能力模型替代。
