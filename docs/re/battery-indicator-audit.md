# 设备页右上角电量（`.right .battery`）依据与本实现

2026-10-03。设备页顶栏右侧原本没有电量块——不是被隐藏，而是从未实现。本轮按当前源码补齐，全部取值都有出处；机器可读结果见 [battery-indicator-audit.json](battery-indicator-audit.json)，抽取脚本 [tools/audit-battery-indicator.cjs](../../tools/audit-battery-indicator.cjs)（支持 `--check`，任何一项对不上就报错）。

## 渲染条件与结构

`.ref/devices/100/static/js/main.1734869e.js` 里设备页顶栏组件：

- 只有 `hasBattery && batteryState !== undefined` 才渲染 `div.battery`（`role="img" id="battery-level-tips" aria-roledescription="battery status"`）；
- 子节点顺序：`<span>{level >= 0 ? "{level} %" : "-"}</span>` → `<div class={batteryState}/>` → 悬停才挂载的 tooltip；
- `level >= 0 && level <= 10` 时 span 额外带 `low-batt`。

负载里没有 `chargingStatus` 时 reducer 把 `batteryState` 置为 `undefined`（`.ref/devices/104/…` 的 `setBatteryState`），也就是**整块不渲染**——本地等价做法是设备没有 `powerStatus` 就返回 `None`，不编造电量。

## 状态机（`qe`／`Sa`，`.ref/devices/104/static/js/main.9b94769e.js`）

| `chargingStatus` | 类名 | 文案 key |
| --- | --- | --- |
| `off` | `batt batt-off` | `BATTERY_OFF` |
| `Charging` 且 `level >= 99` | `batt charging100` | `BATTERY_CHARGED_FULL` |
| `Charging` | `batt charging` | `BATTERY_CHARGING` |
| `NoCharge_BatteryFull` | `bucket(level)` | `BATTERY_PERCENT` |
| `batt-warning` | `batt batt-warning` | `BATTERY_ERROR_TIPS` |
| `ReachChargingLimit`（`pV.PAUSED_CHARGING`） | `bucket(level, true)` | `BATTERY_PERCENT` |
| 其它 / 缺省 | `bucket(100)` | `BATTERY_PERCENT` |

分档公式：`level <= 0` → `batt batt-disconnected`；否则 `10 === level ? 10 : (level > 10 && level < 20) ? 20 : 10 * floor(level / 10)`，暂停充电再补 ` paused`。

## 样式与图标（`.ref/devices/100/static/css/main.dd229426.css`）

- `.right .battery{align-items:center;color:#ccc;display:flex;font-size:14px;height:46px;justify-content:center}`
- `.battery .low-batt{color:#c8323c}`（本地用主题 `danger`，取值同为 `#C8323C`）
- `.nav-tabs .batt{background-position:50%;background-repeat:no-repeat;background-size:20px;height:26px;margin:0 10px;width:26px}` + `.nav-tabs .batt{background-image:url(icon_battery_100…);margin:0 5px}`
- 各状态图标：`batt-0…100` → `icon_battery_N`、`charging`／`charging100`、`batt-disconnected`、`batt-off`（`icon_device_off`）、`batt-warning`（`icon_battery_error`）、`batt-N.paused` → `icon_battery_paused.svg#N`
- 警告工具提示：`.nav-tabs .batt.batt-warning[tooltip]:before{color:#ccc;font:normal normal normal 14px/16px Roboto;…width:352px;…text-align:left}`

16 个电量 SVG 从 182/653/777 三个包里同名同哈希的文件按字节打包（`assets/synapse/battery-*.svg`）。暂停档在原文件里是 20×220 图标条上的 `<view id="N" viewBox="0 N*40 20 20"/>` 片段，gpui 不支持片段选择，因此 `tools/prepare-resources.py` 按原 `viewBox` 把 11 档裁成 `battery-paused-{N}.svg`（记录里带 `view_box` 与来源路径）。

## 文案

`BATTERY_OFF`／`BATTERY_CHARGING`／`BATTERY_CHARGED_FULL`／`BATTERY_PERCENT`（`{{level}}` 占位）／`BATTERY_ERROR_TIPS` 直接取雷云语言包 `locales/*.json`（与源码里的 key 同名）；`BATTERY_PERCENT` 按原版 `ja.JN(a,{level})` 的方式替换 `{{level}}`。

## 本地实现与已知差异

- 实现位置：`src/ui/battery.rs`（状态机 + 元素），接在设备页顶栏帮助图标之前——原版 `.right` 的顺序即「警告 / 过滤条 / 电量 / 扩展项 / 帮助」。
- 逐值对照写在 `src/ui/battery_tests.rs`（只编译、不运行）。
- 已知差异：原版的 `hideBattValue` 开关（只显示图标不显示百分比）在本地数据里没有对应字段，未实现；耳机左右耳电量（`earbudBatteries`）同样没有本地数据来源，未实现。两者都属于「等服务/数据到位再统一接」的范围。
