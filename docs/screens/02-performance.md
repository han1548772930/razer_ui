# 性能：182 独立页面与 653 回报率入口

> Rust 已进入重构版本。本文的原版 JS/CONFIG/CSS 证据继续适用；旧 Rust 对照已作为重构前基线保留，当前代码、已完成项和剩余差异见[重构状态](../re/03-implementation-gap.md)。

## 1. 实际页面树

`[JS]` [182 main](../../.ref/devices/182/static/js/main.db20a7c4.js)：`GM.navs → lM → OM`。

```text
Ls / body-widgets
├─ hs(direction=left) → AM → IM：DPI / 灵敏度阶段
└─ hs(direction=right)
   ├─ !isBle → dm → Rm：回报率
   └─ Dm → Cm：Windows 鼠标属性
```

653 的 `km → Um → Pm` 是 Customize 内回报率卡，不是独立 Performance。777 无此页。Lift-off 属于 182 Calibration。

## 2. DPI、阶段与 profile

`[CONFIG]` 模块 1057：minDPI=100、maxDPI=30000、dpiStep=50、supportXYDPI=true。默认 profile 五阶段为 **400、800、1600、3200、6400**，active=3、visible=true、independent=false。它们是初始值，不是运行时唯一配置。

数字输入由模块 4230 的 stepper 处理，`parseInput` 按步长向上取整后限制范围，例如 `101 → 150`；当前本地 DPI 归一化与此一致。滑条本身以 50 为步长。

`AM` 读取 DPI、dpiStages、profileReducer、customizeReducer。`IM` 使用 `stages[activeStage - 1]`，不能固定读第一个。

| 操作 | 源码调用 | 行为 |
|---|---|---|
| 选择阶段 | setPerformanceSelectedSensitivityStage、setActiveDPI | 同步当前 X/Y 与 independent |
| 修改 X | changeDpiValueX → setDotsPerInchStageValueX | XY 联动时同时同步 Y |
| 修改 Y | changeDpiValueY → setDotsPerInchStageValueY | 只在独立分支有效 |
| 切换 XY | toggleY → setPerformanceIsSensitivityXYEnabled | 切换独立状态，联动分支将 Y 对齐 X |
| 显示阶段 | toggleStages → setPerformanceIsSensitivityStagesEnabled | 影响阶段选择区域 |
| 阶段启用/更新 | setEnableStage / updateStages → updateDPIStage | 更新当前 profile 阶段集合 |

缺 dpiStages 时 IM.render 返回 null；selectedProfile 变化需要重新同步。共有 props 如 useTwoWayTab、noHeader 不应自动推广为 182 当前页面配置。阶段数量/有效项须按原数组和禁用逻辑处理，不能删到没有有效阶段。

## 3. 回报率与运行条件

182 基础 POLLING_RATE / POLLING_RATE_WIRELESS 为 **125、500、1000 Hz**，不含 250。HyperPolling 候选还有 2000、4000，8K 条件满足才有 8000。

`Rm.checkAndUpdateHyperPollingRateList` 核查：

1. duallink-devices 中与 DeviceInfo.dongleId 对应的接收器。
2. 产品是否属于 HyperPolling 支持集合。
3. isDongle / isDongleHyperpollingDevice 路径。
4. POLLING_RATE_8K_FW_VERSION 与 DEVICE_RUNTIME_DATA 的固件门槛。

不能仅写 `productId == 179` 决定显示。checkActivePollingRateValue 根据 dongle/连接状态选择 pollingRateWireless 或 pollingRate，并调用相应 setter。BLE 下当前 182 页不渲染此卡。

1000 Hz 以上存在性能/功耗提示；是否有 in-game polling 由 supportInGamePollingRate 等配置决定，不能因为共享组件有分支就固定显示。

653 Customize 的候选为 **125、250、500、1000、2000、4000、8000 Hz**，与 182 分开建模。

## 4. 系统属性、视觉与资源

`Cm.openMouseProperties()` 调用 `pt.A.OpenMouseProperties()`，是宿主系统操作。

[182 CSS](../../.ref/devices/182/static/css/main.48c20423.css) 中，左右 widget-col 常用 600 宽，DPI 卡在左，回报率和系统属性纵向堆在右。polling-btn-set 是选项组，polling-warn 是提示；本页不应额外插入设备大图。

| 用途 | 来源 |
|---|---|
| DPI 阶段、轴标签、状态 | IM / Zm + CSS |
| Windows 鼠标属性图 | `Cm → um → Nm` 根据系统版本输出 `windows/windows-11` 类；CSS 引用 `windows_logo.8fb1e7e2.svg` / `common-windows-11.d477cadb.svg`，不是 JS 内嵌 SVG |
| 外部链接图标 | external-link-icon 样式/资源 |
| 字体 | 原包 Roboto，见 [资源索引](../re/04-resource-index.md) |

## 5. 重构前基线与验收

`[RUST 基线]` [performance.rs](../../src/features/performance.rs) 读取 stages.first()，缺活动阶段选择与完整 X/Y 编辑；polling_panel 以 productId 179 判断 HyperPolling，漏掉接收器/固件规则；Lift-off 卡归属错误；setter 只更新 AppShell 内存。

`[建议]` 每个 profile 保留阶段和 active ID；GPUI Kit Slider/数值输入共享领域值，避免在 render 重建 Entity。验收覆盖阶段选择、XY 联动、范围/步长、切 profile、BLE 隐藏、普通/HyperPolling/8K 条件、系统属性打开、失败回读。

## 6. 2026-10-01 样式接入

本轮直接复核 `IM → Zm/jm → Wm/Vm` 的 render 与 CSS，未使用旧截图。阶段不是横向标签：`.stages` 是纵向列表，普通行高 68、间隔 4，X-Y 行高 114；左侧序号为 30px 圆形，包含原版 8×6 阶段三角。`stage-header` 高 20，分别显示 DPI、100 和 30000。`stage-control` 高 27、上距 25，阶段显隐使用 Switch。

[当前代码](../../src/features/sensitivity.rs) 按固定五槽实现纵排阶段：每槽数字输入与 250px 滑条在同一行，该槽独立 X-Y 时增加第二行，并使用原版 sensitivity-xy 默认/active/disabled 三态 SVG。所有可编辑槽位均保留各自 InputState/SliderState；修改非当前槽位时将该槽设为当前阶段，无需先点序号才可编辑。五种阶段三角、XY 图标与 Windows 图标均从 `.ref/devices/182/static/media` 打包。

回报率改为原版 72×27、间隔 10 的数字按钮，选择和悬停只强调绿色边框，不再以绿色实心底替代原样式。鼠标属性恢复 44px Windows 图标、20px 间隔和下划线入口，保持真实系统属性调用；当前图形使用 Windows 11 资源，未接入原宿主版本查询。

每个槽位保留稳定 ID、enabled、independent 和 X/Y 数值；关闭槽位只禁用并保留数值，至少保留两个启用槽位。关闭总阶段显示时只显示当前槽，其他槽值和启用状态保留。停用当前槽自动选择下一个启用槽。原拖动图标支持拖放排序，另提供 Alt+上下方向键排序；槽位值、XY 状态、控件实体和当前槽身份随排序保留。

旧本地配置缺少 slots 时按原阶段值、阶段数量与既有 independent 字段迁移，补齐未使用槽位；旧设备 DPI 配置按各阶段自己的 independent 迁移。被禁用或隐藏槽位的延迟控件事件不能改写值。

数字框已改用 Base NumberInput 承担输入、焦点和步进动作，应用提供原 `.stepper` 的 62×26 外观、上下两个 14×12 箭头和原 8×4 图标。箭头在 hover/聚焦时显示，边界降为 0.3 透明度；保留滚轮和键盘步进、Enter/失焦提交、向上取整和范围归一化。鼠标按下立即修改一次，此后每 300ms 连续步进；松开、移出、输入失焦或窗口停用时停止，重复任务也检查设备/Profile、当前页、可编辑槽位及数值边界。鼠标点击路径抑制对应语义 click 的重复修改，键盘动作仍由框架处理。

[sensitivity_tests.rs](../../src/features/sensitivity_tests.rs) 和领域测试保留数值草稿、步进、状态迁移、稳定身份和延迟事件路径的用例。本轮仅做 `cargo check`，不执行测试、应用或 DLL，不能把编译检查视为长按事件顺序或视觉验收。HyperPolling/8K 条件及硬件回读仍未接入。
