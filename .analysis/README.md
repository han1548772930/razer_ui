# 静态分析数据

本目录保存原始输入、分析结果和验证记录。范围、执行与清理规则见 [AGENTS.md](../AGENTS.md)，当前阶段与子任务见[统一路线图](../docs/re/ui-readonly-first-roadmap.md)；此处不维护平行进度。

| 位置 | 内容与状态 |
| --- | --- |
| [inputs.json](inputs.json) | 原件的项目相对路径、字节长度、SHA-256 和来源索引；缺失或身份不符保留在审计中 |
| `inputs/` | 原始源码、二进制、部署／版本声明和获取收据；是否当前及依赖闭包按所属收据判断 |
| `references/` | 外部架构与工具参考，只用于其明确用途；不能作为 Razer 协议证据 |
| `results/` | 原生函数体、前端模块／资源引用、工具诊断、MCP 连接核验和统一 [缺项清单](results/gaps.json) |
| [validation.json](validation.json) | 静态检查和纯／模拟测试结果、命令及代码身份 |
| `verification/` | 当前验证命令的完整输出，身份由验证记录核对 |

原件、来源及缺项从[输入审计](results/input-audit.json)、[corpus](results/corpus.json) 和 [gaps](results/gaps.json)进入。保留输入一致、当前部署稳定、依赖完整、函数捕获、语义完成与运行验收是不同层级；具体结论由所属收据证明。

鼠标、键盘、音频、手柄、系统、配件的 UI／MW 从[产品族矩阵目录](results/families)定位。正式语义查询位于 `results/frontend/` 与[主机收据](results/host-current/local-audit.json)，原生捕获位于 `results/native/<原件SHA>/`；失败与后续补查分别保留，不能由函数数量推断页面或产品完成。

主机当前版本沿[部署元数据](inputs/host-current/metadata-recheck.json)、版本所属的 `inputs/host-current/releases/<版本>/acquisition.json` 及实际提取收据核实；顶层旧获取和[安装快照](inputs/installed-current/acquisition.json)不能自动视为当前。ASAR、外部资源、产品 UI／MW、固件和传输版本分别取证；缓存分片须有完整原件区间证明，不能当作独立模块。分析器能力与命令见[工具说明](../docs/re/tooling.md)。

固定表的维护投影保存在 `inputs/typed-tables/`，与原始厂商证据分开标记；资源映射统一维护于 `tools/resource-mappings.json`，不保留旧 Rust 资源片段目录。产品和 Cargo 不读取这些分析维护输入。

构建／测试样本位于所属 crate 的 `testdata/`，其来源语义需单独核对；构建与测试不得依赖分析原件、报告或 `.work/` 临时数据库。

换电脑时需复制被 Git 忽略的保留原件，再通过 `python -X utf8 tools/workspace.py verify` 核对；只复制跟踪文件不包含原始输入。

常规分析只静态处理原件。官方 DLL／服务／设备功能对照仅从根规则明确授权的独立维护入口执行，按其来源、ABI、恢复与释放条件记录；应用、厂商 JavaScript、安装器和动态调试继续遵守根规则的禁止项。
