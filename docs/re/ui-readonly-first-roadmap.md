# 逆向与实现路线图

本页统一维护当前阶段、已确认语义和下一批边界。用户要求的“先逆向、不扩需求、按依赖拆分、证据足够即收束、确认后实际清理”已写入 [AGENTS.md 第 1.1 节](../../AGENTS.md#11-当前执行优先级先完成逆向)及第 7.1 节；本页不重复维护工程规则。正式证据归 `.analysis/` 收据，未决项归 `gaps.json`。

## 当前阶段

当前处于阶段 1—3：核实当前来源、闭合共享调用链并解释真实产品差异；尚未满足阶段 4 的全量语义验收条件。Rust 迁移、UI／后端完整接线和实际 DLL／设备对照继续单独记账。

已有基线：[输入审计](../../.analysis/results/input-audit.json)于 2026-10-29 核对 279,961 项保留输入，缺失／不匹配／错误均为 0；[原生索引](../../.analysis/results/native-bulk/semantic-index.json)覆盖 881 个 DLL／NODE／SYS 模块；[950 项冻结队列](../../.analysis/results/reverse-pending.json)最初通过 889 项，余下 61 项由[后续增量](../../.analysis/results/reverse-pending/unresolved-current.json)复核通过。这些证明输入和选定静态复核结果，收据仍为 `semantics_complete=false`，不能换算为全项目完成比例。各产品现状见[产品文档](../products/README.md)，已有 Rust 验证仍按[独立收据](../../.analysis/validation.json)判断。

既有共享链和产品结论继续复用，详细范围归[产品专题](../products/README.md)及其正式收据。已确认的配置档导入、轴体批写、相机 HDR、通知消费者和共享关闭链不重新领取；本批从 AMD Package 温度、XTU 服务信息和键盘 AppEngine 应用的精确缺项接续。任务 completed、文件落盘、设备确认与回调排空继续分开。

本批收束日期为 2026-11-04（Asia/Shanghai）：

| 链 | 已确认结果 | 当前边界 |
| --- | --- | --- |
| Hanbo AMD K10／K12 Package 温度 | [配件收据](../../.analysis/results/frontend/accessories-main-audit.json)的 `hanbo_amd_package_chain` 复用 67 个完整原生函数、33 个 PE 槽和 17 个 JS 所有者，确认实际 vendor／family 分派、PCI 双读、温度换算及释放。 | 第二次读取失败仍可能算出 255.875℃；ECS 保存／启用／恢复均未确认成功，不能当作有效观测或恢复确认。其他 AMD provider、优先 owner 和非温度初始化依赖分开继续，详见[AMD 专题](../products/accessories/lighting-and-cooling.md#amd-k10k12-package-温度的-pci-读取与恢复)。 |
| PID737 XTU 服务信息与兼容性 | [系统收据](../../.analysis/results/frontend/system-mw-audit.json)的 `overclock_service_info_chain` 复核 63 个完整 IL 方法，确认 10.0.1.35 服务候选的启动、字典查询、兼容性生产／消费、失败和停止链。 | `Connect` 恒 true；客户端只接受兼容性恰好为 0，失败可保留 channel。双 MSI 的实际安装选择、IPF／驱动和其他服务仍未闭合，详见[XTU 专题](../products/system/thermal-performance.md#pid737-xtu-服务信息与兼容性)。 |
| PID756 AppEngine 生成与 OBM 阈值应用 | [键盘收据](../../.analysis/results/frontend/keyboard-main-audit.json)的 `keyboard_756_appengine_apply_chain` 核对 79 个 AST 所有者，复用已有存储和 HID 捕获，确认变化 profile 生成、当前引擎选择、15 条阈值批写、缓存及存储交接。 | 真实 NACK 抛错并释放 mutex；成功后缓存提交值，没有额外 GET／逐项阈值读回。全部输出执行器、条件 GameMode 和 analog 注册仍有独立边界，详见[AppEngine 专题](../products/keyboard/calibration.md#756appengine-生成与-obm-阈值应用)。 |
| 维护验证与正式发布 | [工具验证收据](../../.analysis/results/tool-verification.json)记录 75/75 通过，54 项工具输入前后稳定，失败／错误／跳过均为 0；三个正式 owner 已回读，原有 7／8／9 个顶层字段完整摘要均不变。 | 修复 mixed CLR 同类型、同 RVA／长度、不同方法名的精确选择错误，实际别名反例已覆盖；首次联合验证的 1 个错误按 `previous_receipt` 保留。三个新段均为 `bounded_observation_complete=true`、`snapshot_stable=true`，仍为 `source_chain_complete=false`、`target_execution=false`。 |
| gaps 与来源 | [gaps.json](../../.analysis/results/gaps.json)只刷新本批配件、系统和键盘 owner，其余 3,200 个 owner 的完整摘要不变，原获取失败及缺项保留。 | 三个新段的 7／4／4 条接续项均逐条匹配缺项。工具验证与捕获数量不代表 Rust、UI／后端、实际 DLL／设备或窗口验收完成，也不换算为功能完成比例。 |

本批按 AMD Package、XTU 服务信息和键盘生成／应用拆成三个独立子任务，均已冻结并正式发布；协调者接通统一查询，独立核对 PCI 双读与 ServiceInfo 字典顺序，完成联合验证、旧字段比对、收据回读和 gaps 更新。复用现有完整捕获，未新增临时副本或分析会话，不修改产品 Rust；已有 K17 温度／时钟及键盘生成／提交实现事实单独保留，本批未重验，全量逆向验收仍是迁移前置。

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

## 当前并行任务

AMD 温度、XTU 服务信息和键盘映射应用已达到上述静态边界；当前三个子任务已轮换到手柄原生通知、相机和鼠标。以下入口已与当前正式段及 gaps 核对；各任务只补真正未决的范围：

| 槽位 | 功能链 | 输入与完成条件 |
| --- | --- | --- |
| 协调者 | 来源、共享分发、正式发布、gaps、专题和清理 | 串行发布共享 owner；按真实字段／依赖拆工，必要时把已稳定 collector 的测试交给空闲子任务。只刷新变化输入及引用 |
| A | PID2636 实际 USB 检测原件 | 从[手柄收据](../../.analysis/results/frontend/gamepad-mw-audit.json)的 `/gamepad_2636_removal_notification_chain/remaining_chains/0` 接续；核当前 detection.node 的 Node 注册、Windows 通知回调、PID／container／instance 构造和停止／排空。先复核原件及完整捕获是否已补齐，再按真实方法入口静态分析；复用已确认 JS 订阅和消费者，不由字符串猜 ABI。独占原生通知工具块、测试、2636 专题及所需私有分析会话的清理 |
| B | PID3595 MJPEG autoQuality | 从[相机收据](../../.analysis/results/frontend/accessories-camera-local-pages-audit.json)的 `/camera_3595_hdr_chain/remaining_local_chains/0` 接续，核实际 Processing 开关的 maker、runner、模式／配置档消费者、真实命令与应答、失败、取消和预览恢复。复用已确认 HDR 与停启链；不由未挂载 selector 新增控件。独占 MJPEG 工具块、测试及相机专题 |
| C | 本地配置导入 legacy worker | 从[鼠标收据](../../.analysis/results/frontend/mouse-mw-audit.json)的 `/mouse_182_profile_import_chain/remaining_chains/0` 接续，定向复核当前 `/profile-migration/worker.js` 部署，核 `loadSyn3File/toSyn4` 的实际产品／版本转换、宏坐标和 monitor-service 返回分支。复用已确认 Import 所有权和失败生命周期，按真实转换分批；不重复导入 UI 或执行厂商 JS。独占 worker 工具块、测试及导入专题 |

领取前核最新收据与缺项，发现已完成即换下一项。每条链的完成条件是输入／结果消费者、失败／取消／释放和适用分支可复核；相同输入无新证据时换取证路径，其余独立任务继续。

## 每批收尾与文档维护

正式结果、引用接线和定向验证完成后：

1. 运行 `python -X utf8 tools/workspace.py clean --dry-run`，核对计划路径、SHA、候选、保留原因、锁和占用。
2. 仅对复核后的计划执行 `clean --apply <计划> --plan-sha256 <SHA>`；候选为空也保留实际 apply 收据。
3. 用 `git diff --check`、Python AST／编译和文档相对链接检查收尾；失败事实写入收据，不删除正式原件或未决证据。
4. 路线与当前阶段只写入本页，具体语义写入产品专题，机器可读结果写入 `.analysis/results/`；删除重复历史叙述、过期分工和失效链接，不新增平行进度档案。

本批已核实并应用[清理计划](../../.analysis/results/scratch-cleanup-20261104T030438463939Z-plan.json)，计划 SHA-256 为 `5866362575ab58d63b8d9cdc085961a1c543702a7969e20abbccdbfb1262230c`；[执行收据](../../.analysis/results/scratch-cleanup-20261104T030438463939Z-plan-applied.json)状态为 `completed`，候选、实际删除、失败和释放空间均为 0。175 项保留内容已核对存在性与路径边界：136 项分析未完成／失败／未复核、26 个共享预留锁、2 项缺少完成依据、2 项上传／重建尚未核实、9 项仍被引用或活动中。本批没有新增临时副本，保留内容的具体依赖与原因归同一计划；历史清理收据继续保留，不在本页重复累计。

本批更新现有三个专题、工具说明及本页，替换过期分工、结果与清理叙述，未新增平行进度文档。五份文档的 246 个相对链接、30 个章节引用、四份受影响 Python 文件的 AST 与 `git diff --check` 均通过；规则、当前路线和产品证据分别维护唯一入口。
