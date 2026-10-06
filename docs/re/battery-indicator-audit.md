# 电量控件当前复核

2026-10-04。实际挂载与CSS级联见[独立复核](source-ui-review-2026-10-04.md)和[导航栏机器收据](device-tabs-audit.json)。[电量表审计](battery-indicator-audit.json)只验证图标/文案表，不再充当位置正确的证明。

已修正：两种产品工作区将电量和帮助放入同一右侧容器；仅has_battery且有power_status时显示；电量用有符号值表达原来的负值显示“-”分支；未知充电状态的默认100图标仍用实际level作提示文案。

内部几何仍按实际级联：46px行、14px文字、26px图标盒、20px图像、左右各5px。百分比在图标之前，0到10为低电红字。

原节点不含tooltip属性，因此 `.batt.batt-warning[tooltip]:before` 的352px不适用。已改用真实tooltip-razer语义：300px对齐容器、按内容收缩的wrapper、Roboto14/16、padding8/10、黑底灰边；右缘对齐、电池底部+5px、水平8px边缘调整、100ms opacity。

仍未完成hideBattValue、externalPowerConnected、左右耳塞电量及对应root状态输入。这些须按产品继续核对，后端/DLL最后统一接入。静态验证不能替代运行时像素验收。

## 2026-10-06 `hideBattValue` 接入（原记录里「无数据来源」的说法已纠正）

原版把 `hideBattValue` 当作**产品工作区里写死的 prop**，不是设备数据。以 182 的
`main.db20a7c4.js` 为准，`main.03a1d509.css` 里只有 `.hideBattValue{margin-right:17px}`：

- `className={"battery " + (this.props.hideBattValue ? "hideBattValue" : "")}`；
- `{!hideBattValue && !o && <span aria-hidden="true" className={level 0..10 ? "low-batt" : ""}>{level >= 0 ? `${level} %` : "-"}</span>}`；
- `{!hideBattValue && <Tooltip position="bottom-left" isMounted={showBatteryTooltip && batteryTips !== undefined} target="battery-level-tips">{batteryTips}</Tooltip>}`。

也就是说：传 `!0` 的产品只显示电量图标——不渲染百分比、也不挂载悬停提示，并加 17px 右边距。

按当前包重算（`tools/audit-battery-indicator.cjs`，扫描 `.ref/devices/*/static/js/*.js` 里的
`hideBattValue:!0`），共 15 个产品传 `!0`：115、131、1330、1342、1370、1372、1374、1443、1453、
2636、2647、2676、4115、4133、4144（另有 27 个产品显式传 `!1`，即显示数值，本地默认行为一致）。
其中 1330（RAZER LEVIATHAN V2）等已是本地注册产品，所以这条不是「无数据来源」而是可以按产品表实现。

本地改动（`src/ui/battery.rs`）：新增 `HIDE_BATTERY_VALUE: &[u32]` 表与隐藏分支——隐藏时只渲染
图标容器（`h(46)`、居中、`mr(17px)`，元素 id 带 `hideBattValue` 后缀便于对照），不渲染百分比、
不包 `SourceTooltip`；非隐藏分支不变（数值 + 图标 + `Battery` 提示框）。审计脚本新增
「当前源产品表 vs 本地表逐项一致」「`.hideBattValue{margin-right:17px}` 在 CSS 里」「本地隐藏分支
标记齐备」三条断言，结果写入 `docs/re/battery-indicator-audit.json` 的 `hide_battery_value`。

仍然缺的只有耳机左右耳电量（`earbudBatteries.left/right` 与 `L`/`R` 文案），那确实需要设备数据。
