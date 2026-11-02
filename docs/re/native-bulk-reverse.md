# 逆向证据索引与静态复核边界

本页只维护捕获、静态复核和专项语义的证据入口；当前阶段、任务调度与后续实现统一见[路线图](ui-readonly-first-roadmap.md)。静态索引、函数捕获、完整功能语义、Rust 实现和实际 DLL／设备结果分别记录。

## 基础捕获统计

下列是[原生索引](../../.analysis/results/native-bulk/semantic-index.json)自身快照，不是当前功能完成数量。

| 项目 | 快照记录 |
| --- | ---: |
| 输入捕获收据 | 1,623 |
| DLL／NODE／SYS 模块 | 881 |
| 函数总数／已捕获 | 2,603,678／2,603,678 |
| 历史基础反编译失败条目，未扣后续补查 | 869 |
| 导出入口／有函数体 | 64,092／60,843 |

完整模块清单、输入身份、函数边界及捕获结果直接读取 [semantic-index.json](../../.analysis/results/native-bulk/semantic-index.json) 和 [capture-index.json](../../.analysis/results/native-bulk/capture-index.json)，不在 Markdown 复制数百行原件／SHA 表。此索引按 DLL／NODE／SYS 筛选，不代表全部 EXE、安装生命周期或完整依赖闭包。

## 统一分析范围

以当前官方实际挂载、初始化、事件消费者和本地依赖链决定范围，核对产品、版本／固件、传输与平台分支。共享链复用必须绑定实际字节及消费者；文件名相似、同族型号和相同 getter 不足以外推。范围规则见 [AGENTS.md](../../AGENTS.md#11-当前执行优先级先完成逆向)。

<!-- reverse-pending:begin -->
## 统一剩余静态任务队列

运行入口：`python -X utf8 tools/workspace.py reverse-pending --workers 16 --process-workers --resume`。本次以有界进程池复核原件身份、完整捕获及产物、补查冲突和前端解析；不启动 IDA 或网络修复。不执行厂商代码，不修改 Rust，不重新启动仍存活的 IDA 会话。

本次队列登记 950 项；状态：`{"passed": 889, "unresolved": 61}`。完整任务身份、实际命令、输出和未决原因见[统一收据](../../.analysis/results/reverse-pending.json)。passed 只表示该任务的静态检查通过；队列结束、伪代码恢复、完整语义、Rust 实现和实际设备验收分别记账，不由脚本自动提升为全功能完成。
<!-- reverse-pending:end -->

<!-- reverse-pending-current:begin -->
## 未决项当前复核

冻结基线的 61 项未决已复核 61 项；当前状态：`{"passed": 61}`，尚未复核 0 项。以[当前统一收据](../../.analysis/results/reverse-pending/unresolved-current.json)及其不可变 `source_review` 为准；上面的 950 项收据保留原完成事实。

静态通过可来自完整反编译、原始函数上下文、精确导入跳转或独立核验的来源汇编／架构契约，每项保留具体依据和原始分析器失败。静态语义、Rust 独立实现、UI／后端接线、模拟验证和实际 DLL／设备验收分别记录；本轮复核不提升后几项的完成状态。
<!-- reverse-pending-current:end -->

上述增量覆盖 49 项原生与 12 项前端静态未决，收据仍明确 `semantics_complete=false`。原失败、缺失 source map、旧部署和工具来源限制继续保留。不能把“这 61 项静态未决归零”解释为全项目语义逆向完成，也不为重新计算总数反复运行已完成队列。生成器拥有的两段标记保留，由 `tools/reverse_pending.py` 更新。

## 补查后的当前边界

| 证据入口 | 用途与边界 |
| --- | --- |
| [输入索引](../../.analysis/inputs.json)、[输入审计](../../.analysis/results/input-audit.json) | 当前所选原件及实际字节身份；索引核对通过不代表全部官方来源已取得 |
| [全库失败补查复核](../../.analysis/results/native-bulk/reanalysis-verification.json) | 保留 241 个有基础失败模块的历史复核，869 条历史失败中恢复 212、仍记录 657；不能与不同任务快照直接相减 |
| [字符转换补查](../../.analysis/results/native-reanalysis-batch.json)、[返回缓冲区补查](../../.analysis/results/native-bulk/demangle-reanalysis.json)、[大函数补查](../../.analysis/results/native-bulk/large-reanalysis.json) | 各自的输入、完整函数、冲突和恢复事实；不是完整产品语义验收 |
| [产品与共享功能](../products/README.md) | 实际页面／MW、原生消费者、产品差异和剩余语义的维护入口 |
| [宿主审阅](../../.analysis/results/host-current/local-audit.json) | 主机本地功能、状态／事件与退出链，按所选 query 的实际覆盖判断 |
| [统一缺项](../../.analysis/results/gaps.json) | 汇总当前各所有者的获取、语义、实现和验收缺项；记录数不等于独立功能数 |
| [Rust 验证](../../.analysis/validation.json) | 当前所选测试、输入身份、实际输出与未执行项；历史通过不累计为本批全套 |

具体产品链直接从所属文档进入：鼠标[高级灵敏度](../products/mouse/advanced-sensitivity.md)、键盘[映射](../products/keyboard/customize.md)与[板载配置](../products/keyboard/profiles.md)、音频[播放与 EQ](../products/audio/playback-and-eq.md)、[摄像头](../products/accessories/camera.md)、[动态灯光](../products/shared/dynamic-lighting.md)、[主机窗口生命周期](../products/host/window-lifecycle.md)。当前身份、完整所有者、差异和失败由这些专题及其正式收据维护，避免在本页重复追加历史批次叙述。

## 结论边界

全量语义完成要求全部范围内调用链查清：状态来源、初始化／读取、观察、输入／计算、真实写包、应答／读回、事件消费者、持久化、失败、取消、恢复和释放，以及所有实际产品分支。未解析的间接调用与必需来源缺口继续补查。

完整语义确认前不进入生产迁移；官方 DLL 只用于静态证据和已授权的显式对照。应用、安装器、厂商 JavaScript 和动态调试继续遵守根目录限制。正式结果接线后，按[清理方法](tooling.md#历史临时文件的保守清理)实际删除无后续依赖的残留，保留原件、有效收据和真实断点依赖。
