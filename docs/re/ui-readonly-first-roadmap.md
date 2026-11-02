# 逆向与实现路线图

本页统一维护当前阶段、已确认语义和下一批边界。用户要求的“先逆向、不扩需求、按依赖拆分、证据足够即收束、确认后实际清理”已写入 [AGENTS.md 第 1.1 节](../../AGENTS.md#11-当前执行优先级先完成逆向)及第 7.1 节；本页不重复维护工程规则。正式证据归 `.analysis/` 收据，未决项归 `gaps.json`。

## 当前阶段

当前处于阶段 1—3：核实当前来源、闭合共享调用链并解释真实产品差异；尚未满足阶段 4 的全量语义验收条件。Rust 迁移、UI／后端完整接线和实际 DLL／设备对照继续单独记账。

已有基线：[输入审计](../../.analysis/results/input-audit.json)于 2026-10-29 核对 279,961 项保留输入，缺失／不匹配／错误均为 0；[原生索引](../../.analysis/results/native-bulk/semantic-index.json)覆盖 881 个 DLL／NODE／SYS 模块；[950 项冻结队列](../../.analysis/results/reverse-pending.json)最初通过 889 项，余下 61 项由[后续增量](../../.analysis/results/reverse-pending/unresolved-current.json)复核通过。这些证明输入和选定静态复核结果，收据仍为 `semantics_complete=false`，不能换算为全项目完成比例。各产品现状见[产品文档](../products/README.md)，已有 Rust 验证仍按[独立收据](../../.analysis/validation.json)判断。

本轮收据日期为 2026-11-03（Asia/Shanghai）：

| 链 | 已确认结果 | 当前边界 |
| --- | --- | --- |
| 图片 32×32 resampler | 主机查询 `lighting-media-resampler` 已发布：LightingEngine 4.0.55.0 的 107 个完整函数、61 个 PE 槽、62 条直接边、24 段常量及 21 段汇编；14/14 定向测试通过。详见[动态灯光专题](../products/shared/dynamic-lighting.md)。 | 仅覆盖当前图片 caller 的 layout4/type0、32×32 输出及其 kernel／系数／边界／扫描线。codec 解码、AI 模型、Quick 分支、Rust／UI／后端、实际 DLL／设备和运行验收未闭合。 |
| RzComDriver 安装选支 | 查询 `common-driver-install-selection` 已发布。已核对版本读取、signed threshold `0x000A0000`、架构分支、`RzCommonU.inf`／`RzCommon.inf` 与官方 `RzCommon.sys` 输出名、DPInst 参数及进程边界；28 个选定语义函数、184 个完整捕获函数、12,251 条指令、20 个 owner digest、9 个语义锚点、20 条实际边；12/12 定向测试通过。语义已写入[固件与安装专题](../products/host/firmware-and-installation.md)。 | 旧 SYS 通知链、完整安装／卸载调用方、Rust 实现、安装服务和实际运行对照仍未闭合；`RzCommon_1.sys` 只作为保留原件别名，不是官方安装输出名。 |
| 747 当前 Mapping native 消费者 | 查询 `keypad-mapping-current-native` 已发布。当前 Mapping 1.3.17.2 原件 SHA 为 `c64e539437b50da2b1f2e4155d8cbb0fb1c3e1ebaad894caf509bad97e9857f9`；75 个完整函数、10,054 条指令、24 个 PE 槽、42 条 direct edges；当前选择 26 个唯一 owner；8/8 定向测试通过，747 消费者与 dispatcher 回归 9/9 通过。语义已写入[键盘映射专题](../products/keyboard/customize.md)。 | 已闭合 dpad deferred pair、释放／重选顺序、`gm → otfs → dkp → SnapTap → global shortcut → ordinary` 优先级、active-map break 快照及 `112EE0`／`IOCTL88883020` 输出门禁。其他产品／固件／传输分支、Rust／UI／后端、实际对照和运行验收仍未闭合。 |
| 本轮维护验证 | [工具验证收据](../../.analysis/results/tool-verification.json)记录本轮 47 项定向测试，47/47 通过，失败／错误／跳过均为 0；三个查询审计均无失败，`target_execution=false`、运行验收保持 `unverified`。 | 工具和静态收据通过不代表产品功能、Rust、UI／后端、官方 DLL、设备或窗口验收完成。 |
| gaps 与来源 | [gaps.json](../../.analysis/results/gaps.json) 已按本轮主机收据刷新，选定来源为 `.analysis/results/host-current/local-audit.json`，当前所有者 3,203 个。 | gaps 是所有来源和所有者的汇总，不是去重后的功能数量；获取失败、语义未决、实现未完成和运行验收未执行必须分别处理。 |

此前已冻结的 747 UI、区域选择窗口和内核输入生产链继续作为正式收据使用；本轮没有重复捕获、IDA 会话、数据库副本或临时输出。

## 分阶段路线

| 阶段 | 工作 | 完成条件 |
| --- | --- | --- |
| 1. 确定当前范围 | 复用现有获取与捕获，核对当前部署、目录、挂载／初始化入口和依赖 | 每个范围内功能有产品、版本／固件、传输／平台身份；缺证据项有明确取证路径 |
| 2. 完成共享链逆向 | 优先处理发现／身份、传输、配置档、映射／宏、存储、事件和清理生命周期 | 输入、读取／初始化、观察、计算、写入、应答／读回、消费者、持久化、失败、取消、恢复和释放均有依据 |
| 3. 收束产品差异 | 共享结论接回鼠标、键盘、音频、手柄、配件／摄像头、系统、灯光和主机的真实分支 | 固件／连接／布局差异已解释，未知间接调用和必需来源缺口已解决 |
| 4. 全量逆向验收 | 从 UI 动作向下、从 dispatcher／服务事件向上复核，只补变化输入和受影响依赖 | 范围内没有未归属、漏审或未解释语义缺口，并完成本阶段清理 |
| 5. 独立 Rust 与接线 | 按确认语义迁移协议、平台、服务、状态和原版 UI，接通全部调用方 | 真实读写和消费者闭环，状态／生命周期所有权清晰，官方业务 DLL 生产依赖及失效数据清除 |
| 6. 分项验证 | 运行适用的自动化／模拟测试及已授权的 DLL／服务／设备对照 | 输入、双方结果、差异、恢复和清理均有收据；受限制项保持未验证 |

阶段 1—4 连续推进，阶段 2 与 3 仅在依赖独立时并行；不以编译、单页通过或工具测试通过替代全量语义验收。

## 下一批任务安排

本轮三个子任务已冻结。下一批最多保留三个边界独立槽位，每项从现有收据和 `gaps.json` 接续，不重复领取已完成链：

| 槽位 | 功能链 | 输入与完成条件 |
| --- | --- | --- |
| 协调者 | 收据、查询分发、gaps、路线图和清理 | 先完成本轮清理与文档一致性检查；不修改未受影响的产品实现 |
| A | Kimberly camera identity：3595／3596 wrapper → Host `FFIPreloadMain` → `GetContainerIdFromDeviceId` → 返回字符串与 `FreeMalloc` | 复用 4.0.896 Host 与 `RzNative_GenericCamera_v1.0.35.0.dll` 现有证据，核参数顺序、导出／helper、失败和释放；不重捕获、不扩展其他 camera 功能 |
| B | Lighting codec／解码及当前 resampler 消费者 | 只补当前 source 中仍未闭合的格式分派、像素展开、位深转换和调用方；不得把 32×32 resampler 结论扩展成全部媒体功能 |
| C | RzComDriver 旧 SYS 通知链及其安装／卸载消费者 | 从本轮确认的 legacy 分支接续，核旧 SYS 真实通知、输入格式与释放，再接回调用方；不重做 OS 选支，不执行安装器或驱动 |

槽位 A 已领取，当前已定位 generic route、DLL 版本和导出 helper，具体 action 接线尚未冻结；B、C 为下一批候选，领取前先核已有收据。相同输入无新证据时更换取证路径，其余独立任务继续。

## 每批收尾与文档维护

正式结果、引用接线和定向验证完成后：

1. 运行 `python -X utf8 tools/workspace.py clean --dry-run`，核对计划路径、SHA、候选、保留原因、锁和占用。
2. 仅对复核后的计划执行 `clean --apply <计划> --plan-sha256 <SHA>`；候选为空也保留实际 apply 收据。
3. 用 `git diff --check`、Python AST／编译和文档相对链接检查收尾；失败事实写入收据，不删除正式原件或未决证据。
4. 路线与当前阶段只写入本页，具体语义写入产品专题，机器可读结果写入 `.analysis/results/`；删除重复历史叙述、过期分工和失效链接，不新增平行进度档案。

本轮[清理计划](../../.analysis/results/scratch-cleanup-20261102T225507249941Z-plan.json)已核对 SHA 并[实际应用](../../.analysis/results/scratch-cleanup-20261102T225507249941Z-plan-applied.json)：保护检查失败 0、候选 0、实际删除 0、失败 0、释放 0 字节；189 项因正式引用、未决分析、后续依赖或锁保留。本轮三个切片没有新增临时捕获或数据库。
