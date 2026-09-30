# 电源（TAB_POWER）

> 本页由雷云自己的产品模块提取；本文件只记录原版事实，不记录本项目实现状态。
> 证据工具：`node .ref/tools/gen-screen-docs.js`

## 1. 出现在哪些设备上

| 设备 | productId | 设备类型 | 该设备标签页顺序 |
|---|---|---|---|
| Razer DeathAdder V3 Pro | 182 | 鼠标 | 第 5 个：自定义 · 性能 · 正在配对 · 校准 · 电源 · 滚动 |
| Blackwidow V4 Pro | 653 | 键盘 | 第 4 个：自定义 · 性能 · 灯光 · 电源 · 滚动 |
| RAZER KRAKEN BT SANRIO LIMITED EDITION | 777 | 耳机 | 第 4 个：自定义 · 灯光 · 校准 · 电源 · 声音 · 麦克风 |

标签页真实文案：**电源**（key `TAB_POWER`）

## 2. 布局

### 2.0 页面骨架（所有设备页共用）

设备页**不是**「一列全宽卡片」。真实骨架如下（逐条引自设备模块的 CSS）：

| 选择器 | 布局 | 样式 |
|---|---|---|
| `body, html` | `font-family:Roboto,sans-serif; font-size:16px; height:100%; margin:0; max-width:1920px; min-height:720px; overflow:hidden; width:100%` | `background-color:#222; color:#ccc; user-select:none` |
| `.main-container` | `display:flex; flex-direction:column; height:100%; min-width:600px; position:absolute; width:100%` | `background-color:#222` |
| `.nav-tabs` | `display:flex; align-items:center; min-height:48px; position:relative; width:100%; z-index:105` | `background-color:#222; border-bottom:2px solid #000; color:#5d5d5d` |
| `.body-wrapper` | `flex:1 1; height:100%; min-width:600px; padding:10px 20px 20px; width:100%` | — |
| `.body-widgets` | `flex-direction:row; flex-wrap:wrap; justify-content:center; margin:auto; max-width:1240px` | — |
| `.body-widgets .widget` | `flex:0 0 auto; height:auto; margin:10px auto; max-width:600px; min-width:600px; padding:30px 40px; position:relative; font-size:14px` | `background-color:#111; border-radius:5px` |
| `.widget-col` | `flex-direction:column; height:fit-content; width:600px` | — |
| `.widget-prod` | `height:250px; margin:10px auto; max-width:1220px; min-width:1024px; width:100%` | — |
| `.widget-prod img` | `left:50%; position:absolute; top:50%` | — |
| `.thx-btn` | `display:inline-block; padding:.5rem 1.5rem; text-align:center; text-transform:uppercase` | `background-color:#44d62c; border-radius:3px; color:#000` |
| `.thx-btn:hover` | — | `opacity:.8` |
| `.thx-btn:active` | — | `opacity:.6` |
| `.thx-btn.disabled` | `cursor:default` | `opacity:.3` |
| `.thx-btn.secondary` | — | `background-color:#707070; color:#fff` |

由此可推出的界面排布：

```
.main-container（纵向，底 #222）
├─ .nav-tabs           顶栏，最小高 48px，下边线 2px #000，文字 #5d5d5d
└─ .body-wrapper（内边距 10/20/20）
   ├─ .widget-prod      顶部产品图区，高 250px，宽 1024–1220px，图片绝对居中
   └─ .body-widgets     横向排列 + 自动换行 + 居中，最大宽 1240px
      ├─ .widget        固定 600px 宽，内边距 30/40，底 #111，圆角 5px，字号 14px
      ├─ .widget
      └─ .widget-col    600px 宽的列，内部再纵向堆叠
```

> 关键数值：**卡片固定 600px、容器最大 1240px、产品图区高 250px、控件圆角 3px、卡片圆角 5px**。

### 2.0.1 全局样式

**调色板**（按出现次数排序，共统计 1844 处颜色声明）：

| 次数 | 颜色 | 角色 |
|---:|---|---|
| 289 | `#44d62c` | 主色 / 激活态 / 标题 |
| 275 | `#ccc` | 主文字 |
| 157 | `#5d5d5d` | **通用边框** |
| 139 | `#111` | 卡片 / 输入框底色 |
| 132 | `#000` | 按钮文字、描边、顶栏下边线 |
| 108 | `#0000` | 透明 |
| 80 | `#707070` | 次要按钮底色 |
| 70 | `#999` | 次要文字 |
| 68 | `#fff` | 次要按钮文字 |
| 63 | `#fd8611` | 橙色：提示 / 警告 / 分享 |
| 59 | `#222` | 页面底色 / 顶栏底色 |
| 30 | `#ffffff1a` | 半透明白：hover 覆盖 |
| 25 | `#0000004d` | 半透明黑：按钮描边 |
| 23 | `#212121` | 深色文字（浅底上）/ 离线指示 |
| 19 | `#ffffff4d` | 半透明白：按下覆盖 |
| 15 | `#2d2d2d` | **hover 底色** |
| 15 | `#fd4949` | 危险红（亮） |
| 14 | `#c8323c` | 危险红 |
| 14 | `#333` | 开关关闭态底色 |
| 11 | `#515151` | **下拉框边框** |
| 11 | `#4a4a4a` | 帮助图标底色 |
| 10 | `#292929` |  |

**字体与圆角**（同样按出现次数排序）：

| 项 | 值 | 说明 |
|---|---|---|
| 正文字体 | `Roboto, sans-serif` | `body,html` 全局设置 |
| 标题字体 | `RazerF5` | 区块标题（如 `.key-config .body .heading`） |
| 正文字号 | `14px` | 出现 251 次，设备页里最常用 |
| 小字号 | `12px` | 出现 76 次，次要按钮与说明 |
| 全局字号 | `16px` | `body,html`；进入 `.widget` 后变为 14px |
| 控件圆角 | `3px` | 出现 89 次 |
| 卡片圆角 | `5px` | 出现 43 次 |
| 窗口最小尺寸 | `min-width:600px` / `min-height:720px` | `.main-container` / `body,html` |

> ⚠️ 通用边框是 **`#5d5d5d`**（出现 162 次），不是 `#555`（仅 7 次）。
> hover 底色是 **`#2d2d2d`**；下拉框边框是 **`#515151`**。

### 2.1 该界面的分区（布局 + 样式）

下表把该界面的类名按语义归入 UI 分区，并给出它们在 CSS 里的**实际布局与样式声明**。
分区顺序即界面自上而下 / 自左而右的排布。

#### Razer DeathAdder V3 Pro（productId 182）

该界面共 34 个布局类名，分 3 个分区。

**① 电量与充电**（10）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `battery` | `align-items:center; display:flex; height:46px; justify-content:center` | `color:#ccc; font-size:14px`<br>`color:#c8323c` |
| `battery-header` | `display:flex; position:relative` | — |
| `battery-health` | — | `opacity:1` |
| `battery-help-container` | `left:32px; position:relative; top:-8px` | — |
| `battery-level` | `height:10px; margin:5px 0` | `background-image:url(../../static/media/icon_battery_graph.5140e4be.svg); background-repeat:no-repeat; background-size:cover` |
| `battery-percent` | `display:flex; justify-content:space-between` | — |
| `battery-tooltip` | `display:flex; gap:20px; height:100%`<br>`align-items:center; gap:5px; justify-content:center`<br>`height:20px; width:20px` | `text-align:center`<br>`background-position:50%; background-repeat:no-repeat; background-size:20px` |
| `battery-value` | — | — |
| `rapid-trigger-slider-battery` | `left:50%; width:30%` | — |
| `slider-battery` | `left:50%; width:30%` | — |

**② 省电（闲置休眠 / 低电量）**（16）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `FirmwareFactoryReset_poweroff-text__tkcSU` | `margin:10px auto 16px` | `color:#999; text-align:center` |
| `audio-power-saving` | `min-width:78px` | — |
| `device--cta-power` | `height:24px; width:24px`<br>`bottom:54%; left:6%` | `background-color:#111; background:url(../../static/media/power-on-btn.aaab82c7.svg); background-position:50%; background-repeat:no-repeat; border:1px solid #0000; border-radius:50%`<br>`border-color:#44d62c` |
| `device--cta-power-off` | — | `background:url(../../static/media/power-off-btn.32ef0657.svg)` |
| `icon-power` | `height:20px; position:relative; width:20px` | `transition:all .3s`<br>`fill:#44d62c`<br>`fill:#111`<br>`fill:#7de36c` |
| `icon-power--power-off` | — | `fill:#c8323c`<br>`fill:#d97077` |
| `idleEffect` | — | `opacity:1`<br>`opacity:.3` |
| `mouse-power-saving` | `min-width:78px; padding:7px 16px` | `color:#ccc; font-size:12px; line-height:14px` |
| `power-icons` | `display:flex; justify-content:center` | — |
| `power-off` | — | `opacity:1`<br>`opacity:.5` |
| `power-saving-effect` | `display:flex; flex-direction:column; position:relative`<br>`margin:20px 0 10px`<br>`display:flex`<br>`width:-webkit-fit-content; width:fit-content` | `color:#ccc`<br>`opacity:1` |
| `powerSaving-desc` | — | — |
| `powerSaving-select` | `display:flex; gap:10px` | — |
| `powerSaving-value` | `align-items:center; display:flex; justify-content:center; min-width:90px; padding:7px 30px`<br>`min-width:78px`<br>`min-width:78px; padding:7px 16px`<br>`padding:4px 36px` | `background:#111; border:1px solid #5d5d5d; border-radius:3px; color:#fff; font-size:12px; line-height:12px`<br>`color:#ccc; font-size:12px; line-height:14px`<br>`border:1px solid #44d62c; cursor:pointer`<br>`background:#292929; border-color:#44d62c` |
| `powerSaving-wrap--disabled` | — | `opacity:30%` |
| `system-power-saving` | `padding:4px 36px` | — |

**③ 熄灯 / 调暗**（8）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `DeviceCard_dimmedContent__dlMvE` | — | `opacity:.3` |
| `DimKeyboardLighting_backdrop__WXv8r` | `height:30px; position:absolute; width:300px` | `background-color:#111; opacity:.5` |
| `DimKeyboardLighting_width-auto__C0C3F` | `width:48px!important` | — |
| `custom-dim-corner` | `height:100%; width:100%` | `background:radial-gradient(#0000,#222)` |
| `dim` | — | `color:#707070` |
| `dim-corner` | `height:100%; left:0; position:absolute; top:0; width:100%`<br>`min-width:0; min-width:auto` | `background:radial-gradient(#0000,#222)` |
| `info-dimmed` | — | `color:#ccc; opacity:.3` |
| `warning-dimmed` | — | `color:#c8323c; opacity:.3` |

#### Blackwidow V4 Pro（productId 653）

该界面共 33 个布局类名，分 3 个分区。

**① 电量与充电**（10）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `battery` | `align-items:center; display:flex; height:46px; justify-content:center` | `color:#ccc; font-size:14px`<br>`color:#c8323c` |
| `battery-header` | `display:flex; position:relative` | — |
| `battery-health` | — | `opacity:1` |
| `battery-help-container` | `left:32px; position:relative; top:-8px` | — |
| `battery-level` | `height:10px; margin:5px 0` | `background-image:url(../../static/media/icon_battery_graph.5140e4be.svg); background-repeat:no-repeat; background-size:cover` |
| `battery-percent` | `display:flex; justify-content:space-between` | — |
| `battery-tooltip` | `display:flex; gap:20px; height:100%`<br>`align-items:center; gap:5px; justify-content:center`<br>`height:20px; width:20px` | `text-align:center`<br>`background-position:50%; background-repeat:no-repeat; background-size:20px` |
| `battery-value` | — | — |
| `rapid-trigger-slider-battery` | `left:50%; width:30%` | — |
| `slider-battery` | `left:50%; width:30%` | — |

**② 省电（闲置休眠 / 低电量）**（16）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `FirmwareFactoryReset_poweroff-text__tkcSU` | `margin:10px auto 16px` | `color:#999; text-align:center` |
| `audio-power-saving` | `min-width:78px` | — |
| `device--cta-power` | `height:24px; width:24px`<br>`bottom:54%; left:6%` | `background-color:#111; background:url(../../static/media/power-on-btn.aaab82c7.svg); background-position:50%; background-repeat:no-repeat; border:1px solid #0000; border-radius:50%`<br>`border-color:#44d62c` |
| `device--cta-power-off` | — | `background:url(../../static/media/power-off-btn.32ef0657.svg)` |
| `icon-power` | `height:20px; position:relative; width:20px` | `transition:all .3s`<br>`fill:#44d62c`<br>`fill:#111`<br>`fill:#7de36c` |
| `icon-power--power-off` | — | `fill:#c8323c`<br>`fill:#d97077` |
| `idleEffect` | — | `opacity:1`<br>`opacity:.3` |
| `mouse-power-saving` | `min-width:78px; padding:7px 16px` | `color:#ccc; font-size:12px; line-height:14px` |
| `power-icons` | `display:flex; justify-content:center` | — |
| `power-off` | — | `opacity:1`<br>`opacity:.5` |
| `power-saving-effect` | `display:flex; flex-direction:column; position:relative`<br>`margin:20px 0 10px`<br>`display:flex`<br>`width:-webkit-fit-content; width:fit-content` | `color:#ccc`<br>`opacity:1` |
| `powerSaving-desc` | — | — |
| `powerSaving-select` | `display:flex; gap:10px` | — |
| `powerSaving-value` | `align-items:center; display:flex; justify-content:center; min-width:90px; padding:7px 30px`<br>`min-width:78px`<br>`min-width:78px; padding:7px 16px`<br>`padding:4px 36px` | `background:#111; border:1px solid #5d5d5d; border-radius:3px; color:#fff; font-size:12px; line-height:12px`<br>`color:#ccc; font-size:12px; line-height:14px`<br>`border:1px solid #44d62c; cursor:pointer`<br>`background:#292929; border-color:#44d62c` |
| `powerSaving-wrap--disabled` | — | `opacity:30%` |
| `system-power-saving` | `padding:4px 36px` | — |

**③ 熄灯 / 调暗**（7）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `DimKeyboardLighting_backdrop__WXv8r` | `height:30px; position:absolute; width:300px` | `background-color:#111; opacity:.5` |
| `DimKeyboardLighting_width-auto__C0C3F` | `width:48px!important` | — |
| `custom-dim-corner` | `height:100%; width:100%` | `background:radial-gradient(#0000,#222)` |
| `dim` | — | `color:#707070` |
| `dim-corner` | `height:100%; left:0; position:absolute; top:0; width:100%`<br>`min-width:0; min-width:auto`<br>`min-width:770px` | `background:radial-gradient(#0000,#222)` |
| `info-dimmed` | — | `color:#ccc; opacity:.3` |
| `warning-dimmed` | — | `color:#c8323c; opacity:.3` |

#### RAZER KRAKEN BT SANRIO LIMITED EDITION（productId 777）

该界面共 30 个布局类名，分 3 个分区。

**① 电量与充电**（9）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `battery` | `align-items:center; display:flex; height:46px; justify-content:center` | `color:#ccc; font-size:14px`<br>`color:#c8323c` |
| `battery-header` | `display:flex; position:relative` | — |
| `battery-health` | — | `opacity:1` |
| `battery-help-container` | `left:32px; position:relative; top:-8px` | — |
| `battery-level` | `height:10px; margin:5px 0` | `background-image:url(../../static/media/icon_battery_graph.5140e4be.svg); background-repeat:no-repeat; background-size:cover` |
| `battery-percent` | `display:flex; justify-content:space-between` | — |
| `battery-tooltip` | `display:flex; gap:20px; height:100%`<br>`align-items:center; gap:5px; justify-content:center`<br>`height:20px; width:20px` | `text-align:center`<br>`background-position:50%; background-repeat:no-repeat; background-size:20px` |
| `battery-value` | — | — |
| `slider-battery` | `left:50%; width:30%` | — |

**② 省电（闲置休眠 / 低电量）**（16）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `FirmwareFactoryReset_poweroff-text__tkcSU` | `margin:10px auto 16px` | `color:#999; text-align:center` |
| `audio-power-saving` | `min-width:78px` | — |
| `device--cta-power` | `height:24px; width:24px`<br>`bottom:54%; left:6%` | `background-color:#111; background:url(../../static/media/power-on-btn.aaab82c7.svg); background-position:50%; background-repeat:no-repeat; border:1px solid #0000; border-radius:50%`<br>`border-color:#44d62c` |
| `device--cta-power-off` | — | `background:url(../../static/media/power-off-btn.32ef0657.svg)` |
| `icon-power` | `height:20px; position:relative; width:20px` | `transition:all .3s`<br>`fill:#44d62c`<br>`fill:#111`<br>`fill:#7de36c` |
| `icon-power--power-off` | — | `fill:#c8323c`<br>`fill:#d97077` |
| `idleEffect` | — | `opacity:1`<br>`opacity:.3` |
| `mouse-power-saving` | `min-width:78px; padding:7px 16px` | `color:#ccc; font-size:12px; line-height:14px` |
| `power-icons` | `display:flex; justify-content:center` | — |
| `power-off` | — | `opacity:1`<br>`opacity:.5` |
| `power-saving-effect` | `display:flex; flex-direction:column; position:relative`<br>`margin:20px 0 10px`<br>`display:flex`<br>`width:-webkit-fit-content; width:fit-content` | `color:#ccc`<br>`opacity:1` |
| `powerSaving-desc` | — | — |
| `powerSaving-select` | `display:flex; gap:10px` | — |
| `powerSaving-value` | `align-items:center; display:flex; justify-content:center; min-width:90px; padding:7px 30px`<br>`min-width:78px`<br>`min-width:78px; padding:7px 16px`<br>`padding:4px 36px` | `background:#111; border:1px solid #5d5d5d; border-radius:3px; color:#fff; font-size:12px; line-height:12px`<br>`color:#ccc; font-size:12px; line-height:14px`<br>`border:1px solid #44d62c; cursor:pointer`<br>`background:#292929; border-color:#44d62c` |
| `powerSaving-wrap--disabled` | — | `opacity:30%` |
| `system-power-saving` | `padding:4px 36px` | — |

**③ 熄灯 / 调暗**（5）

| 类名 | 布局 | 样式（颜色 / 圆角 / 字号 / 字体 / 状态） |
|---|---|---|
| `custom-dim-corner` | `height:100%; width:100%` | `background:radial-gradient(#0000,#222)` |
| `dim` | — | `color:#707070` |
| `dim-corner` | `height:100%; left:0; position:absolute; top:0; width:100%`<br>`min-width:0; min-width:auto` | `background:radial-gradient(#0000,#222)` |
| `info-dimmed` | — | `color:#ccc; opacity:.3` |
| `warning-dimmed` | — | `color:#c8323c; opacity:.3` |

## 3. 功能项

该界面对应的雷云文案 key，共 **90** 条（中文为雷云原文）：

| key | 中文 | English |
|---|---|---|
| `BATTERY_CHARGED_FULL` | 100% 已充满电 | 100% charged |
| `BATTERY_CHARGING` | 电池正在充电 | Battery Charging |
| `BATTERY_CHARGING_OVERRIDE_CONTENT` | 让系统充满电一次。 | Allow system to fully charge once. |
| `BATTERY_CHARGING_OVERRIDE_WARNING_CONTENT` | 电池充满电后，电池健康优化功能会重新启用 | Battery Health Optimizer is re-enabled when battery is fully charged |
| `BATTERY_CONTENT` | 电池在达到上限 (%) 时将停止充电。 | Battery will stop charging when it has reached the limit (%). |
| `BATTERY_ERROR_TIPS` | 系统不支持所安装的电池，并可能会因此受到损坏。 请使用系统支持的 Razer 雷蛇电池更换此电池。 | The battery installed is not supported and may cause damage to the system. Replace the battery with a supported Razer battery for this system. |
| `BATTERY_HEALTH_OPTIMIZER` | 电池健康优化功能 | Battery health optimizer |
| `BATTERY_HEALTH_OPTIMIZER_MOUSEMAT_DESCRIPTION` | 当达到设定的百分比时，鼠标垫将停止为设备充电。 | When the set percentage has been reached, the mouse mat will stop charging the device. |
| `BATTERY_HEALTH_OPTIMIZER_MOUSEMAT_TOOLTIP` | 要启用此功能，必须配对兼容的 Razer 雷蛇鼠标并将其设置为 HyperSpeed Wireless 无线模式。 | To enable this feature, a compatible Razer mouse must be paired and set to HyperSpeed Wireless mode. |
| `BATTERY_LEVEL` | 电池电量 | Battery Level |
| `BATTERY_LEVEL_EFFECT` | 电池: {{percentageBattery}}% | Battery: {{percentageBattery}}% |
| `BATTERY_LEVEL_STATUS` | 电池电量状态 | Battery Level Status |
| `BATTERY_LEVEL_STATUS_TOOLTIP` | 即使未在充电，电池电量也会与所选设备保持同步。雷云必须运行，此设置才能持续生效。 | The battery level will remain synced with the selected device even when not charging. Razer Synapse must be running for this setting to stay active. |
| `BATTERY_LIFE` | 电池电量 | BATTERY LIFE |
| `BATTERY_OFF` | 关闭 | off |
| `BATTERY_PERCENT` | {{level}}% 电池 | {{level}}% Battery |
| `BATTERY_REFRESH_RATE` | 使用电池时刷新率 | Battery Refresh Rate |
| `BATTERY_REFRESH_RATE_TITTLE` | 以电池供电时，请将笔记本电脑的屏幕刷新率切换为 60Hz。 | Switch laptop screen refresh rate to 60Hz when on battery. |
| `BATTERY_SELECTION` | 电池选择 | BATTERY SELECTION |
| `BATTERY_SELECTION_DESC` | 选择你使用的电池类型以获得更准确的电池电量指示。 | Select the type of battery you're using for more accurate battery level indication. |
| `BATTERY_SELECTION_TOOLTIP` | 提高电池电量指示的准确性。 | Enhances the accuracy of the battery level indication. |
| `BATTERY_STATUS` | 电池状态 | Battery Status |
| `BATTERY_STATUS_AUDIO_DESC` | 根据设备当前的电池电量，从绿色 (100%)、黄色 (50%) 和红色 (8%) 逐渐变化。 | Gradually changes from Green (100%), Yellow (50%), and Red (8%) depending on the current battery level of your device. |
| `BATTERY_STATUS_DESC` | 根据设备当前的电池电量，从绿色 (100%)、黄色 (66%)、橙色 (33%) 和红色 (0%) 逐渐变化。 | Gradually changes from Green (100%), Yellow (66%), Orange (33%), and Red (0%) depending on the current battery level of your device. |
| `BATTERY_STATUS_KEYBOARD_DESC` | 根据设备当前的电池电量，从绿色 (100%)、黄色 (66%)、橙色 (33%) 和红色 (8%) 逐渐变化。 | Gradually changes from Green (100%), Yellow (66%), Orange (33%) and Red (8%) depending on the current battery level of your device. |
| `BATTERY_TOOLTIP` | 将最大可充电量限制在 80% 或更低，以此来延长电池保持健康的时间。<br><br>将电池的最大可充电量设置为 50-80%。此值设置得较低可延长电池寿命，但需要频繁充电。 | Extend the health of your battery by <br>limiting the maximum charging capacity to <br>80% or less.<br><br>Set a maximum battery charge level from 50-80%. Lower charge limit will prolong <br>battery life but requires frequent charging <br>times. |
| `BATTERY_TYPE_AKALINE` | 碱性电池 | Alkaline |
| `BATTERY_TYPE_LITHIUM` | 锂离子电池 | Lithium |
| `BATTERY_TYPE_RECHARGEABLE_NIMH` | 可充电镍氢电池 | Rechargeable NiMh |
| `BATTERY_WARNING_ONLY` | 仅电池警告 | Battery Warning Only |
| `BRIGHTNESS_WHEN_INACTIVE` | 启用时的亮度 | Brightness when inactive |
| `DIM_KEYBOARD_LIGHTING_DESC` | 以电池供电时，在无活动（分钟）后，设备将会变暗。 | Device will turn dim after (mins) of inactivity when running on battery. |
| `DIM_KEYBOARD_LIGHTING_TIPS` | 设置设备闲置多长时间后调暗灯光效果。<br><br> 当设备使用无线连接且不处于充电状态时，调暗灯光功能可起作用。 | Set how long the device should be idle before it will dim the lighting.<br><br>This dim lighting function only works when the device using wireless connection and not being charged. |
| `DIM_LIGHTING_DESC` | 处于无线模式时，在闲置以下时间（分钟）后调暗灯光效果 | When wireless, dim lighting if idle for (minutes) |
| `DIM_LIGHTING_DESC_V2` | 系统在闲置（分钟）后，设备亮度将降至 20%。 | Device dims to 20% brightness after (mins) of system inactivity. |
| `DIM_LIGHTING_HEADER` | 暗光效果 | DIM LIGHTING |
| `DIM_LIGHTING_TOOLTIP` | 设置设备闲置多长时间后调暗灯光效果  | Set how long the device should be idle before it will dim the lighting  |
| `DIM_LIGHTING_TOOLTIP_V2` | 选择系统闲置多长时间后亮度会降至 20%。<br><br>此功能仅在设备亮度高于 20% 时激活。<br><br>当 Razer Synapse 雷云运行时，亮度会随系统活动自动恢复。若没有恢复，请按下设备上的任意按键以恢复亮度。<br><br>这些设置保存在设备上，并应用于所有配置文件。 | Choose how long the system should remain idle before the lighting dims to 20% brightness.<br><br>This feature only activates when the device brightness is set above 20%.<br><br>Brightness restores with system activity when Razer Synapse is running. Otherwise, press any button on the device to restore it.<br><br>These settings are saved to your device and are applied to all profiles. |
| `IDLE_EFFECT` | 闲置效果 | IDLE EFFECT |
| `IDLE_EFFECT_DESC` | 当充电板空闲或未为设备充电时会激活此灯光效果。 | This effect is active when the charging pad is idle or not charging a device. |
| `IDLE_EFFECT_TOOLTIP` | 自定义充电板空闲或未为设备充电时的灯光效果。 这些设置存储在充电板上。即使 Razer Synapse 雷云没有运行，只要设备有插入，那么就算设备处于空闲状态，这些设置仍会继续生效。 | Customize the lighting effect when the charging pad is idle or not charging a device. These settings are stored on the charging pad and will remain active while idle (and plugged in), even when Razer Synapse is not running. |
| `IDLE_FOR_MIN` | 当闲置超过以下时长（分钟）时 | When idle for (minutes) |
| `LOW_POWER_MODE_DESC` | 处于无线模式时，当电池电量低于以下百分比 (%) 时进入低能耗模式 | When wireless, enter Low Power Mode if the battery level is below (%) |
| `LOW_POWER_MODE_DESC_V2` | 处于无线模式时，当电池电量低于以下百分比 (%) 时进入低能耗模式 | When wireless, enter Low Power Mode if the battery level is below (%) |
| `LOW_POWER_MODE_HEADER` | 低能耗模式 | LOW POWER MODE |
| `LOW_POWER_MODE_HEADER_V2` | 低能耗模式 | LOW POWER MODE |
| `LOW_POWER_MODE_TOOLTIP` | 当设备进入低能耗模式后，设备的追踪速度和传感器加速会自动降低以节省电力。 | Once the device enters Low Power Mode, the device's tracking speed and sensor acceleration are automatically reduced to conserve battery life. |
| `LOW_POWER_MODE_WARN` | 当轮询率为 2000 Hz 及以上时，不支持低能耗模式。 | Low Power Mode is not supported at polling rates of 2000 Hz and above. |
| `POWER_BUTTON_SHORTCUTS_2_SECS` | 2 秒 | 2 secs |
| `POWER_BUTTON_SHORTCUTS_BLUETOOTH_NOTE` | 适用于连接的蓝牙移动设备。 | Works on connected Bluetooth mobile devices. |
| `POWER_BUTTON_SHORTCUTS_DURING_CALL` | 通话期间 | during call |
| `POWER_BUTTON_SHORTCUTS_END_CALL` | 挂断电话 | End call |
| `POWER_BUTTON_SHORTCUTS_INCOMING_CALL` | 来电 | incoming call |
| `POWER_BUTTON_SHORTCUTS_LONG_PRESS_2_SECS` | 长按 2 秒 | Long Press 2 Secs |
| `POWER_BUTTON_SHORTCUTS_REJECT_CALL` | 拒绝来电 | Reject call |
| `POWER_BUTTON_SHORTCUTS_SWITCH_CALLS_3_WAY` | 切换通话<br>（3 向通话） | Switch calls<br>(3-way calls) |
| `POWER_BUTTON_SHORTCUTS_TITLE` | 电源按钮快捷方式 | Power Button Shortcuts |
| `POWER_BUTTON_SHORTCUTS_TRIPLE_PRESS` | 连按三下 | Triple Press |
| `POWER_INDICATOR` | 电源指示灯 | POWER INDICATOR |
| `POWER_INDICATOR_BATTERY_WARNING_ONLY_DESC` | 始终保持熄灭，仅在设备需要充电时闪烁红色。 | Remains off at all times and only displays blinking red when the device needs charging. |
| `POWER_INDICATOR_CONNECTION_STATUS_DESC` | 只有当设备连接到接收器时才会点亮并保持白色常亮。 | Only active and solid white when a device is connected to the dongle. |
| `POWER_INDICATOR_CONNECTION_STATUS_DESC_2` | 指示与接收器的连接状态：绿色 (2.4GHz)、白色（同步音频）或熄灭（未连接）。 | Indicates connection status to the dongle: Green (2.4GHz), White (Simultaneous), or Off (not connected). |
| `POWER_ON` | 打开/关闭电源 | Power ON/OFF |
| `POWER_ON_OFF` | 打开/关闭电源 | Power On/Off |
| `POWER_SAVING` | 节能 | POWER SAVING |
| `POWER_SAVING_DESC` | 在闲置以下时间（分钟）后进入睡眠模式 | Enter sleep mode if idle for (minutes) |
| `POWER_SAVING_DESC_V2` | 处于无线模式时，在闲置以下时间（分钟）后进入睡眠模式 | When wireless, enter sleep mode if idle for (minutes) |
| `POWER_SAVING_DESC_V3` | 在闲置以下时间（分钟）后进入待机模式 | Enter standby mode if idle for (minutes) |
| `POWER_SAVING_DESC_V4` | 在闲置以下时间后进入睡眠模式 | Enter sleep mode if idle for |
| `POWER_SAVING_DESC_V5` | Razer Clio 雷蛇悦神未播放音频以下时间（分钟）后将会关闭设备 | Device will turn off when there's no audio playback on Razer Clio after (mins) |
| `POWER_SAVING_DESC_V6` | 当系统闲置超过（分钟）时进入睡眠模式。 | Enter sleep mode when the system is idle for (minutes). |
| `POWER_SAVING_DESC_V7` | {{deviceName}} 未播放音频以下时间（分钟）后将会关闭设备 | Device will turn off when there's no audio playback on {{deviceName}} after (mins) |
| `POWER_SAVING_EFFECT_DESC` | 以电池供电时使用功耗已优化的光谱循环效果。 | Use power-optimized Spectrum Cycling when running on battery. |
| `POWER_SAVING_EFFECT_DESC_2` | 使用电池供电时，降低灯光效果的变换频率，以节省电量。 | Reduce the effect's refresh rates when on battery to conserve power. |
| `POWER_SAVING_EFFECT_TIP` | 以电池供电时，使用功耗已优化的光谱循环效果以节省电量。<br> <br> 注意：由于此效果独立运行，其灯光效果可能与标准光谱循环效果有所不同，而且不会与其他支持 Razer Chroma 雷蛇幻彩技术的设备同步。 | Save power while running on battery using power-optimized Spectrum Cycling. <br> <br> Note: As this effect runs independently, it may look different from the standard Spectrum Cycling, and will not sync with other Razer Chroma-enabled devices. |
| `POWER_SAVING_HEADER` | 无线节能 | WIRELESS POWER SAVING |
| `POWER_SAVING_MODE` | 激活节能模式 | Activate Power Saving Mode |
| `POWER_SAVING_MODE_WARNING_DESC_1` | 由于设备当前处于节能模式，因此无法使用 Synapse 雷云设置。 | Synapse settings are inaccessible as your device is currently in Power Saving Mode. |
| `POWER_SAVING_MODE_WARNING_DESC_2` | 要自定义 Synapse 雷云设置，只需长按电池寿续航时间/状态按键 3 秒。 | To customize Synapse settings, simply press and hold the Battery life / status button for 3 seconds. |
| `POWER_SAVING_MODE_WARNING_DESC_3` | 如要自定义 Synapse 雷云设置，只需长按电池电量按键 3 秒。 | To customize Synapse settings, simply press and hold the Battery level button for 3 seconds. |
| `POWER_SAVING_MODE_WARNING_TITLE` | 节能模式 | Power Saving Mode |
| `POWER_SAVING_SUB_DESC` | 注：你可以通过短按设备的电源键，或使用鼠标或键盘唤醒设备。 | Note: You can wake the device with a short press of its power button or by using your mouse or keyboard. |
| `POWER_SAVING_TOOLTIP` | 设置设备闲置多长时间后进入睡眠模式。 | Set how long the device should be idle before it enters sleep mode. |
| `POWER_SAVING_TOOLTIP_V2` | 设置设备闲置多长时间后进入待机模式。 | Set how long the device should be idle before it enters standby mode. |
| `POWER_SAVING_TOOLTIP_V3` | 设置系统闲置多长时间后设备进入睡眠模式。<br><br>该设备还将随你的电脑自动进入睡眠和唤醒状态，或在你使用鼠标或键盘时自动响应。 | Set how long the system should be idle before the device enters sleep mode.<br><br>The device will also sleep and wake automatically with your PC or when you use your mouse or keyboard. |
| `POWER_SAVING_WARNING` | 启用“麦克风监听（侧音）”时，设备不会进入无线节能模式。 | Your device will not enter Wireless Power Saving mode while Mic Monitoring (Sidetone) is enabled. |
| `POWER_SAVING_WARNING_V1` | 当连接到运行 Razer Synapse 雷云的电脑时，Razer Clio 雷蛇悦神会保持开启状态，直至系统关机、休眠或进入睡眠模式。 | When connected to a PC with Razer Synapse, Razer Clio remains on until the system shuts down, hibernates, or goes to sleep. |
| `POWER_SAVING_WARNING_V2` | 当连接到运行 Razer Synapse 雷云的电脑时，{{deviceName}} 会保持开启状态，直至系统关机、休眠或进入睡眠模式。 | When connected to a PC with Razer Synapse, {{deviceName}} remains on until the system shuts down, hibernates, or goes to sleep. |
| `POWER_TABLE_INFO` | 功耗 | Power |
| `SLEEP` | 睡眠 | Sleep |
