# 校准（`TAB_CALIBRATION`，productId `182`）

> 本文只记录 182 鼠标模块实际拥有的表面校准路径。共享 locale 中的手柄、游戏控制器、耳机或其他校准文案不属于本页面结论。

## 1. 入口与边界

- 模块：`.ref/devices/182`，产品：Razer DeathAdder V3 Pro，`productId=182`。
- 标签：`TAB_CALIBRATION`。
- 182 的校准语义是鼠标传感器/鼠标垫表面校准，不是拇指控制杆校准，也不是耳机校准。
- 页面会根据当前表面、传感器数据、校准状态和设备服务回传决定按钮与提示；不能由 `CALIBRATION_*` key 的数量推导完整页面。

## 2. 实际页面内容

182 主 bundle 的实际文案集合明确包含以下鼠标表面校准流程：

```text
Calibration
├─ surface/profile 选择
│  ├─ 预校准 Razer 表面数据说明
│  ├─ 自定义表面入口
│  └─ selected / calibrated 状态
├─ manual calibration action
│  ├─ CALIBRATE / START
│  ├─ 单击鼠标左键并移动鼠标
│  ├─ 以 Z 字形覆盖整个鼠标垫
│  └─ 不抬起鼠标持续移动至少 3 秒（对应特定传感器流程）
└─ 结果状态
   ├─ calibrating / calculating tolerance
   ├─ moving too fast
   ├─ completed
   ├─ error / retry
   └─ Esc 取消或再次单击左键结束（按当前流程分支）
```

注意：`main.db20a7c4.js` 中这些文案是同一个校准功能族的不同状态/版本；不能把每条文案同时显示成独立卡片。

## 3. 表面卡片布局

主 CSS 对 182 的 `.surface` 给出明确几何：

| 选择器 | 已确认值 |
|---|---|
| `.mats.flex` | `align-self:flex-start; flex-wrap:wrap` |
| `.mats.flex > div` | `flex:0 0 auto` |
| `.surface` | `width:290px; height:200px; margin:0 20px 20px 0; padding:8px 20px; background:#111; border-radius:5px; position:relative` |
| `.surface:hover` | `top:-4px`，过渡 `.2s` |
| `.surface.add` | `background:#222; border:2px dashed #5d5d5d` |
| `.surface.add:hover` | 边框变为 `#44d62c`，不抬升 |
| `.surface .img` | `width:250px; height:140px` |
| `.surface.selected .check-circle` | 背景 `#44d62c`，伪元素绘制选中勾 |

因此表面列表是横向 wrap 的 `290px` 卡片，不是单列 `SettingRow`；添加表面是虚线卡片，不是普通绿色主按钮。

## 4. 说明区与弹层

- `.calibration-body` 的 CSS 为 `margin:auto; max-width:600px`，它是校准说明/内容区域的宽度约束。
- 预校准表面信息包含传感器微调说明，以及 `pre-calibrated Razer surface profile` 语义；只有当当前选择的是预校准表面时才应显示对应说明。
- `.choose-a-mat` 是选择表面相关弹层/区域，默认样式包含 `background:#000; border:1px solid #5d5d5d; font-size:14px; padding:8px 10px`；它不是普通 widget 卡片。
- `exclamation` 为 `14px` 圆形提示图标，底色 `#5d5d5d`，使用 warning SVG；提示内容由当前校准状态决定。

## 5. 操作与状态

### 5.1 空闲

- 显示当前表面及其是否已校准。
- 可选择已存在的表面。
- 可进入 `ADD_MAT` / `CREATE_OWN_SURFACE_PROFILE` 流程；自定义表面与预置表面需要区别对待。

### 5.2 校准中

- 显示 `CALIBRATING`、`CALIBRATE_MAX_HEIGHT`、`CALIBRATING_MAX_DEPTH` 或 `CALIBRATE_CALCULATING_TOLERANCE` 等当前阶段文案之一。
- 显示鼠标操作指导；移动过快时显示 `CALIBRATE_WARNING_TOO_FAST`。
- 校准中不能把表面状态直接标为成功，也不能允许删除/切换导致流程失去目标。

### 5.3 完成、错误、重试

- 成功使用 `CALIBRATION_COMPLETED` / `CALIBRATION_SUCCESSFUL` 语义；成功后才将当前表面标为已校准。
- 失败使用 `CALIBRATION_ERROR` / `CALIBRATION_ERROR_MSG`，并提供 `CALIBRATION_RETRY`/`RETRY` 语义。
- `CALIBRATE_END`、`ESC` 取消是流程控制，不等同于失败。
- 当前实现必须等待设备服务确认结果；不能点击开始后立即显示“校准成功”。

## 6. 颜色和通用密度

仅记录实际命中选择器的值：

| 角色 | 值 |
|---|---|
| 页面底色 | `#222` |
| 主文字 | `#ccc` |
| 表面卡片底色 | `#111` |
| 添加卡片底色 | `#222` |
| 通用边框/虚线 | `#5d5d5d` |
| selected / hover accent | `#44d62c` |
| 警示提示 | 使用当前 warning/exclamation 资源和对应状态，不将所有警示都硬编码为绿色 |

## 7. 明确排除

- 不把 `CALIBRATION_STEP0..5` 的拇指控制杆文案写进 182 鼠标表面校准页面。
- 不把 777 的校准分支、PlayStation/手柄分支和通用校准 key 当成 182 的 DOM。
- 不因 CSS 中存在 `.calibration-body`、`.surface` 就声称所有设备都渲染这些节点；本文结论限定在 182 的页面路由和实际鼠标文案。

## 8. 证据文件

- `.ref/devices/182/static/js/main.db20a7c4.js`
- `.ref/devices/182/static/css/main.48c20423.css`
- `.ref/devices/182/manifest.json`
