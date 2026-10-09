# Razer 查询的跨平台 HID 实现

2026-10-09。按 [OpenLogi 审阅](openlogi-device-communication-review.md) 拆分 backend、协议、设备会话和观察状态。参考 commit 为 `1505c6525470bc0a38ae3ba79d70e950347d2532`，不采用 Logitech HID++ 报文。

现有 Windows 鼠标/接收器查询已经走 `razer-device::backend::FeatureTransport`，由 `razer-hid` 实现，不再提取或加载官方 `HID.node`。契约归协议层，依赖方向为 `razer-hid → razer-device`，协议、模型和资源不依赖系统 HID 后端。其他 DLL、COM 音频和宿主服务仍在 `razer-service`；不声称全部 DLL 功能已经替换。直接查询只覆盖原能力资产中核验的命令。已接入 agent/IPC 的直接 DPI/USB 回报率入口及限制见 [写入契约](device-write-current.md)。

[机器证据](cross-platform-hid-current-evidence.json) 复核官方 PE 的 4 个代码范围及 hidapi 的 7 份源码收据。原件为现行 host 的 `node-rz-hid/build/Release/HID.node`，SHA-256 `f611827603911d7807c8499dd231bdf77898fbe2ec3ce40215dccfbb7185cc1f`。

| 原代码行为 | 新实现边界 |
| --- | --- |
| `hid_open_path` / `0x17250`：读写打开失败重试 access=0，共享读写、overlapped | Windows 系统后端相同策略；按唯一 collection 的实际路径打开 |
| `hid_send_feature_report` / `0x17a10`：HidD_SetFeature，成功返回输入长度 | hidapi 检查完整发送，上层使用 `Result<()>`；发送成功不表示设备已确认 |
| `hid_get_feature_report` / `0x17ac0`：DeviceIoControl `0xB0192`，同一入/出缓冲区，等待完成，transferred+1 | hidapi 对 Report ID 0 返回同样的实际长度；当前能力的 report ID 均为 0，不能推广到未知编号协议 |
| `hid_close` / `0x17de0`：取消并释放 handle | 后端 RAII 管理系统 handle，不涉及厂商 opaque handle |
| middleware 的报文、事务、命令、状态、重试 | [共享查询执行器](../../crates/razer-device/src/device_query.rs) 沿用原能力参数和解码器；身份及 relay 校验留在会话入口 |

Razer 报文仍依据 [设备读取证据](mouse-read-capabilities-current-evidence.json)、[接收器发现证据](receiver-discovery-current-evidence.json) 和 [原生 HID 证据](receiver-native-hid-current-evidence.json)。HID descriptor 是传输规格，不是产品功能证据。

后端固定 `hidapi =2.6.7`，crate/checksum `818c0e1d27887aaf76fe737042e27a66b796a7b099e6d2e1a72d106c2dff3fa6` 与 Cargo.lock 相符。选用 Feature 能力后端而非直接复制 OpenLogi 的 output/input channel；保留相同的 backend 分层。

| 平台 | 后端 |
| --- | --- |
| Windows | 静态 hidapi 系统后端；协议和 HID 接口包没有直接 Win32 FFI |
| Linux | `linux-native-basic-udev`、hidraw Feature ioctl，无 libudev 开发库要求 |
| macOS | IOKit Feature，启用 `macos-shared-device`，不独占普通鼠标/键盘输入 |

Windows 元数据、窗口和托盘仍需要各自平台 API，留在对应适配包。根可执行包不直接依赖 windows-sys；跨平台不等于没有操作系统接口。

以下属于应用政策，不能冒充厂商原码：节点保留原始 path 字节、usage pair、接口号和描述符身份，打开前后要求唯一匹配；一个 worker 线程共用一个 HID manager；handle 持有跨进程路径锁。后端会对较短 Windows Feature report padding，因此应用按实际 descriptor 的 Report ID/长度拒绝规格不一致，Windows adapter 另核对 caps，避免改变原精确报文。所有响应长度取实际返回，不补造成功。上层观察期限和父进程超时不被当作固件协议常量。

现有 `DeviceRead`、`ReceiverWirelessStatus` 保留 Windows 真 ContainerId、实例、接口和 relay 前后 peer 校验。新 worker 入口 `HidNodes`、`HidNodeRead`、`HidNodeReceiverStatus` 可三平台分派：直接设备须源能力唯一匹配，接收器返回真实 raw PID/status；接口号未知、节点变化或响应不匹配失败。后端未报告枚举完整性时记录 `not_reported_by_backend`。

portable 节点不生成假的 Windows GUID。Linux/macOS UI 物理分组、热插拔及 receiver relay 尚未完整适配；协议/传输跨平台与整个 UI 三平台验收是不同进度。开发只核对 PE、源码、checksum 和 Windows 静态编译，未运行测试、应用、DLL 或硬件查询。
