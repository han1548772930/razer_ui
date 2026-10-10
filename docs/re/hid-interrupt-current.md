# 当前 HID 硬件事件的中断读取

当前宿主的 `node-rz-hid/build/Release/HID.node` SHA-256 为 `f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f`。IDA/Hex-Rays 的 `hid_read_timeout` 位于 RVA `0x17800..0x179ea`，`hid_read` 位于 `0x179f0..0x179fd`。完整正文、原字节校验、当前宿主消费者和 Rust 文件收据见 [证据](hid-interrupt-current-evidence.json)。

原读取走 overlapped `ReadFile`，等待超时返回 0；成功保留实际字节数。Windows API 缓冲区首字节为零时去掉这个无编号 Report ID 占位，非零时保留真实 Report ID。它与 Feature 查询、控制通道 `GET_REPORT` 是不同通道，不能用 Feature ACK 或控制 Input Report 模拟异步硬件事件。

当前 `electron/modules/hidHardwareEvents/index.js` 按 vendor、product、ContainerId、usagePage、usage 唯一选择 collection，注册 `data/error` 监听。仅实际 producer 配置 `hidConfig.appendReportId` 时才在数据前补 `hidConfig.reportId ?? 5`；其他情况下原样转发。取消注册移除监听并关闭保留的句柄。HID Report ID 与产品 parser 的 recordId 必须分别取证，不能对所有事件统一补 5 或删零。

共享 `FeatureTransport::read_interrupt` 和 hidapi 适配已实现独立中断读取，保留上述原始字节语义；返回 0 表示超时。报告编号和长度由实际 descriptor 核对，不猜长度，不截断。有限超时、要求缓冲区容纳最大报告及拒绝混合编号/无编号输入属于应用校验策略，不冒充原产品协议。

164/241 配对 `Session` 要求命令前建立 `HardwareEvents` 订阅，事件 54/status2 才确认配对，事件 55/status2 才取得扫描结果。其实际 mapping_engine producer 已由独立 IDA 正文恢复：按同一 ContainerId/PID 保留全部 collection，读取实际完成字节，不补零或 recordId。直接事件适配、异步服务 controller、Dock 页面及成功后的本地缓存链已接入，见 [接收器契约](receiver-ui-current.md)；配对后的元数据读取、runtime/serial 发布、profile 合并和其他平台物理分组仍逐项核对，不能由 reader 推定完成。没有运行应用、DLL 或硬件操作；`cargo check --locked -p razer-hid -p razer-device --all-targets` 已通过。
