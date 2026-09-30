# 性能（`TAB_PERFORMANCE`，productId `182`）

> 本文只记录 `182` 模块实际进入性能页面的内容。共享 CSS、共享 locale key、其他产品的性能组件和未触发的 lazy chunk 均不视为页面证据。

## 1. 入口与显示条件

- 模块：`.ref/devices/182`，产品：Razer DeathAdder V3 Pro，`productId=182`。
- 标签：`TAB_PERFORMANCE`。
- 性能页面使用该设备的 `DeviceInfo`、当前 profile、DPI stages、连接方式和设备能力决定可见控件。
- `HyperPolling` 相关轮询档位受接收器/连接状态约束；不能因为语言包存在 `HYPERPOLLING` 就默认显示所有高频档位。
- 键盘的 Actuation、Snap Tap、Dynamic Keystroke 等能力不属于本文，不能从共享组件名推导到 182 鼠标。

## 2. 鼠标性能页面的实际分区

`main.db20a7c4.js` 和 `MapSensitivity.ed0c234c.chunk.js` 确认的页面语义为：

```text
Performance
├─ DPI / sensitivity 区
│  ├─ DPI 当前值（X；支持 XY 时才有 Y）
│  ├─ XY 开关（由 DeviceInfo.supportXYDPI 决定）
│  ├─ X/Y slider
│  └─ DPI stages / 由阶段动作打开的阶段配置入口
├─ Polling rate 区
│  ├─ 可用轮询率按钮组
│  ├─ 当前选中状态
│  └─ 连接限制/电量/CPU 提示（仅满足条件时显示）
├─ Lift-off / Smart Tracking 区
│  ├─ tracking distance 说明
│  ├─ asymmetric cut-off 开关或相关配置
│  └─ surface distance / lift-off distance 控件（取决于设备状态）
└─ 其他鼠标能力
   └─ 只有主页面条件和对应组件实际进入时才出现
```

不要把该页实现成“所有能力均存在的设置表”。原页面由设备能力和当前映射状态决定分支。

## 3. DPI 与阶段

### 3.1 `MapSensitivity` 的已确认行为

`MapSensitivity.ed0c234c.chunk.js` 实际 render：

- 使用 `DeviceInfo.minDPI`、`maxDPI`、`dpiStep` 作为 slider 边界和步进；缺失时才使用组件默认回退。
- `supportXYDPI` 为真时渲染 X、Y 两个 slider；否则只渲染单轴。
- XY 开关切换时，关闭 XY 会令 Y 跟随 X；不是两个永远独立的输入框。
- DPI 输入框类为 `.stage-input`，源码限制 `maxLength=5`，并在 blur/key handling 时校正数值。
- 阶段配置的数据集会排除 `DPI_Clutch` / `DPI_OnTheFly` 等不适用于 `CycleUpSensitivityStages` 的动作；Hypershift 也会改变可用动作集合。
- `configure-sensitivity` / `config-sensitivity` 点击会跳转到性能设置，而不是伪造一个本地弹窗。

### 3.2 阶段视觉

主 CSS 的阶段列表确认：

| 选择器 | 值 |
|---|---|
| `.stages` | `display:flex; flex-direction:column` |
| `.stages .stage` | `height:68px; display:flex; align-items:center; border-radius:3px; margin-bottom:4px` |
| `.stage-ordinal` | `30px × 30px` |
| `.stage-input` | `60px × 26px; background:#111; border:1px solid #5d5d5d; color:#ccc; font-size:14px` |
| `.stage.drag-over` | 绿色底边 `#44d62c` |
| `.description-stages` | `color:#999; line-height:17px` |

## 4. 轮询率

`main.48c20423.css` 的实际选择器：

```css
.polling-rate { padding-top:10px; position:relative; z-index:1 }
.polling-rate .dropdown-area { margin-left:0; width:100px }
.customize-polling-rate-button {
  align-items:center; background-color:#222; border:1px solid #5d5d5d;
  border-radius:3px; color:#ccc; display:flex; font-size:14px;
  height:27px; justify-content:center; text-transform:uppercase; width:72px
}
.customize-polling-rate-button.configWidth { min-width:90px }
.customize-polling-rate-button.active,
.customize-polling-rate-button:hover { border-color:#44d62c }
```

因此轮询率是按钮组，不应改成普通下拉框；每个按钮可能有 6px 圆形速率指示点（`.customize-polling-rate-button-color`）。高轮询率的可用性由当前连接/接收器状态决定。

## 5. Lift-off / Smart Tracking

已确认的 CSS 与文案语义：

- `.lift-off-wrapper` 最大宽度 `290px`。
- `.lift-off`：`width:290px; padding:20px; background:#111; border-radius:5px; font-size:14px; line-height:17px`。
- `.lift-off .title`：`#44d62c`、RazerF5、`16px`、大写、底部间距 `20px`。
- `Smart Tracking`、`Asymmetric cut-off`、`surface distance`、`lift-off distance` 是不同语义，不能合并为一个“抬升距离”字段。
- `CALIBRATION` 相关文字在 locale 中存在，但性能页面是否打开校准流程必须由实际性能组件条件触发；不能仅凭 key 认定有按钮。

## 6. 颜色、密度与操作状态

- 页面底色 `#222`，主文字 `#ccc`，卡片/输入底色 `#111`，主色 `#44d62c`，通用边框 `#5d5d5d`。
- 主卡片来自 `.widget`：`600px`、`padding:30px 40px`、`border-radius:5px`；产品模块页面外壳的 `.body-widgets` 最大宽 `1240px`。
- slider 的 disabled 状态必须降低透明度并阻止交互；不能显示为可编辑但不写回设备状态。
- 轮询率 active 是绿色边框，不是绿色实心按钮。
- 修改 DPI/轮询率/追踪距离后，保存状态由原页面的设备服务/保存机制确认；不能直接显示“成功”而没有服务回传。

## 7. 明确排除

- 不把 `TAB_PERFORMANCE` 共享 key 当作 653/777 的实际页面结构。
- 不把键盘专属 Actuation/Snap Tap/Dynamic Keystroke 填进 182 鼠标页面。
- 不把独立 `Map*.chunk.js` 文件存在等同于默认 render；它们由当前 mapping type 或设备条件动态加载。

## 8. 证据文件

- `.ref/devices/182/static/js/main.db20a7c4.js`
- `.ref/devices/182/static/js/MapSensitivity.ed0c234c.chunk.js`
- `.ref/devices/182/static/js/MapMouse.d4aa1eba.chunk.js`
- `.ref/devices/182/static/css/main.48c20423.css`
- `.ref/devices/182/manifest.json`
