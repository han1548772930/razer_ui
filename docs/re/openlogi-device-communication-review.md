# 设备通信架构参考

本文记录 [OpenLogi](https://github.com/AprilNEA/OpenLogi) 的架构参考及本项目的职责对应。Razer 功能、字段和报文以当前 Razer 原件证据为依据。

## 采用的设计方式

| 参考设计 | Razer 项目归属 | 实际用途 |
| --- | --- | --- |
| 协议层定义传输契约，系统后端实现契约 | `razer-device`、`razer-hid` | 协议与操作系统实现分开；`razer-hid` 依赖设备层 |
| 原始接口与逻辑设备分开 | `razer-hid`、`razer-discovery` | 区分 HID collection、物理设备、接收器及其真实子设备 |
| 设备能力决定可用操作 | `razer-device`、`razer-catalog` | 只提供当前 Razer 原件已证明适用的读写操作 |
| 后台执行与界面分开 | `razer-agent`、`razer-service`、`razer-ipc` | UI 发请求，agent 执行并返回真实结果；页面不依赖 HID 实现 |
| 写入检查设备路由并处理响应、错误 | `razer-device::device_writes` | 重新核对目标、验证应答与回读；发送完成不等于设置成功 |

具体包边界和仍存在的兼容入口见 [当前 workspace 架构](workspace-architecture-current.md)。没有引入上游协议依赖或把其设备字段写入 Razer 运行数据。表中的能力和命令必须另有 Razer 原代码证据；参考架构不能补全未知参数。

## Razer 功能依据

- DLL 与服务：[全量功能清单](dll-function-inventory.md)、[内部通信逆向](dll-device-communication-current.md)。
- 现有跨平台查询：[HID 实现](cross-platform-hid-current.md)、[设备读取证据](mouse-read-capabilities-current-evidence.json)。
- 当前直接写入：[写入证据](device-write-capabilities-current-evidence.json)。产品范围、命令、选择参数和解析方式来自当前 Razer middleware；不扩推到其他产品。

读写接口分别记录产品适用条件、目标身份、响应和失败；未接入的消费者保留为缺口。开发仅进行静态分析、格式检查及 `cargo check --locked --all-targets`。

## 外部参考收据

参考版本为 commit `1505c6525470bc0a38ae3ba79d70e950347d2532`。[原始审阅收据](openlogi-device-communication-review.json) 保存静态读取的 157 份上游源码身份和摘录，仅用于复核参考版本，不能当作 Razer 功能证据。[架构收据](workspace-openlogi-architecture-current.json) 保存实际包边界的对照。

[audit-openlogi-reference.cjs](../../tools/audit-openlogi-reference.cjs) 只读取文件、核对 SHA/Git blob 和文本区间，不导入或执行上游代码。源码缓存缺失时，该工具的 `--fetch --check` 按固定 commit 准备文本；不会自动追踪远端 master。
