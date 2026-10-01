# 校准：182 Smart Tracking

> Rust 已进入重构版本。本文的原版 JS/CONFIG/CSS 证据继续适用；旧 Rust 对照已作为重构前基线保留，当前代码、已完成项和剩余差异见[重构状态](../re/03-implementation-gap.md)。

## 1. 实际入口

`[JS]` [182 main](../../.ref/devices/182/static/js/main.db20a7c4.js)：`GM.navs → mD → PD → pD → LD`。`pD` 读取 smartOnlyCalibrationReducer.smartTracking。

本地实际入口是 Smart Tracking，不是“新增鼠标垫 → 移动扫描 → 完成/失败”的表面校准向导。共享 manualCalibration 数据、CSS 和完成图片不证明此路由使用它们。653、777 均无 Calibration 根页面。

## 2. 结构

```text
LD / body-widgets
├─ 条件首次提示 DD
│  └─ welcome / calibration-welcome / mouse-mat-calibration
└─ ms / widget：SMARTTRACKING
   ├─ SMART_TRACKING_DISC
   ├─ enableAsym checkbox + ENABLEASYMMETRICCUTOFF + tooltip
   ├─ 对称：TRACKINGDISTANCE
   ├─ 非对称：LIFTOFFDISTANCE + LANDINGDISTANCE + 提示
   └─ RESET 标题 + RESET_DESC1 说明
```

最后的 RESET 在此 render 树中是说明区，没有实际 reset 按钮/handler，不能把标题改成可点击重置。

## 3. 数值与联动

| 字段 | 范围 | setter |
|---|---|---|
| isAsymmetric | bool | setIsAsymmetricCutOff |
| trackingDistance | 1–3，step 1 | setTrackingDistance |
| liftOffDistance | 2–26，step 1 | setLiftOffDistance |
| landingDistance | 1–25，step 1 | setLandingDistance |

lift/landing 上限存在 prop 覆盖能力；上述为当前 LD 的默认边界。事件还调用 `setSmartOnlyCalibrationValues` 保存整组 smartTracking。

```text
抬升值 a <= landingDistance：
    landingDistance = max(1, a - 1)
着陆值 a >= liftOffDistance：
    liftOffDistance = min(a + 1, 26)
```

不变量为 `landingDistance < liftOffDistance`。原 UI 没有证明这些数字是毫米，不得自行附加 mm。对称的 1–3 是等级范围。slider 存在 cancelMouseUpOnValueChange 约定，需要避免连续拖动和 mouseup 重复提交。

## 4. 首次提示

LD.useEffect 从 `eE.A` 读取 `Ze.jU0` 持久化标记，已确认后不再显示；关闭/确认走对应回调。它是介绍提示，不是 Running/Completed 校准状态机。

`DD` 只有右上关闭入口、标题和正文，没有确认按钮。`.welcome .title` 为 RazerF5、26px、行高 30、下距 10、绿色居中；正文 14px、行高 16、居中。提示使用 #2d2d2d 底、圆角 5、padding 20px 30px；36×36 指关闭命中区，关闭图标本身为 20px。此前把标题记为 20px、把整个 36px 区域记为图标的描述不准确。

## 5. 资源

| 用途 | 路径 / 锚点 |
|---|---|
| 页面与提示 | main 的 LD / DD |
| 布局 | [main.48c20423.css](../../.ref/devices/182/static/css/main.48c20423.css) 的 calibration-welcome、calibration_tool_tip |
| 提示关闭 | [icon_close_white.8ab462b8.svg](../../.ref/devices/182/static/media/icon_close_white.8ab462b8.svg) |
| hover/active 关闭 | [icon_close_green.45f61360.svg](../../.ref/devices/182/static/media/icon_close_green.45f61360.svg) |
| slider / checkbox | 组件和 CSS，不需要扫描背景图 |

mouse_calibration_complete/error 等虽然在包内，但当前路由未引用为完成流程，见 [资源索引](../re/04-resource-index.md)。

## 6. 重构前基线与验收

`[RUST 基线]` [calibration.rs](../../src/features/calibration.rs) 和 AppShell::start_calibration/add_surface/finish_calibration 围绕表面/校准流程，与本地 LD 路由不一致。

`[建议]` 用独立 smartTracking 领域状态、Checkbox 和 retained SliderState 表达。验收覆盖两种模式、边界自动调整、profile 隔离、首次提示持久化、禁用/加载和后端失败。不要为本页添加无源码依据的成功动画。

## 7. 2026-10-01 样式接入

[当前代码](../../src/features/device_pages.rs) 已恢复 `DD` 的居中介绍区、26px 标题及右上关闭入口，并接入原白色/绿色关闭 SVG；关闭继续触发 `WorkspaceEvent::IntroDismissed`，使用既有独立持久化流程。标题与正文读取原语言键 `MOUSE_MAT_CALIBRATION_HEADER` / `MOUSE_MAT_CALIBRATION`。

Smart Tracking 卡按 `LD` 的间距组织：正文下距 20、非对称 checkbox 与帮助提示同一行、各距离标题上距 20/下距 10。对称追踪采用 36px 无数值气泡滑条，低/中/高标签分布在下方；抬升与着陆采用 64px 滑条区域，在轨道上方显示随当前数值位置移动的气泡，下方保留低/高标签。保留原 SliderState、联动边界和框架拖动行为；`control-Tracking/Lift/Landing` 身份不变。气泡尖角和框架滑块悬停动画尚未与源 CSS 完全一致。

非对称模式的次要说明改用原 `WARNING_SETTING_LANDING_DISTANCE`，重置区使用原 `RESET_DESC1` 正文并保持不可点击。样式结论来自 JS/CSS 和资源文件，不采用旧截图。
