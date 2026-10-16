# 项目文档

本项目以无需账户和互联网的本地设备配置为目标。每项本地功能需要完整源码证据、Rust 实现和 UI／服务接线；界面外观、路由或协议包装器不代表功能完成。

| 文档 | 用途 |
| --- | --- |
| [本地优先范围](re/local-first-scope.md) | 必须实现的本地功能、需要移除的联网入口和执行边界 |
| [完整实现路线](re/ui-readonly-first-roadmap.md) | 主任务／三个子任务交接、暂停与继续位置，以及逐项实现和验收要求 |
| [架构与证据身份](re/architecture.md) | Rust 分层、本地 IPC、协议版本与证据哈希的用途 |
| [静态逆向工具选型](re/tooling.md) | 专业前端与原生工具的能力、选择理由及执行边界 |
| [本地资源包格式](asset-pack-format.md) | 独立 SQLite、按需媒体缓存及容器验证边界 |
| [工具维护说明](../tools/README.md) | 静态维护／分析入口与项目工具位置 |
| [分析目录说明](../.analysis/README.md) | 原始输入、动态哈希、测试样本和新结果的位置 |
| [开发规则](../AGENTS.md) | 完整 UI、GPUI Kit、当前源码和静态分析约束 |

2026-10-15 已复核保留宿主 4.0.827 获取收据的 36 个文件，其长度与 SHA-256 全部匹配；本机核对的主 EXE、ASAR、mapping_engine.dll、simple_service.dll 与 HID.node 和保留原件一致。六个产品族保留独立 UI／MW 探测矩阵，完整资源与分析输出仍需逐项核对。本轮开始时输入索引缺失，恢复后的索引须重新进行字节审计；它不证明功能语义或全源齐备。IDA 路径为 `E:/IDA`（9.1）或 `D:/ida/IDA Pro 8.3`（8.3），逐输入捕获、失败及工具版本见 [工具说明](re/tooling.md) 和 [当前路线](re/ui-readonly-first-roadmap.md)。

分析缺项集中维护，验证命令与结果见 [验证记录](../.analysis/validation.json)。
