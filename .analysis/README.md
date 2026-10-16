# 静态分析数据

本目录保存原始输入、外部架构／工具参考、分析结果和验证记录。

| 位置 | 内容与状态 |
| --- | --- |
| [inputs.json](inputs.json) | 本轮已从保留证据恢复并重新审计；记录相对路径、原始字节长度与 SHA-256，缺失和身份不符仍保留，不能把已存在文件扫描当作原始来源证明 |
| `inputs/` | 保留的原始源码、二进制和获取记录；当前版本与完整性待核对 |
| `references/` | OpenLogi 等外部参考，只用于其明确用途；不能作为 Razer 协议证据 |
| `results/` | 原生函数体、前端模块／资源引用、工具诊断、MCP 连接核验和统一 [缺项清单](results/gaps.json) |
| [validation.json](validation.json) | 静态检查和纯／模拟测试结果、命令及代码身份 |
| `verification/` | 当前验证命令的完整输出，身份由验证记录核对 |

Dashboard 已保存官方原始 HTML／JS／CSS／资源及获取前后入口／manifest 字节稳定性记录；124 个引用的 source map 不可用。宿主、其他设备类别和原生输入的当前版本与依赖闭包需逐项核对。具体原件、来源及未解项见 [输入审计](results/input-audit.json)、[corpus](results/corpus.json) 和 [gaps](results/gaps.json)。

鼠标、键盘、音频、手柄、系统、配件的 UI／MW 分别保存在 [产品族矩阵目录](results/families)。候选表只用于定位端点。各矩阵逐产品分开记录探测、原始资源获取、静态恢复、语义审查与实现验收；端点或下载覆盖不能解释为完整页面。此前未核验的数量不作为本轮完成证明，缺失明细保存在统一审计和 gaps。

2026-10-15 已对 [宿主收据](inputs/host-current/acquisition.json) 中 36 个原件重新计算长度和 SHA-256，均与 2026-10-14 获取记录匹配。本机主 EXE、ASAR、mapping_engine.dll、simple_service.dll 与 HID.node 也与保留宿主原件匹配。同日 [官方部署元数据复核](results/host-current/metadata-recheck.json) 双取 6 个官方 JSON 端点，全部与旧基线字节一致，Synapse／Chroma 声明版本仍为 4.0.827；宿主完整依赖闭包仍为 false。产品 UI 和 MW 分别使用独立版本／构建号，不因 ID 相同而推断兼容。

本轮 [installed-current 静态获取](inputs/installed-current/acquisition.json) 已重新完成：564 个安装文件、5,093 个官方公共静态缓存容器及其响应资源，获取／解析失败为 0；6 个前端缓存组无同 URL 字节冲突，当前部署与完整资源闭包仍须独立核验。当前 IDA 为用户指定的 `E:/IDA` 中 9.1／Hex-Rays 9.1.0.250226；原生完整函数、反编译失败及汇编以各 SHA 的 capture 为准，全函数捕获不表示语义或产品接线完成。工具身份与未完成的 AppEngine／全语料捕获见 [工具说明](../docs/re/tooling.md)。

宿主 native 应用资源沿实际稳定 HTML manifest 与设备资源统一获取；本轮静态提取 16 个容器、676 个文件。corpus 另保存当前 installer 的完整下载策略，并以完整官方文件的精确区间证明 15 份 `pindex` 缓存属于分片；原分片保持独立，不当作完整模块。8 个官方资源与清单的实际字节差异及未证明来源保留，完整依赖、安装分支、ABI 与产品实现继续逐链推进。

构建／测试样本位于所属 crate 的 `testdata/`，其来源语义需单独核对。构建不得依赖 `.work/` 或分析报告。

所有输入哈希从实际字节计算，路径相对项目根。换电脑时复制原始输入并运行 `python tools/workspace.py verify`；只复制 Git 跟踪文件不会带上被忽略的原始输入。证据身份变化需要重审对应分析，不能成为产品运行门槛。

分析目标始终作为数据处理。禁止执行厂商 JavaScript、应用、DLL／导出函数、helper、installer、真实服务或真实设备；IDA／Hex-Rays 只做静态分析，不启用目标调试执行。工作流规则见 [完整实现路线](../docs/re/ui-readonly-first-roadmap.md)。
