# 电源（`TAB_POWER`）

> 本页只记录 `.ref` 中雷云产品模块已经证明的事实，不记录本项目 Rust 实现状态。功能必须以产品模块的 JS 渲染分支和 CSS 为准；仅存在于 locale 的文案不等于页面一定显示。

## 1. 页面归属

| productId | 设备 | 是否有电源标签 | 证据 |
|---:|---|---|---|
| `182` | Razer DeathAdder V3 Pro | 有 | `.ref/devices/182/static/js/main.db20a7c4.js` 的标签常量与设备路由；`.ref/devices/182/manifest.json` |
| `653` | Blackwidow V4 Pro | 有 | `.ref/devices/653/static/js/main.7b71cce5.js` 的标签常量与设备默认配置；`.ref/devices/653/manifest.json` |
| `777` | RAZER KRAKEN BT SANRIO LIMITED EDITION | 有 | `.ref/devices/777/static/js/main.eb70ce38.js` 的标签常量与设备路由；`.ref/devices/777/manifest.json` |

三个设备模块都导出 `TAB_POWER`，但电源控件的具体可用状态由设备连接方式、无线状态、轮询率和设备能力决定，不能把鼠标、键盘、耳机的默认配置互相套用。

## 2. 外层布局

电源页使用设备模块通用页面骨架；电量状态本身还会出现在产品顶栏右侧，不能误画成电源页正文卡片。

| 选择器 | 确切布局 | 确切样式 |
|---|---|---|
| `body, html` | `height:100%; width:100%; min-height:720px; max-width:1920px; overflow:hidden; margin:0` | `Roboto, sans-serif; font-size:16px; background:#222; color:#ccc; user-select:none` |
| `.main-container` | `display:flex; flex-direction:column; position:absolute; width:100%; height:100%; min-width:600px` | `background:#222` |
| `.nav-tabs` | `display:flex; align-items:center; position:relative; width:100%; min-height:48px; z-index:105` | `background:#222; border-bottom:2px solid #000; color:#5d5d5d` |
| `.body-wrapper` | `flex:1 1; width:100%; height:100%; min-width:600px; padding:10px 20px 20px` | 默认可滚动；页面状态为禁止滚动时使用 `.no-scroll` |
| `.body-widgets` | `display:flex; flex-direction:row; flex-wrap:wrap; justify-content:center; max-width:1240px; margin:auto` | 无额外背景 |
| `.body-widgets .widget` | `flex:0 0 auto; min-width:600px; max-width:600px; height:auto; margin:10px auto; padding:30px 40px; position:relative` | `background:#111; border-radius:5px; font-size:14px` |
| `.widget-col` | `display:flex; flex-direction:column; width:600px; height:fit-content` | — |

### 2.1 控件通用状态

以下是电源页实际引用的共享控件样式，不代表每个设备都显示所有控件：

| 控件/状态 | 样式与行为 |
|---|---|
| `.powerSaving-value` | `display:flex; align-items:center; justify-content:center; min-width:90px; padding:7px 30px; background:#111; border:1px solid #5d5d5d; border-radius:3px; color:#fff; font-size:12px; line-height:12px` |
| `.powerSaving-value:hover` | 边框变为 `#44d62c`，光标为 pointer |
| `.powerSaving-value.active` | `background:#292929; border-color:#44d62c` |
| `.powerSaving-select` | `display:flex; gap:10px; margin-top:20px`；仅在产品使用离散省电选项时出现 |
| `.powerSaving-wrap--disabled` | `opacity:30%; pointer-events:none; user-select:none` |
| `.powerSaving-desc` | `margin-top:20px` |
| `.power-saving-effect` | `display:flex; flex-direction:column; position:relative; z-index:-1`；标题 `margin:20px 0 10px`，内容横向排列 |
| `.power-saving-effect .content .text` | `color:#ccc; margin-bottom:15px; padding-right:15px` |
| `.warning-text` | 由设备条件决定是否插入；不能在默认页面中始终显示 |

## 3. 正文功能与状态

### 3.1 共享的省电设置组件

三个模块都包含同一组省电状态动作：`setPowerSaving`、`setLowPowerMode`、`loadPowerSavingFromDevice`、`setPowerSavingMode`、`toggleRequirePowerWarning`。真正渲染的两个主要组件在压缩 JS 中分别表现为：

1. **省电时间组件**：标题使用 `POWER_SAVING_HEADER`，描述使用设备条件对应的 `POWER_SAVING_DESC*`，写入 `powerSavingValue`。
2. **低电量/低功耗组件**：标题使用 `POWER_MODE_HEADER*` 或对应低功耗文案，写入 `lowPowerMode`；当设备的无线轮询率不满足要求时，控件不可用并显示 warning 文案。

源码证据：

- `.ref/devices/182/static/js/main.db20a7c4.js`：`class CM` 渲染 `powerSaving-select` 或步进滑块；`class pM` 渲染 `lowPowerMode` 滑块并按 polling rate 判断 `active`。
- `.ref/devices/653/static/js/main.7b71cce5.js`：同一共享组件和键盘设备默认配置，默认 `sleepModeInSeconds:15`、`lowPowerMode:30`。
- `.ref/devices/777/static/js/main.eb70ce38.js`：同一共享动作；耳机模块同时拥有 `earbudBatteryState` 和左右电池状态，不应按鼠标单电池文案实现。

### 3.2 productId 182：DeathAdder V3 Pro

设备配置中的初始值为 `powerSavingValue:5`、`lowPowerMode:5`，但初始值不是 UI 范围的唯一证据。JS 明确证明的交互是：

| 控件 | 真实行为 |
|---|---|
| 省电时间 | 当 `usePowerSavingButton` 为真时，显示三个离散按钮：`15`、`30`、`45` 分钟；按钮通过 `.powerSaving-select` 横向排列，当前值使用 `.active`。否则走共享滑块组件。 |
| 低功耗 | 滑块范围 `5–100`，步长 `5`，显示 `5%–100%`；`isEnabled()` 在无线/蓝牙模式使用无线轮询率，否则使用有线轮询率，并要求 `<=1000`。 |
| 低功耗不可用 | 不满足轮询率条件时，滑块使用 inactive 状态，并插入 warning 文案；不能仍画成可操作的绿色控件。 |
| 状态保存 | 改变省电值派发 `SET_POWER_SAVING_VALUE`；改变低功耗值派发 `SET_LOW_POWER_MODE`，并通过本地状态机/广播同步。 |

### 3.3 productId 653：Blackwidow V4 Pro

设备 manifest/JS 默认配置明确包含：`sleepModeInSeconds:15`、`lowPowerMode:30`、`ledPowerSettings.idleValue:60`。这些字段证明设备支持睡眠/低功耗和灯光相关的设备状态，但不能据此额外添加“电池健康度”“鼠标抬升距离”等控件。

页面实现应保留：

- 电源标签和共享电源设置容器；
- 与键盘设备状态相匹配的低功耗/睡眠控件；
- 共享的 disabled、warning、active 状态；
- 如果实际设备状态没有电池值，不显示电池百分比卡片。

源码中确实存在全局 `battery` 顶栏组件和 `getBatteryLevel` 等能力，但这只证明导航栏可以显示电池状态，不证明电源正文一定显示独立电池卡片。

### 3.4 productId 777：KRAKEN BT SANRIO

耳机模块的 manifest 名称和 JS 状态明确使用耳机电池模型：`earbudBatteries`、`earbudBatteryState.left`、`earbudBatteryState.right`。因此电量提示需要支持左右电池状态；不能复用鼠标单值 `batteryValue` 的视觉和文案。

已确认的状态：

- 顶栏电量提示可以显示左、右电池百分比和各自状态图标；
- 电池状态图标来自 `icon_battery_*.svg`、`icon_battery_charging*.svg`、`icon_battery_disconnected.svg`、`icon_battery_error.svg` 等资源；
- 电源页仍使用共享省电动作，但是否显示某个低功耗警告必须服从耳机模块的运行时状态；
- 不应凭 locale 中的鼠标、电池寿命、抬升距离或无线接收器文案增加耳机控件。

源码证据：`.ref/devices/777/static/js/main.eb70ce38.js` 的 `earbudBatteries`/`earbudBatteryState` 顶栏渲染、`powerSavingValue`/`lowPowerMode` 状态动作；`.ref/devices/777/manifest.json` 的设备身份和版本信息。

## 4. 电量状态图标与颜色

电池图标不是纯色占位。实现时必须使用 manifest 中映射的带哈希 SVG 资源，不能用一套自绘图标替代：

- `icon_battery_0.svg`、`icon_battery_10.svg` … `icon_battery_100.svg`：按电量等级切换；
- `icon_battery_charging.svg`、`icon_battery_charging_100.svg`：充电态；
- `icon_battery_disconnected.svg`：断开态；
- `icon_battery_error.svg`：错误态；
- `icon_battery_paused.svg`：暂停/特殊状态。

正文和顶栏的颜色证据来自 `.ref/devices/*/static/css/main.*.css`：页面底色 `#222`、卡片 `#111`、普通文字 `#ccc`、次级文字 `#999`、通用边框 `#5d5d5d`、主色 `#44d62c`、active 背景 `#292929`、低电量文字 `#c8323c`。不要把 SVG 的内部颜色替换成 `#44d62c`；SVG 资源本身的填充/路径颜色优先于外层 CSS。

## 5. 明确删除的无证据内容

以下内容不再作为电源页规格：

- “所有设备都有相同的电池正文卡片”；
- “键盘一定有鼠标式电量百分比和低电量滑块”；
- “耳机显示单一电池值”；
- “存在电池健康度调节、充电上限、功率曲线、瓦数图表”等，仅凭共享 API 名称或 locale 推导的功能；
- 把 `POWER_SAVING_*` locale 全量当作当前页面同时可见的文案；
- 把产品卡片里的电源按钮 `.device--cta-power` 当成电源页控件。它属于设备轮播卡片，CSS 位置为 `24px × 24px`、圆形、默认背景 `#111`，并使用 `power-on-btn.svg`/`power-off-btn.svg`，不应放进电源正文。

## 6. 证据索引

- JS：`.ref/devices/182/static/js/main.db20a7c4.js`
- JS：`.ref/devices/653/static/js/main.7b71cce5.js`
- JS：`.ref/devices/777/static/js/main.eb70ce38.js`
- CSS：`.ref/devices/182/static/css/main.48c20423.css`
- CSS：`.ref/devices/653/static/css/main.5425442a.css`
- CSS：`.ref/devices/777/static/css/main.e4bab2aa.css`
- 设备 manifest：`.ref/devices/182/manifest.json`、`.ref/devices/653/manifest.json`、`.ref/devices/777/manifest.json`
- 资源索引：`.ref/devices/182/asset-manifest.json`、`.ref/devices/653/asset-manifest.json`、`.ref/devices/777/asset-manifest.json`
