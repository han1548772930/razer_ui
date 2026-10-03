# 键盘触发行程页静态接入

2026-10-03，当前 `.ref/devices/` 中的产品入口和实际挂载页。证据由 `tools/extract-keyboard-actuation.cjs` 静态 Acorn 解析产生，记录于 `src/features/keyboard_actuation_data.json`；未执行下载的 JavaScript。

接入 580、614、642、678、679、688、719、720、721、728、740、741、742、746、747 的首批触发点编辑。各产品分别使用其 `DeviceInfo.analogSpecs.actuationInfo`，不把 12 个单位与 1638 个单位的模拟轴换算混用。第一代的显示偏移单独保留；742 的最大行程和 747 的默认值也保留自身定义。

使用原有按键几何，按源代码常量及模拟输入类型过滤不可选择按键；禁用映射、动态键程、模拟控制器、摇杆移动及已有第二段动作限制也参与判断。支持多选、全选、取消选择、改触发点和同步至全部可用按键。数值持久化到当前配置／键映射的 `mappings[].mapping[].actuationPoint`，使用原始整数单位。创建新触发点时建立标准及 Hypershift 两份映射；改映射和重置按键功能保留触发点与已有快速触发数据。

范围检查通过 `tools/validate-keyboard-actuation.py`。`cargo check --locked --all-targets` 通过；没有运行应用或测试。

这些仍是部分实现：尚缺快速触发编辑、Snap Tap、实时传感器／硬件调整、原版动画、触发点整体重置确认和混合选择值的完整优先级规则。当前触发点控件为普通滑块，还未复刻原版垂直动画布局。全产品覆盖报告只将这些页记为 `partial_native`，不表示完成还原。
