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

后端固定 `hidapi =2.6.7`，crate/checksum `818c0e1d27887aaf76fe737042e27a66b796a7b099e6d2e1a72d106c2dff3fa6` 与 Cargo.lock 相符。Feature、Output 和控制 Input 各有独立接口；Output 使用 hidapi.write，控制 Input 使用 get_input_report，不混用 Feature 或中断 read。后端不能实现的通道明确返回错误。设备报文依据 Razer 原件，OpenLogi 只提供 backend 分层参考。

Audio Mixer 当前两层 DLL 的 [证据](audio-mixer-dll-protocol-current.md) 已驱动 47 个展开硬件控制项的 Rust 读写及 IPC。descriptor 静态解析 Input/Output/Feature，包括 padding 与 global push/pop；Mixer 按原 HIDP_CAPS 的 collection 最大报告长度构造缓冲，验证实际 ID/报告长度，超出原 66 字节缓冲边界拒绝。接收实际返回长度，不补造响应。另已在共享协议实现 22 条驱动矩阵路由和 0..8 重置序列，Windows 句柄/IOCTL 隔离在 service 适配；其他平台显式报能力缺失。原 100ms WriteFile 等待、关闭 Sleep500ms 与 hidapi 生命周期没有等价实现声明；COM、产品页面消费及实际驱动身份关联验收仍缺。

| 平台 | 后端 |
| --- | --- |
| Windows | 静态 hidapi 系统后端；协议和 HID 接口包没有直接 Win32 FFI |
| Linux | `linux-native-basic-udev`、hidraw Feature/Output/control-Input，无 libudev 开发库要求；具体 OS 调用支持错误向上传递 |
| macOS | IOKit HID 通道，启用 `macos-shared-device`，不独占普通鼠标/键盘输入；具体 OS 调用支持错误向上传递 |

Windows 元数据、窗口和托盘仍需要各自平台 API，留在对应适配包。根可执行包不直接依赖 windows-sys；跨平台不等于没有操作系统接口。

平台边界按实际实现分目录，参考 OpenLogi 的 backend 契约、共享 native transport 与 Windows 专用 adapter 分层：

- `razer-device/src/` 保留 Razer 协议、能力和参数编码，不含系统后端。
- `razer-hid/src/transport/hidapi.rs` 是 Windows/Linux/macOS 共用的 hidapi 实现，descriptor 解析仍共享。
- `razer-discovery/src/direct/platform/windows.rs` 只解析 Windows ContainerId；直接路由和 portable collection 观察在共享层。
- `razer-service/src/runtime/portable.rs` 接共享 HID 请求，`runtime/windows/` 放 Windows USB/HID 容器、relay、DLL worker 和驱动适配；`runtime/mod.rs` 统一分派。
- `razer-platform/src/platform/windows/` 放 SCM、系统滚轮设置、系统属性及设备通知适配，原公共入口仅委托和声明不支持。

Linux/macOS 共用真实 transport；没有创建仅占目录的空实现。尚无等价实现的 OS 专属能力保持明确错误。

原 DeviceInfo 的 `claimInterface` 既可能为数字，也可能为 PID 映射。原 factory 数字分支使用数字值，映射分支使用实际连接对象 `productId` 查表，然后将接口号传给 constructor/connectHidDevice。运行资产将主 PID 对应项保存为 `claim_interface`，原完整映射保存为 `claim_interfaces_by_pid`；共享 `claim_interface_for(actual_pid)` 精确选取实际物理 PID。未包含该 PID 时拒绝是本应用的保守路由政策；原映射缺项得到 undefined，基类 constructor 则默认 0，不声称原程序也抛同样错误。Blade 的 `rzDeviceType` 接口 2 override 依赖原类型观察，该特殊分支尚未完整接入，不能由默认有线路由推定完成。原始证据保留全部选择分支。鼠标/键盘能力资产均须通过嵌入 JSON 的字段类型与范围检查，避免某一产品的映射使整张读取目录解析失败。

以下属于应用政策，不能冒充厂商原码：节点保留原始 path 字节、usage pair、接口号和描述符身份，打开前后要求唯一匹配；一个 worker 线程共用一个 HID manager；handle 持有跨进程路径锁。后端会对较短 Windows Feature report padding，因此应用按实际 descriptor 的 Report ID/长度拒绝规格不一致，Windows adapter 另核对 caps，避免改变原精确报文。所有响应长度取实际返回，不补造成功。上层观察期限和父进程超时不被当作固件协议常量。

现有 `DeviceRead`、`ReceiverWirelessStatus` 保留 Windows 真 ContainerId、实例、接口和 relay 前后 peer 校验。新 worker 入口 `HidNodes`、`HidNodeRead`、`HidNodeReceiverStatus` 可三平台分派：直接设备须源能力唯一匹配，接收器返回真实 raw PID/status；接口号未知、节点变化或响应不匹配失败。后端未报告枚举完整性时记录 `not_reported_by_backend`。

服务连接页面在 Windows 保留实际 USB/HID 容器观察；Linux/macOS 改为真实 `HidNodes` 枚举和 collection 观察，不再请求 Windows USB/HID 容器接口。诊断单独保留 `hid_nodes` 原响应；后端未提供枚举完整性时保持部分结果，缺席不推定断连。直接鼠标读取、182 休眠/回报率写入及 9 款键盘亮度读写复用共享路由，Windows 容器解析仅在 `cfg(windows)` 适配模块。打开精确节点并读取实际 descriptor 后才允许协议通信；未知接口号明确不支持。

179 配对父卡/工具后续 Bindings 查询使用 [接收器路由](../../crates/razer-discovery/src/receiver.rs)。Shell 从当前发现中取得唯一物理 owner；Windows 按真实 ContainerId 重枚举，portable 按保留的完整 node 重枚举并核对 descriptor，再发原 V2 查询。回复仍使用同一当前目录/原始 PID/status 投影，按原会话和取消逻辑交给 feature 后增量更新关联设备。页面的 collection 前缀仅决定是否安排读取；它不能授权打开接口。请求完成还核对 discovery revision、owner、工作区身份和完整 node。Dock 的 BIND_INFO 同样使用这一查询路由，但完整 Scan/Pair/Unpair 与 164/241 生命周期仍未闭合。

portable 节点不生成假的 Windows GUID，collection scope 只作本地观察身份。macOS 后端可能返回接口号 -1，目前没有来源等价的接口映射，不能据此声称鼠标/键盘在 macOS 已端到端支持。Linux/macOS UI 物理分组、热插拔及 receiver relay 尚未完整适配；无源核实 collection 选择规格的产品也仍缺发现支持。Windows 专属 COM/SCM/驱动服务在其他平台明确不支持。协议/传输跨平台与整个 UI 三平台验收是不同进度。环境只安装 Windows Rust target，开发只核对 PE、源码、checksum 和 Windows 静态编译，未运行测试、应用、DLL 或硬件查询。
