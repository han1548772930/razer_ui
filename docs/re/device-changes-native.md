# 设备插拔后的发现更新

USB/HID 插拔事件触发重新枚举。接口枚举、接收器无线状态和鼠标配置读取是独立链路，不能用一次成功枚举表示设备配置已读取。

当前原版在 4.0.827 `UsbRzDeviceAction.initUSB` 调用 `startMonitoring`，发布 `nodeUSBEvent-add/remove`；当前 background 源分别消费事件进入连接/断开链。三条完整 AST、文件 SHA-256 与区间见 [当前收据](device-changes-current-evidence.json)，由 `tools/audit-device-changes.cjs` 静态解析生成。USB_DEVICE 类 GUID 另由当前 `detection.node` 的静态 PE 字节核实。

原生适配使用 Windows `CM_Register_Notification` 订阅 USB_DEVICE 和 HID 接口的 arrival/removal，只通知既有发现流程重新枚举；没有执行 Node 插件，也不把 Windows API 称为已证明的厂商 C 导出。接口事件本身不创建产品、填入配置或推定接收器的无线 peer 状态。

- `src/backend/device_changes.rs`：专属线程注册/注销。回调只向容量 1 的通道投递失效信号，合并同轮事件；注册完成后再通知重扫，覆盖首次枚举与订阅的交错。Box 保存的上下文在注销完成后释放；若注销失败则保留上下文以免悬空回调。UI 销毁仅断开停止通道，不等待原生注销。
- `src/shell/runtime_page.rs`：监听任务以弱实体更新；忙时保留一次后续扫描，手工服务刷新不会被自动发现覆盖。主动断开时先取消监听及排队刷新，再关闭 worker。注册失败明确显示，可手动刷新。先发布接口与配对发现，等待 shell 建立工作区/捕获配置与连接代际并确认后，再读取参数；后半段不重新刷新连接代际。慢参数查询不阻止首页先出现设备，切换本地配置后到达的旧结果不能注入新配置。
- `src/shell/runtime_page/diagnostics.rs`：连接 actor 将本次真实 USB/HID 响应、发现与逐字段读取错误、运行文件路径及时间记录到 `%APPDATA%/razer_ui/discovery-latest.json`，同目录临时文件写完后替换。记录与 `profiles.json` 分离，不创建历史假设备；没有请求的阶段记为 not_requested，成功子项和失败子项分别保留，完整配置读取始终另行标为 false。此文件只会在用户实际运行应用后生成。
- `Device.dashboard.readonly_values/readonly_power` 全部使用 serde skip，读取固件/电量只供当前会话的帮助/电量 UI 消费，不进入草稿或磁盘，不触发 Changed。即时 DPI 保留独立 x/y 读值，不推造 DPI 阶段；连接失效会清空本轮读值。新增仅编译用例覆盖读取前后序列化结果完全不变与连接失效清理。

回调、上下文寿命、注销线程、Ready/枚举交错、忙时合并及断开清理只完成静态核对；编译与修复边界见 [修复登记](ui-fix-registry.json)。Windows 监听、应用、测试和 DLL 均未运行验收。

仍需完成接收器内部无线开关/待机变化的源 MW 观察链，以及各设备的实际配置查询。不能用 USB/HID 插拔覆盖这些状态，更不能以绿色编译替代实机读取。
