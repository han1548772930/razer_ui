# 当前直接设备写入

当前已实现产品 182 的直接 DPI 与 USB 高速回报率写入协议，以及 agent/IPC 路由。页面事件尚未连接，其他写入能力尚未实现。开发验证仅进行静态源码/PE 分析、资源检查、格式化和 `cargo check`；运行验收未进行。

## 当前已实现的协议入口

| 产品/操作 | 当前 Razer 原码 | 实现及范围 |
| --- | --- | --- |
| 182 / 活动 DPI X/Y | `setDpiLevel(0,x,y)`，header `[7,4,5]`；X/Y/Z 大端 u16，原 caller 省略 Z、helper 默认 0 | 先读当前值；不同才设置；检查原 setter parser 的应答，再回读比对 X/Y；不写持久化 DPI 阶段表 |
| 182 / USB 高速回报率 | `setUSBHighSpeedPollingRate(profile,hz)`，header `[2,0,64]`；profile caller 默认 1，Hz 转当前源枚举编码 | 检查 setter 应答并回读确认；回读是本应用确认策略，原 setter caller 本身没有该确认步骤 |

完整原文、SHA-256、UTF-16 半开区间、delegate、字节编码器、parser 与实际 caller 保存在 [写入证据](device-write-capabilities-current-evidence.json)。[生成器](../../tools/audit-device-write-capabilities.cjs) 只用 Acorn 解析当前原件并校验原读取/factory 收据，不导入厂商 JS。运行资产为 [device-write-capabilities.json](../../assets/data/device-write-capabilities.json)。

设备 API 位于 [device_writes.rs](../../crates/razer-device/src/device_writes.rs)，协议不依赖系统后端或原 DLL。复用 source-derived Feature envelope、事务、状态及重试参数；这两条命令不属于该产品需要特殊事务 bit 的 `0x46/0x41`。

## 路由、确认与失败

IPC 增加 `HidNodeWrite { node, product_id, setting }` 和 `DeviceWrite { target, setting }`。前者走跨平台 HID node，后者复用现有 Windows 真实 ContainerId/接口观察；两者都在 agent 执行，没有新增 DLL 写调用。当前只允许源核实的直接设备，接收器 relay 和 BLE 写入仍缺逐产品证明，不套用直接设备报文。

同一打开的 transport/锁覆盖读取、设置与回读，操作期间重查真实身份。DPI 校验当前产品源码范围；回报率只接受原枚举。发送完成、收到 setter 应答和设置回读成功分别处理；超时、断连、应答失配或回读不符返回错误，不构造成功状态。应答不明时提示设备可能已接受设置，调用方需要重新读取；不自动以本地草稿覆盖真实观察。

整个确认过程使用 20 秒应用预算，IPC 为 30 秒；这是本项目上限，不是厂商协议常量。值已经一致时仅返回实际读取结果、`changed=false`，不下发 setter。

## 未完成的范围

这两条写入已接到 agent/IPC，尚未接产品页面的编辑事件；本地编辑/保存继续是本地草稿。UI 接入需要对应连接代际、配置作用域与异步请求队列，不能把本地保存改名为设备写入成功。

这不是所有 DLL 全功能完成。全量分母、二级库、COM/驱动/服务与未恢复函数继续见 [DLL 全量清单](dll-function-inventory.md)、[内部通信逆向](dll-device-communication-current.md) 和 [Mixer 二级链](audio-mixer-dll-protocol-current.md)。键盘、耳机、灯光、映射、配对、固件以及其他产品写操作仍需逐项闭合原件证据并实现。没有进行实机或三平台运行验收。
