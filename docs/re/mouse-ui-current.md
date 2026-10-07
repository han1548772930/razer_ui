# 鼠标当前实现与能力边界

共享 UI 按经过各自当前源码核实的能力数据创建，不以产品编号命名 Rust 实现或在共享逻辑中分派。编号只作为产品登记、源证据、资源与能力目录的身份键。

## 滚轮

`mouse_scroll_wheel.rs`读取 `mouse_scroll_wheel_data.json`：226 为三模式、有禁用上限及 FreeSpin 派生锁定；153/170 为两模式且无等级滑条，FreeSpin 只锁 SmartReel；203/204/214/45066 为两模式加等级且使用各自独立条件。211/235 当前挂载树没有滚轮编辑面板，profile 存在 scrollWheel 字段不构成渲染依据。

两模式切换按照源整组点击翻转，点击选中半边也改变模式。无等级和有等级的布局、禁用透明度分别取源 CSS。预览、释放提交、本地字段归属和真实观察相互独立，派生锁定不改写保存的 enabled 值。

证据：[滚轮当前收据](mouse-scroll-wheel-current-evidence.json)、[三模式收据](mouse-226-scroll-current-evidence.json)。维护工具：`prepare-mouse-scroll-wheel.cjs --check`、`prepare-mouse-226-scroll.cjs --check`。

## DPI 与系统属性

`mouse_dpi_rows.rs`按能力定义 profile 字段、最少可见行、阶段关闭行为、可见性/拖排回退、说明和负边距。70 的存储 Active 对应运行 visible，不可用它冒充运行拖动标题 Active；源字段缺失时不捏造编号。

`mouse_dpi_grid.rs`接收 GridSpec：分段范围、步长和初始编辑锁来自目录。百分比位置与实际 DPI 模型分别保存；拖动预览、释放提交，外部同步不写草稿。阶段编辑保留 XY、隐藏行及源特有回退；过期拖排检查 owner、原数组和恢复代际。

数字编辑器处理源整数文法、空值/越界归一化、Enter/Escape/失焦、临时步进和重复计时。窗口级数字框外滚轮、完整焦点/量化与像素仍待核对。

`mouse_properties.rs`按能力选择标签、图标、布局和系统属性命令，不按单一型号创建。证据为 [DPI 行能力](mouse-dpi-rows-capability-evidence.json)、[分段 DPI](mouse-226-dpi-current-evidence.json)、[数字框](mouse-226-dpi-number-current-evidence.json)、[Properties](mouse-properties-capabilities-current-evidence.json)。

## 轮询与真实读值

`mouse_polling.rs` / `mouse_polling_view.rs`区分未知、有线、dongle 和 BLE；BLE 不挂载回报率面板。未知保留源默认分支可编辑的本地草稿，并明确未读取。字段按连接选择，限制和提示由真实固件/DualLink/dock 观察决定，不能由产品目录推算 8K 就绪。

`_pollingLocalFieldsV1`逐字段保存显式本地覆盖。观察优先于未编辑默认值，但不覆盖用户显式草稿；纯观察不保存、不产生 dirty。源自动降档属于设备写入，当前不执行。

连接观察已通过发现结果接入；已核实的 Rate 读取生产链仅覆盖能力目录登记范围，不能推广至其他型号。完整 DualLink/dock 拓扑、226 Rate 与 OTFS 运行编辑锁生产者仍有缺口；`isOtfsActive` 导出声明不证明它和该 UI 的 runtimeData 属于同一 owner。Sensitivity Matcher 主体、独立向导和完整状态仍未接入。

具体来源见 [轮询模型](mouse-polling-model-current-evidence.json)和 [读取能力](mouse-read-capabilities-current-evidence.json)；对应工具 `prepare-mouse-polling-model.cjs --check`、`audit-mouse-read-capabilities.cjs --check`。设备发现与接收器的边界见 [Receiver](receiver-ui-current.md)。

## 验证与继续工作

表面校准尚未完整复刻。70 的当前 `TAB_CALIBRATION` 挂载 `rR -> sR/nR`：卡片来自 `calibration.profiles`，选择写 `selectedProfile.guid`，产品 `DEVICE_SUPPORTED_MATS` 只用于过滤新增目录。现共享页面已移除把目录当设备配置的伪造选中动作及源中不存在的 `calibration.selectedSurface` 写入；静态目录保留为信息，明确设备校准配置尚未读取。欢迎层、真实配置卡/菜单、添加抽屉/校准流程、多配置时的 1..10 抬升范围与 `featureAccess` 条件仍待接入，不能由目录或本地草稿推定运行配置。证据见 [表面校准边界](mouse-surface-calibration-current-evidence.json)，静态检查为 `audit-mouse-surface-calibration.cjs --check`。

`audit-feature-capabilities.py --check`检查编号 Rust 模块及已重构共享组件的 PID 字面量分支。能力只扩到独立核实的产品；当前目录不足以表示所有鼠标 UI 已完成。源码/资源检查和编译通过不代表实际连接、设备读写或窗口验收。已修项以 [修复登记](ui-fix-registry.json)为准。
