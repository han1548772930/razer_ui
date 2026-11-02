# razer_ui

使用 Rust + GPUI Kit 独立实现当前稳定版 Razer Synapse 的本地设备配置功能。范围依据当前官方源码、二进制和实际调用链；原版没有的功能不新增，已有本地功能完整保留，账户、云同步、在线目录和远程更新业务移除。

当前优先完成全范围语义逆向，再按已确认的行为修正必要架构、实现 Rust 并接通页面与后端。批量捕获已有基础，完整功能语义尚未完成；当前进度、下一批安排及各阶段完成条件统一维护在[路线图](docs/re/ui-readonly-first-roadmap.md)，静态捕获和补查收据见[逆向证据索引](docs/re/native-bulk-reverse.md)。

设备实测、持久化配置和待提交草稿分别建模；真实读写、初始化、应答确认及失败行为以官方调用链为依据。官方业务 DLL 仅用于静态分析和显式对照，不作为生产后端。完整规则见 [AGENTS.md](AGENTS.md)。

| 入口 | 内容 |
| --- | --- |
| [项目文档](docs/README.md) | 各类文档的唯一阅读入口 |
| [产品与共享功能](docs/products/README.md) | 各产品族、型号差异、宿主和共享功能语义 |
| [本地优先范围](docs/re/local-first-scope.md) | 本地能力与排除项 |
| [架构说明](docs/re/architecture.md) | 当前 crate 职责、依赖与状态所有者 |
| [资源交付](docs/asset-pack-format.md) | 独立 SQLite、按需媒体与缓存；编译无需整套媒体 |
| [Windows 安装](docs/windows-installation.md) | 安装配置、普通权限缓存和卸载边界 |
| [维护工具](tools/README.md) | `tools/workspace.py` 统一入口、准备和验证命令 |

分析原件、索引和正式结果保留在 [.analysis](.analysis/README.md)，测试夹具归所属 crate。缺项由现有审阅收据与 `gaps.json` 维护；已确认、已归档且无后续依赖的临时残留通过统一清理计划实际删除，清理结果单独留据。

允许的静态、纯 Rust、模拟及显式 DLL／服务／设备对照测试以根规则为准。应用窗口、厂商 JavaScript、安装器及动态调试不执行；未实际验证的结果保持未验证。
